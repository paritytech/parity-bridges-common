// Copyright 2019-2021 Parity Technologies (UK) Ltd.
// This file is part of Parity Bridges Common.

// Parity Bridges Common is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity Bridges Common is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity Bridges Common.  If not, see <http://www.gnu.org/licenses/>.

//! Module that adds XCM support to bridge pallets. The pallet allows to dynamically
//! open and close bridges between local (to this pallet location) and remote XCM
//! destinations.
//!
//! Every bridge between two XCM locations has a dedicated lane in associated
//! messages pallet. Assuming that this pallet is deployed at the bridge hub
//! parachain and there's a similar pallet at the bridged network, the dynamic
//! bridge lifetime is as follows:
//!
//! 1) the sibling parachain opens a XCMP channel with this bridge hub;
//!
//! 2) the sibling parachain funds its sovereign parachain account at this bridge hub. It shall hold
//!    enough funds to pay for the bridge (see `BridgeReserve`);
//!
//! 3) the sibling parachain opens the bridge by sending XCM `Transact` instruction with the
//!    `open_bridge` call. The `BridgeReserve` amount is reserved on the sovereign account of
//!    sibling parachain;
//!
//! 4) at the other side of the bridge, the same thing (1, 2, 3) happens. Parachains that need to
//!    connect over the bridge need to coordinate the moment when they start sending messages over
//!    the bridge. Otherwise they may lose messages and/or bundled assets;
//!
//! 5) when the bridge is opened, anyone can watch for bridge rules (see
//!    [`bp_xcm_bridge_hub::BridgeMisbehavior`] for details) violations. If something goes wrong
//!    with the bridge, he may call the `report_misbehavior` method. If the report is confirmed, the
//!    inbound channel with the parent/sibling sending chain is suspended and the `Penalty` is paid
//!    (out of reserved funds) to the reporter;
//!
//! 6) if the bridge owner has managed to resolve misbehavior reason, it may resume the regular
//!    bridge operations by calling the `resume_misbehaving_bridges` method. This method doesn't
//!    check that the misbehavior has been fixed. The bridge owner must have enough funds to
//!    replenish his bridge reserve by the `Penalty` amount for every misbehaving bridge;
//!
//! 6) when either side wants to close the bridge, it sends the XCM `Transact` with the
//!    `close_bridge` call. The bridge is closed immediately if there are no queued messages.
//!    Otherwise, the owner must repeat the `close_bridge` call to prune all queued messages first.
//!
//! The pallet doesn't provide any mechanism for graceful closure, because it always involves
//! some contract between two connected chains and the bridge hub knows nothing about that. It
//! is the task for the connected chains to make sure that all required actions are completed
//! before the closure. In the end, the bridge hub can't even guarantee that all messages that
//! are delivered to the destination, are processed in the way their sender expects. So if we
//! can't guarantee that, we shall not care about more complex procedures and leave it to the
//! participating parties.
//!
//! There's other way to cause pauses in bridge operations - it is what we call a "misbehavior".
//! That's a situation when the bridge owner/maintainer have violated contract between its chain and
//! the bridge bub. This contract is quite simple - we want to see there's a running relayer,
//! serving the bridge lane and that the number of messages in all bridge queues stays below some
//! hard limit. The latter requirement may be achieved using some rate limiter at the sending chain.
//! We have an example implementation in the `pallet-xcm-bridge-hub-router` pallet.

#![cfg_attr(not(feature = "std"), no_std)]

use bp_messages::{LaneId, LaneState, MessageNonce};
use bp_runtime::{AccountIdOf, BalanceOf, BlockNumberOf, RangeInclusiveExt};
use bp_xcm_bridge_hub::{
	bridge_locations, Bridge, BridgeLimits, BridgeLocations, BridgeLocationsError,
	BridgeMisbehavior, BridgeState, LocalXcmChannelManager,
};
use frame_support::traits::{
	tokens::BalanceStatus, Currency, ExistenceRequirement, ReservableCurrency,
};
use frame_system::Config as SystemConfig;
use pallet_bridge_messages::{Config as BridgeMessagesConfig, LanesManagerError};
use sp_runtime::{traits::Zero, Saturating};
use xcm::prelude::*;
use xcm_executor::traits::ConvertLocation;

pub use pallet::*;

mod mock;

/// The target that will be used when publishing logs related to this pallet.
pub const LOG_TARGET: &str = "runtime::bridge-xcm";

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::config]
	#[pallet::disable_frame_system_supertrait_check]
	pub trait Config<I: 'static = ()>:
		BridgeMessagesConfig<Self::BridgeMessagesPalletInstance>
	{
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self, I>>
			+ IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Runtime's universal location.
		type UniversalLocation: Get<InteriorMultiLocation>;
		// TODO: https://github.com/paritytech/parity-bridges-common/issues/1666 remove `ChainId` and
		// replace it with the `NetworkId` - then we'll be able to use
		// `T as pallet_bridge_messages::Config<T::BridgeMessagesPalletInstance>::BridgedChain::NetworkId`
		/// Bridged network id.
		#[pallet::constant]
		type BridgedNetworkId: Get<NetworkId>;
		/// Associated messages pallet instance that bridges us with the
		/// `BridgedNetworkId` consensus.
		type BridgeMessagesPalletInstance: 'static;

		/// Maximal number of bridges per single local origin (parent/sibling chain).
		#[pallet::constant]
		type MaxBridgesPerLocalOrigin: Get<u32>;

		/// A set of XCM locations within local consensus system that are allowed to open
		/// bridges with remote destinations.
		// TODO: there's only one impl of `EnsureOrigin<Success = MultiLocation>` -
		// `EnsureXcmOrigin`, but it doesn't do what we need. Is there some other way to check
		// `Origin` and get matching `MultiLocation`???
		type AllowedOpenBridgeOrigin: EnsureOrigin<
			<Self as SystemConfig>::RuntimeOrigin,
			Success = MultiLocation,
		>;
		/// A converter between a multi-location and a sovereign account.
		type BridgeOriginAccountIdConverter: ConvertLocation<Self::AccountId>;

		/// Amount of this chain native tokens that is reserved on the sibling parachain account
		/// when bridge open request is registered.
		///
		/// This value must be selected using following in mind: part of this amount
		/// (`Self::Penalty`) may go to misbehavior reporter, but still the remainder must be large
		/// enough to cover cost of remaining `close_bridge` transactions and make it profitable for
		/// bridge owner to submit those transactions and actually close the bridge. If it will be
		/// small enough and won't cover those costs, the owner will have no reason to close the
		/// bridge.
		#[pallet::constant]
		type BridgeReserve: Get<BalanceOf<ThisChainOf<Self, I>>>;
		/// Currency used to pay for bridge registration.
		type NativeCurrency: ReservableCurrency<Self::AccountId>;

		/// Bridge limits that the regular bridge must not exceed.
		type BridgeLimits: Get<BridgeLimits>;
		/// The penalty (in chain native tokens) that is paid from the bridge reserve to the bridge
		/// misbehavior reporter.
		#[pallet::constant]
		type Penalty: Get<BalanceOf<ThisChainOf<Self, I>>>;
		/// A way to interact with parent/sibling chains that are participating in the bridge.
		///
		/// We need to be able to suspend/resume the channel with the bridge origin chains when
		/// we receive a misbehavior reports. We also need a way to send bridge queues state report
		/// to those chains.
		type LocalXcmChannelManager: LocalXcmChannelManager;
	}

	/// An alias for the bridge metadata.
	pub type BridgeOf<T, I> = Bridge<ThisChainOf<T, I>>;
	/// An alias for the this chain.
	pub type ThisChainOf<T, I> =
		pallet_bridge_messages::ThisChainOf<T, <T as Config<I>>::BridgeMessagesPalletInstance>;
	/// An alias for the associated lanes manager.
	pub type LanesManagerOf<T, I> =
		pallet_bridge_messages::LanesManager<T, <T as Config<I>>::BridgeMessagesPalletInstance>;

	#[pallet::pallet]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::call]
	impl<T: Config<I>, I: 'static> Pallet<T, I>
	where
		T: frame_system::Config<
			AccountId = AccountIdOf<ThisChainOf<T, I>>,
			BlockNumber = BlockNumberOf<ThisChainOf<T, I>>,
		>,
		T::NativeCurrency: Currency<T::AccountId, Balance = BalanceOf<ThisChainOf<T, I>>>,
	{
		/// Open a bridge between two locations.
		///
		/// The caller must be within the `T::AllowedOpenBridgeOrigin` filter (presumably: a sibling
		/// parachain or a parent relay chain). The `bridge_destination_relative_location` must be a
		/// destination within the consensus of the `T::BridgedNetworkId` network.
		///
		/// The `BridgeReserve` amount is reserved on the caller account. This reserve
		/// is unreserved after bridge is closed.
		///
		/// The states after this call: bridge is `Opened`, outbound lane is `Opened`, inbound lane
		/// is `Opened`.
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::zero())] // TODO: https://github.com/paritytech/parity-bridges-common/issues/1760 - weights
		pub fn open_bridge(
			origin: OriginFor<T>,
			bridge_destination_relative_location: Box<VersionedMultiLocation>,
		) -> DispatchResult {
			// check and compute required bridge locations
			let locations = Self::bridge_locations(origin, bridge_destination_relative_location)?;

			// reserve balance on the parachain sovereign account
			let reserve = T::BridgeReserve::get();
			let bridge_owner_account = T::BridgeOriginAccountIdConverter::convert_location(
				&locations.bridge_origin_relative_location,
			)
			.ok_or(Error::<T, I>::InvalidBridgeOriginAccount)?;
			T::NativeCurrency::reserve(&bridge_owner_account, reserve)
				.map_err(|_| Error::<T, I>::FailedToReserveBridgeReserve)?;

			// remember that the local origin has opened given bridge
			BridgesByLocalOrigin::<T, I>::try_mutate(
				VersionedInteriorMultiLocation::from(locations.bridge_origin_universal_location),
				|storage_bridges| {
					let mut bridges = storage_bridges.to_vec();
					bridges.push(locations.lane_id);
					*storage_bridges = bridges
						.try_into()
						.map_err(|_| Error::<T, I>::TooManyBridgesForLocalOrigin)?;
					Ok::<(), Error<T, I>>(())
				},
			)?;

			// save bridge metadata
			Bridges::<T, I>::try_mutate(locations.lane_id, |bridge| match bridge {
				Some(_) => Err(Error::<T, I>::BridgeAlreadyExists),
				None => {
					*bridge = Some(BridgeOf::<T, I> {
						bridge_origin_relative_location: Box::new(
							locations.bridge_origin_relative_location.into(),
						),
						state: BridgeState::Opened,
						bridge_owner_account,
						reserve,
					});
					Ok(())
				},
			})?;

			// create new lanes. Under normal circumstances, following calls shall never fail
			let lanes_manager = LanesManagerOf::<T, I>::new();
			lanes_manager
				.create_inbound_lane(locations.lane_id)
				.map_err(Error::<T, I>::LanesManager)?;
			lanes_manager
				.create_outbound_lane(locations.lane_id)
				.map_err(Error::<T, I>::LanesManager)?;

			// write something to log
			log::trace!(
				target: LOG_TARGET,
				"Bridge {:?} between {:?} and {:?} has been opened",
				locations.lane_id,
				locations.bridge_origin_universal_location,
				locations.bridge_destination_universal_location,
			);

			// deposit `BridgeOpened` event
			Self::deposit_event(Event::<T, I>::BridgeOpened {
				lane_id: locations.lane_id,
				local_endpoint: Box::new(locations.bridge_origin_universal_location),
				remote_endpoint: Box::new(locations.bridge_destination_universal_location),
			});

			Ok(())
		}

		/// Try to close the bridge.
		///
		/// Can only be called by the "owner" of this side of the bridge, meaning that the
		/// inbound XCM channel with the local origin chain is working. So the bridge can't
		/// be in the misbehaving state.
		///
		/// Closed bridge is a bridge without any traces in the runtime storage. So this method
		/// first tries to prune all queued messages at the outbound lane. When there are no
		/// outbound messages left, outbound and inbound lanes are purged. After that, funds
		/// are returned back to the owner of this side of the bridge.
		///
		/// The number of messages that we may prune in a single call is limited by the
		/// `may_prune_messages` argument. If there are more messages in the queue, the method
		/// prunes exactly `may_prune_messages` and exits early. The caller may call it again
		/// until outbound queue is depleted and get his funds back.
		///
		/// The states after this call: everything is either `Closed`, or purged from the
		/// runtime storage.
		#[pallet::call_index(1)]
		#[pallet::weight(Weight::zero())] // TODO: https://github.com/paritytech/parity-bridges-common/issues/1760 - weights
		pub fn close_bridge(
			origin: OriginFor<T>,
			bridge_destination_relative_location: Box<VersionedMultiLocation>,
			may_prune_messages: MessageNonce,
		) -> DispatchResult {
			// compute required bridge locations
			let locations = Self::bridge_locations(origin, bridge_destination_relative_location)?;

			// TODO: https://github.com/paritytech/parity-bridges-common/issues/1760 - may do refund here, if
			// bridge/lanes are already closed + for messages that are not pruned

			// update bridge metadata - this also guarantees that the bridge is in the proper state
			let bridge =
				Bridges::<T, I>::try_mutate_exists(locations.lane_id, |bridge| match bridge {
					Some(bridge) => {
						// just double check - normally the inbound XCM channel will be closed for
						// misbehaving bridge, so he won't be able to call the `close_bridge`
						ensure!(
							bridge.state != BridgeState::Misbehaving,
							Error::<T, I>::CannotCloseMisbehavingBridge,
						);
						bridge.state = BridgeState::Closed;
						Ok(bridge.clone())
					},
					None => Err(Error::<T, I>::UnknownBridge),
				})?;

			// close inbound and outbound lanes
			let lanes_manager = LanesManagerOf::<T, I>::new();
			let mut inbound_lane = lanes_manager
				.any_state_inbound_lane(locations.lane_id)
				.map_err(Error::<T, I>::LanesManager)?;
			let mut outbound_lane = lanes_manager
				.any_state_outbound_lane(locations.lane_id)
				.map_err(Error::<T, I>::LanesManager)?;

			// now prune queued messages
			let mut pruned_messages = 0;
			for _ in outbound_lane.queued_messages() {
				if pruned_messages == may_prune_messages {
					break
				}

				outbound_lane.remove_oldest_unpruned_message();
				pruned_messages += 1;
			}

			// if there are outbound messages in the queue, just update states and early exit
			if !outbound_lane.queued_messages().is_empty() {
				// update lanes state. Under normal circumstances, following calls shall never fail
				inbound_lane.set_state(LaneState::Closed);
				outbound_lane.set_state(LaneState::Closed);

				// write something to log
				let enqueued_messages = outbound_lane.queued_messages().checked_len().unwrap_or(0);
				log::trace!(
					target: LOG_TARGET,
					"Bridge {:?} between {:?} and {:?} is closing. {} messages remaining",
					locations.lane_id,
					locations.bridge_origin_universal_location,
					locations.bridge_destination_universal_location,
					enqueued_messages,
				);

				// deposit the `ClosingBridge` event
				Self::deposit_event(Event::<T, I>::ClosingBridge {
					lane_id: locations.lane_id,
					pruned_messages,
					enqueued_messages,
				});

				return Ok(())
			}

			// else we have pruned all messages, so lanes and the bridge itself may gone
			inbound_lane.purge();
			outbound_lane.purge();
			Bridges::<T, I>::remove(locations.lane_id);
			BridgesByLocalOrigin::<T, I>::try_mutate(
				VersionedInteriorMultiLocation::from(locations.bridge_origin_universal_location),
				|storage_bridges| {
					let mut bridges = storage_bridges.to_vec();
					bridges.retain(|b| *b != locations.lane_id);
					*storage_bridges = bridges
						.try_into()
						.map_err(|_| Error::<T, I>::TooManyBridgesForLocalOrigin)?;
					Ok::<(), Error<T, I>>(())
				},
			)?;

			// unreserve remaining amount
			let failed_to_unreserve =
				T::NativeCurrency::unreserve(&bridge.bridge_owner_account, bridge.reserve);
			if !failed_to_unreserve.is_zero() {
				// we can't do anything here - looks like funds have been (partially) unreserved
				// before by someone else. Let's not fail, though - it'll be worse for the caller
				log::trace!(
					target: LOG_TARGET,
					"Failed to unreserve {:?} during ridge {:?} closure",
					failed_to_unreserve,
					locations.lane_id,
				);
			}

			// write something to log
			log::trace!(
				target: LOG_TARGET,
				"Bridge {:?} between {:?} and {:?} has been closed",
				locations.lane_id,
				locations.bridge_origin_universal_location,
				locations.bridge_destination_universal_location,
			);

			// deposit the `BridgePruned` event
			Self::deposit_event(Event::<T, I>::BridgePruned {
				lane_id: locations.lane_id,
				pruned_messages,
			});

			Ok(())
		}

		/// Report bridge misbehavior.
		///
		/// The origin must be signed. If misbehavior is confirmed, some (`Penalty`) portion of
		/// bridge reserve is transferred to the reporter. The inbound XCM channel between local
		/// origin chain and the bridge hub is suspended and the outbound lane of the bridge is
		/// immediately closed.
		///
		/// The bridge owner must fix the misbehavior by processing the corresponding bridge queue.
		/// After that, he may call (using some associated account) the `resume_misbehaving_bridges`
		/// to return to regular bridge operations.
		#[pallet::call_index(2)]
		#[pallet::weight(Weight::zero())] // TODO: https://github.com/paritytech/parity-bridges-common/issues/1760 - weights
		pub fn report_misbehavior(
			origin: OriginFor<T>,
			lane_id: LaneId,
			misbehavior: BridgeMisbehavior,
		) -> DispatchResult {
			// the `reporter` may be associated with the bridge owner, so in theory `owner` may
			// report his own misbehavior and he won't actually get penalized. But:
			// 1) the bridge stops working normally && no messages are enqueued until misbehavior is
			//    resolved. So we have achieved what we wanted to, except that the owner haven't
			//    paid anything for that;
			// 2) the owner still has to deal with a risk that someone else will submit report
			//    before his own.
			//
			// We may have a smart "altruistic" reporter, who will do `resume_misbehaving_bridges` +
			// `report_misbehavior` in one batch for this case. In this case he'll pay `Penalty`,
			// but it will be reported in a next call. So he only loses the transaction fee.
			let reporter = ensure_signed(origin)?;

			// check report
			match misbehavior {
				BridgeMisbehavior::TooManyQueuedOutboundMessages => {
					let outbound_lane = LanesManagerOf::<T, I>::new()
						.active_outbound_lane(lane_id)
						.map_err(Error::<T, I>::LanesManager)?;
					let queued_messages =
						outbound_lane.queued_messages().checked_len().unwrap_or(0);
					let max_queued_messages = T::BridgeLimits::get().max_queued_outbound_messages;
					ensure!(
						queued_messages > max_queued_messages,
						Error::<T, I>::InvalidMisbehaviorReport,
					);
				},
			}

			// fine bridge owner and switch bridge to msbehaving state
			Self::on_misbehavior(reporter, lane_id)?;

			// write something to log
			log::trace!(target: LOG_TARGET, "Bridge {:?} is misbehaving: {:?}", lane_id, misbehavior);

			// deposit the `BridgeMisbehaving` event
			Self::deposit_event(Event::<T, I>::BridgeMisbehaving { lane_id, misbehavior });

			Ok(())
		}

		/// Resume misbehaving bridge operations.
		///
		/// This call acts as a confirmation that the previously reported misbehavior for all
		/// bridges, originated by the `bridge_origin_universal_location` are fixed.
		///
		/// The caller is assumed to be associated with the bridge owner, because the caller
		/// must be able to send the `Penalty` amount (per every misbehaving bridge) to the bridge
		/// owner account. We can't require calls from the bridge owner account itself, because
		/// the inbound XCM channel is presumably closed and the parent/sibling chain can't control
		/// this account.
		///
		/// The inbound channel with the owner (parent/sibling chain) is reopened during this call.
		#[pallet::call_index(3)]
		#[pallet::weight(Weight::zero())] // TODO: https://github.com/paritytech/parity-bridges-common/issues/1760 - weights
		pub fn resume_misbehaving_bridges(
			origin: OriginFor<T>,
			bridge_origin_universal_location: Box<VersionedInteriorMultiLocation>,
		) -> DispatchResult {
			let associated_owner_account = ensure_signed(origin)?;

			// get all bridges, owned by given origin
			let owner_bridges = BridgesByLocalOrigin::<T, I>::get(bridge_origin_universal_location);

			// iterate through all bridges and resume every misbehaving bridge
			let to_replenish = T::Penalty::get();
			let mut total_to_replenish: BalanceOf<ThisChainOf<T, I>> = Zero::zero();
			let mut last_bridge = None;
			for owner_bridge_id in owner_bridges {
				let bridge =
					Bridges::<T, I>::get(owner_bridge_id).ok_or(Error::<T, I>::UnknownBridge)?;
				if bridge.state != BridgeState::Misbehaving {
					continue
				}

				// update bridge metadata
				Bridges::<T, I>::mutate_extant(owner_bridge_id, |bridge| {
					// even though the the message misbehavior may have been reported during the
					// closure, let's revert to the `Opened`
					bridge.state = BridgeState::Opened;
					bridge.reserve = bridge.reserve.saturating_add(to_replenish);

					last_bridge = Some(bridge.clone());
				});

				// reopen outbound lane state
				LanesManagerOf::<T, I>::new()
					.any_state_outbound_lane(owner_bridge_id)
					.map_err(Error::<T, I>::LanesManager)?
					.set_state(LaneState::Opened);

				// remember to actually replenish the owner account
				total_to_replenish = total_to_replenish.saturating_add(to_replenish);
			}

			// if no bridges are misbehaving, that's an invalid call
			let last_bridge =
				last_bridge.map(Ok).unwrap_or(Err(Error::<T, I>::NoMisbehavingBridges))?;

			// actually replenish the owner account
			T::NativeCurrency::transfer(
				&associated_owner_account,
				&last_bridge.bridge_owner_account,
				total_to_replenish,
				ExistenceRequirement::AllowDeath,
			)?;

			// and reserve replenished amount on the bridge owner account
			T::NativeCurrency::reserve(&last_bridge.bridge_owner_account, total_to_replenish)
				.map_err(|_| Error::<T, I>::FailedToReserveBridgeReserve)?;

			// resume the inbound channel between bridge hub and the bridge origin
			T::LocalXcmChannelManager::resume_inbound_channel(Self::xcm_into_latest(
				*last_bridge.bridge_origin_relative_location,
			)?)
			.map_err(|_| Error::<T, I>::FailedToResumeInboundChannel)?;

			Ok(())
		}

		/// Report reachable bridge queues state.
		///
		/// The origin must pass the `AllowedOpenBridgeOrigin` filter. The pallet prepares
		/// the bridge queues state structure and sends it back to the caller.
		#[pallet::call_index(4)]
		#[pallet::weight(Weight::zero())] // TODO: https://github.com/paritytech/parity-bridges-common/issues/1760 - weights
		pub fn report_bridge_queues_state(
			_origin: OriginFor<T>,
			_lane_id: LaneId,
			_encoded_call_prefix: Vec<u8>,
			_encoded_call_suffix: Vec<u8>,
		) -> DispatchResult {
			// TODO: implement me in https://github.com/paritytech/parity-bridges-common/pull/2233
			// Something like:
			//
			// ```nocompile
			// let bridge_origin_relative_location = T::AllowedOpenBridgeOrigin::ensure_origin(origin)?;
			// ...
			// let state = BridgeQueuesState { .. };
			//  let encoded_call = encoded_call_prefix ++ state.encode() ++ encoded_call_suffix;
			// T::ToBridgeOriginSender::send(
			//     bridge_origin_relative_location,
			//     vec![Xcm::Transact { call: encoded_call }],
			// );
			unimplemented!("")
		}
	}

	impl<T: Config<I>, I: 'static> Pallet<T, I>
	where
		T: frame_system::Config<
			AccountId = AccountIdOf<ThisChainOf<T, I>>,
			BlockNumber = BlockNumberOf<ThisChainOf<T, I>>,
		>,
		T::NativeCurrency: Currency<T::AccountId, Balance = BalanceOf<ThisChainOf<T, I>>>,
	{
		/// Return bridge endpoint locations and dedicated lane identifier.
		pub fn bridge_locations(
			origin: OriginFor<T>,
			bridge_destination_relative_location: Box<VersionedMultiLocation>,
		) -> Result<Box<BridgeLocations>, sp_runtime::DispatchError> {
			bridge_locations(
				Box::new(T::UniversalLocation::get()),
				Box::new(T::AllowedOpenBridgeOrigin::ensure_origin(origin)?),
				Box::new(Self::xcm_into_latest(*bridge_destination_relative_location)?),
				T::BridgedNetworkId::get(),
			)
			.map_err(|e| Error::<T, I>::BridgeLocations(e).into())
		}

		/// Actions when misbehavior is detected.
		fn on_misbehavior(reporter: T::AccountId, lane_id: LaneId) -> Result<(), Error<T, I>> {
			// check bridge state - we can't take penalty from already misbehaving bridge
			let bridge = Bridges::<T, I>::get(lane_id).ok_or(Error::<T, I>::UnknownBridge)?;
			ensure!(bridge.state != BridgeState::Misbehaving, Error::<T, I>::AlreadyMisbehaving);

			// transfer penalty to the reporter
			let penalty = T::Penalty::get();
			let result = T::NativeCurrency::repatriate_reserved(
				&bridge.bridge_owner_account,
				&reporter,
				penalty,
				BalanceStatus::Free,
			);
			let updated_reserve = match result {
				Ok(failed_penalty) if failed_penalty.is_zero() => {
					log::trace!(
						target: LOG_TARGET,
						"Bridge owner account {:?} has been slashed for {:?}. Funds were deposited to {:?}",
						bridge.bridge_owner_account,
						penalty,
						reporter,
					);
					bridge.reserve.saturating_sub(penalty)
				},
				Ok(failed_penalty) => {
					log::trace!(
						target: LOG_TARGET,
						"Bridge onwer account {:?} has been partially slashed for {:?}. Funds were deposited to {:?}. \
						Failed to slash: {:?}",
						bridge.bridge_owner_account,
						penalty,
						reporter,
						failed_penalty,
					);
					bridge.reserve.saturating_sub(penalty.saturating_sub(failed_penalty))
				},
				Err(e) => {
					// it may fail if there's no beneficiary account. But since we're in the call
					// that is signed by the `reporter`, it should never happen in practice
					log::debug!(
						target: LOG_TARGET,
						"Failed to slash bridge onwer account {:?}: {:?}. Maybe reporter account doesn't exist? \
						Reporter: {:?}, amount: {:?}, failed to slash: {:?}",
						bridge.bridge_owner_account,
						e,
						reporter,
						penalty,
						penalty,
					);
					bridge.reserve
				},
			};

			// suspend the inbound channel between bridge hub and the bridge owner. This shall
			// guarantee that no more incoming XCM messages will be handled by the bridge hub until
			// misbehavior is resolved
			T::LocalXcmChannelManager::suspend_inbound_channel(Self::xcm_into_latest(
				*bridge.bridge_origin_relative_location,
			)?)
			.map_err(|_| Error::<T, I>::FailedToSuspendInboundChannel)?;

			// update bridge metadata (we know it exists, because it has been read at the beginning
			// of this method)
			Bridges::<T, I>::mutate_extant(lane_id, |bridge| {
				bridge.state = BridgeState::Misbehaving;
				bridge.reserve = updated_reserve;
			});

			// update outbound lane state. Under normal circumstances, following calls shall never
			// fail. We don't close the inbound lane - if we do, the bridged side will be unable to
			// unblock itself.
			//
			// No messages are dropped because of the closed lane, because the inbound channel has
			// been closed (`suspend_inbound_channel` call above).
			LanesManagerOf::<T, I>::new()
				.active_outbound_lane(lane_id)
				.map_err(Error::<T, I>::LanesManager)?
				.set_state(LaneState::Closed);

			Ok(())
		}

		/// Convert versioned XCM struct into latest known XCM version.
		fn xcm_into_latest<V: TryInto<U, Error = ()>, U>(versioned: V) -> Result<U, Error<T, I>> {
			versioned.try_into().map_err(|_| Error::UnsupportedXcmVersion)
		}
	}

	/// All registered bridges.
	#[pallet::storage]
	pub type Bridges<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Identity, LaneId, BridgeOf<T, I>>;

	/// All bridges opened by given local origin' universal location.
	///
	/// We're using `VersionedInteriorMultiLocation` here, but since we only allow write to this
	/// map from local calls AND we'll upgrade this map during migration to new XCM version, it
	/// shall not allow multiple bridges between the same locations.
	#[pallet::storage]
	pub type BridgesByLocalOrigin<T: Config<I>, I: 'static = ()> = StorageMap<
		_,
		Blake2_256,
		VersionedInteriorMultiLocation,
		BoundedVec<LaneId, T::MaxBridgesPerLocalOrigin>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config<I>, I: 'static = ()> {
		/// The bridge between two locations has been opened.
		BridgeOpened {
			/// Universal location of local bridge endpoint.
			local_endpoint: Box<InteriorMultiLocation>,
			/// Universal location of remote bridge endpoint.
			remote_endpoint: Box<InteriorMultiLocation>,
			/// Bridge and its lane identifier.
			lane_id: LaneId,
		},
		/// Bridge is going to be closed, but not yet fully pruned from the runtime storage.
		ClosingBridge {
			/// Bridge and its lane identifier.
			lane_id: LaneId,
			/// Number of pruned messages during the close call.
			pruned_messages: MessageNonce,
			/// Number of enqueued messages that need to be pruned in follow up calls.
			enqueued_messages: MessageNonce,
		},
		/// Bridge has been closed and pruned from the runtime storage. It now may be reopened
		/// again by any participant.
		BridgePruned {
			/// Bridge and its lane identifier.
			lane_id: LaneId,
			/// Number of pruned messages during the close call.
			pruned_messages: MessageNonce,
		},
		/// Bridge is misbehaving.
		BridgeMisbehaving {
			/// Bridge and its lane identifier.
			lane_id: LaneId,
			/// Bridge misbehavior type.
			misbehavior: BridgeMisbehavior,
		},
	}

	#[pallet::error]
	pub enum Error<T, I = ()> {
		/// Bridge locations error.
		BridgeLocations(BridgeLocationsError),
		/// Invalid local bridge origin account.
		InvalidBridgeOriginAccount,
		/// The bridge is already registered in this pallet.
		BridgeAlreadyExists,
		/// The local origin already owns a maximal number of bridges.
		TooManyBridgesForLocalOrigin,
		/// Trying to close already closed bridge.
		BridgeAlreadyClosed,
		/// Cannot close misbehaving bridge. The bridge owner should fix the misbehavior first.
		CannotCloseMisbehavingBridge,
		/// Lanes manager error.
		LanesManager(LanesManagerError),
		/// Trying to access unknown bridge.
		UnknownBridge,
		/// The bridge origin can't pay the required amount for opening the bridge.
		FailedToReserveBridgeReserve,
		/// Invalid misbehavior report has been submitted.
		InvalidMisbehaviorReport,
		/// The given bridge is already in the misbehaving state.
		AlreadyMisbehaving,
		/// Failed to suspend the inbound channel with the parent/sibling bridge origin.
		FailedToSuspendInboundChannel,
		/// There are no misbehaving bridges to be resumed.
		NoMisbehavingBridges,
		/// Failed to resume the inbound channel with the parent/sibling bridge origin.
		FailedToResumeInboundChannel,
		/// The version of XCM location argument is unsupported.
		UnsupportedXcmVersion,
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use mock::*;

	use frame_support::{assert_noop, assert_ok, traits::fungible::Mutate, BoundedVec};
	use frame_system::{EventRecord, Phase};
	use sp_runtime::{DispatchError, TokenError};

	fn fund_origin_sovereign_account(locations: &BridgeLocations, balance: Balance) -> AccountId {
		let bridge_owner_account =
			LocationToAccountId::convert_location(&locations.bridge_origin_relative_location)
				.unwrap();
		Balances::mint_into(&bridge_owner_account, balance).unwrap();
		bridge_owner_account
	}

	fn mock_open_bridge_from_with(
		origin: RuntimeOrigin,
		with: MultiLocation,
	) -> (BridgeOf<TestRuntime, ()>, BridgeLocations) {
		let reserve = BridgeReserve::get();
		let locations = XcmOverBridge::bridge_locations(origin, Box::new(with.into())).unwrap();
		let bridge_owner_account =
			fund_origin_sovereign_account(&locations, reserve + ExistentialDeposit::get());
		Balances::reserve(&bridge_owner_account, reserve).unwrap();

		let bridge = Bridge {
			bridge_origin_relative_location: Box::new(
				locations.bridge_origin_relative_location.into(),
			),
			state: BridgeState::Opened,
			bridge_owner_account,
			reserve,
		};
		Bridges::<TestRuntime, ()>::insert(locations.lane_id, bridge.clone());
		BridgesByLocalOrigin::<TestRuntime, ()>::mutate(
			VersionedInteriorMultiLocation::from(locations.bridge_origin_universal_location),
			|storage_bridges| {
				let mut bridges = storage_bridges.to_vec();
				bridges.push(locations.lane_id);
				*storage_bridges = BoundedVec::try_from(bridges).unwrap();
			},
		);

		let lanes_manager = LanesManagerOf::<TestRuntime, ()>::new();
		lanes_manager.create_inbound_lane(locations.lane_id).unwrap();
		lanes_manager.create_outbound_lane(locations.lane_id).unwrap();

		(bridge, *locations)
	}

	fn mock_open_bridge_from(
		origin: RuntimeOrigin,
	) -> (BridgeOf<TestRuntime, ()>, BridgeLocations) {
		mock_open_bridge_from_with(origin, bridged_asset_hub_location())
	}

	fn enqueue_message(lane: LaneId) {
		let lanes_manager = LanesManagerOf::<TestRuntime, ()>::new();
		lanes_manager
			.active_outbound_lane(lane)
			.unwrap()
			.send_message(vec![42])
			.unwrap();
	}

	fn ensure_misbehavior_state(
		reporter: AccountId,
		locations: BridgeLocations,
		free_balance: Balance,
		reserved_balance: Balance,
		total_reward: Balance,
	) {
		// check that the bridge and its lanes are in the Closed state
		let lanes_manager = LanesManagerOf::<TestRuntime, ()>::new();
		assert_eq!(
			Bridges::<TestRuntime, ()>::get(locations.lane_id).map(|b| b.state),
			Some(BridgeState::Misbehaving)
		);
		assert_eq!(
			lanes_manager.any_state_inbound_lane(locations.lane_id).unwrap().state(),
			LaneState::Opened
		);
		assert_eq!(
			lanes_manager.any_state_outbound_lane(locations.lane_id).unwrap().state(),
			LaneState::Closed
		);

		// check that reporter balance is increased, bridge owner balance is decreased and
		// we have saved this fact in the bridge meta
		let bridge = Bridges::<TestRuntime, ()>::get(locations.lane_id).unwrap();
		assert_eq!(Balances::free_balance(&bridge.bridge_owner_account), free_balance);
		assert_eq!(
			Balances::reserved_balance(&bridge.bridge_owner_account),
			reserved_balance - Penalty::get()
		);
		assert_eq!(Balances::free_balance(&reporter), ExistentialDeposit::get() + total_reward);
		assert_eq!(Balances::reserved_balance(&reporter), 0);
		assert_eq!(
			Bridges::<TestRuntime, ()>::get(locations.lane_id).map(|b| b.reserve),
			Some(BridgeReserve::get() - Penalty::get())
		);

		assert!(TestLocalXcmChannelManager::is_inbound_channel_suspended(
			locations.bridge_origin_relative_location
		));
	}

	#[test]
	fn open_bridge_fails_if_origin_is_not_allowed() {
		run_test(|| {
			assert_noop!(
				XcmOverBridge::open_bridge(
					AllowedOpenBridgeOrigin::disallowed_origin(),
					Box::new(bridged_asset_hub_location().into()),
				),
				sp_runtime::DispatchError::BadOrigin,
			);
		})
	}

	#[test]
	fn open_bridge_fails_if_origin_is_not_relative() {
		run_test(|| {
			assert_noop!(
				XcmOverBridge::open_bridge(
					AllowedOpenBridgeOrigin::parent_relay_chain_universal_origin(),
					Box::new(bridged_asset_hub_location().into()),
				),
				Error::<TestRuntime, ()>::BridgeLocations(
					BridgeLocationsError::InvalidBridgeOrigin
				),
			);

			assert_noop!(
				XcmOverBridge::open_bridge(
					AllowedOpenBridgeOrigin::sibling_parachain_universal_origin(),
					Box::new(bridged_asset_hub_location().into()),
				),
				Error::<TestRuntime, ()>::BridgeLocations(
					BridgeLocationsError::InvalidBridgeOrigin
				),
			);
		})
	}

	#[test]
	fn open_bridge_fails_if_destination_is_not_remote() {
		run_test(|| {
			assert_noop!(
				XcmOverBridge::open_bridge(
					AllowedOpenBridgeOrigin::parent_relay_chain_origin(),
					Box::new(
						MultiLocation {
							parents: 2,
							interior: X2(
								GlobalConsensus(RelayNetwork::get()),
								Parachain(BRIDGED_ASSET_HUB_ID)
							)
						}
						.into()
					),
				),
				Error::<TestRuntime, ()>::BridgeLocations(BridgeLocationsError::DestinationIsLocal),
			);
		});
	}

	#[test]
	fn open_bridge_fails_if_outside_of_bridged_consensus() {
		run_test(|| {
			assert_noop!(
				XcmOverBridge::open_bridge(
					AllowedOpenBridgeOrigin::parent_relay_chain_origin(),
					Box::new(
						MultiLocation {
							parents: 2,
							interior: X2(
								GlobalConsensus(NonBridgedRelayNetwork::get()),
								Parachain(BRIDGED_ASSET_HUB_ID)
							)
						}
						.into()
					),
				),
				Error::<TestRuntime, ()>::BridgeLocations(
					BridgeLocationsError::UnreachableDestination
				),
			);
		});
	}

	#[test]
	fn open_bridge_fails_if_origin_has_no_sovereign_account() {
		run_test(|| {
			assert_noop!(
				XcmOverBridge::open_bridge(
					AllowedOpenBridgeOrigin::origin_without_sovereign_account(),
					Box::new(bridged_asset_hub_location().into()),
				),
				Error::<TestRuntime, ()>::InvalidBridgeOriginAccount,
			);
		});
	}

	#[test]
	fn open_bridge_fails_if_origin_sovereign_account_has_no_enough_funds() {
		run_test(|| {
			assert_noop!(
				XcmOverBridge::open_bridge(
					AllowedOpenBridgeOrigin::parent_relay_chain_origin(),
					Box::new(bridged_asset_hub_location().into()),
				),
				Error::<TestRuntime, ()>::FailedToReserveBridgeReserve,
			);
		});
	}

	#[test]
	fn open_bridge_fails_if_origin_has_reached_bridges_limit() {
		run_test(|| {
			let origin = AllowedOpenBridgeOrigin::parent_relay_chain_origin();
			let locations = XcmOverBridge::bridge_locations(
				origin.clone(),
				Box::new(bridged_asset_hub_location().into()),
			)
			.unwrap();
			fund_origin_sovereign_account(
				&locations,
				BridgeReserve::get() + ExistentialDeposit::get(),
			);

			BridgesByLocalOrigin::<TestRuntime, ()>::insert(
				VersionedInteriorMultiLocation::from(locations.bridge_origin_universal_location),
				BoundedVec::try_from(vec![
					LaneId::new(1, 2);
					MaxBridgesPerLocalOrigin::get() as usize
				])
				.unwrap(),
			);

			assert_noop!(
				XcmOverBridge::open_bridge(origin, Box::new(bridged_asset_hub_location().into()),),
				Error::<TestRuntime, ()>::TooManyBridgesForLocalOrigin,
			);
		})
	}

	#[test]
	fn open_bridge_fails_if_it_already_exists() {
		run_test(|| {
			let origin = AllowedOpenBridgeOrigin::parent_relay_chain_origin();
			let locations = XcmOverBridge::bridge_locations(
				origin.clone(),
				Box::new(bridged_asset_hub_location().into()),
			)
			.unwrap();
			fund_origin_sovereign_account(
				&locations,
				BridgeReserve::get() + ExistentialDeposit::get(),
			);

			Bridges::<TestRuntime, ()>::insert(
				locations.lane_id,
				Bridge {
					bridge_origin_relative_location: Box::new(
						locations.bridge_origin_relative_location.into(),
					),
					state: BridgeState::Opened,
					bridge_owner_account: [0u8; 32].into(),
					reserve: 0,
				},
			);

			assert_noop!(
				XcmOverBridge::open_bridge(origin, Box::new(bridged_asset_hub_location().into()),),
				Error::<TestRuntime, ()>::BridgeAlreadyExists,
			);
		})
	}

	#[test]
	fn open_bridge_fails_if_its_lanes_already_exists() {
		run_test(|| {
			let origin = AllowedOpenBridgeOrigin::parent_relay_chain_origin();
			let locations = XcmOverBridge::bridge_locations(
				origin.clone(),
				Box::new(bridged_asset_hub_location().into()),
			)
			.unwrap();
			fund_origin_sovereign_account(
				&locations,
				BridgeReserve::get() + ExistentialDeposit::get(),
			);

			let lanes_manager = LanesManagerOf::<TestRuntime, ()>::new();

			lanes_manager.create_inbound_lane(locations.lane_id).unwrap();
			assert_noop!(
				XcmOverBridge::open_bridge(
					origin.clone(),
					Box::new(bridged_asset_hub_location().into()),
				),
				Error::<TestRuntime, ()>::LanesManager(LanesManagerError::InboundLaneAlreadyExists),
			);

			lanes_manager.active_inbound_lane(locations.lane_id).unwrap().purge();
			lanes_manager.create_outbound_lane(locations.lane_id).unwrap();
			assert_noop!(
				XcmOverBridge::open_bridge(origin, Box::new(bridged_asset_hub_location().into()),),
				Error::<TestRuntime, ()>::LanesManager(
					LanesManagerError::OutboundLaneAlreadyExists
				),
			);
		})
	}

	#[test]
	fn open_bridge_works() {
		run_test(|| {
			// in our test runtime, we expect that bridge may be opened by parent relay chain
			// and any sibling parachain
			let origins = [
				AllowedOpenBridgeOrigin::parent_relay_chain_origin(),
				AllowedOpenBridgeOrigin::sibling_parachain_origin(),
			];

			// check that every origin may open the bridge
			let lanes_manager = LanesManagerOf::<TestRuntime, ()>::new();
			let expected_reserve = BridgeReserve::get();
			let existential_deposit = ExistentialDeposit::get();
			for origin in origins {
				// reset events
				System::set_block_number(1);
				System::reset_events();

				// compute all other locations
				let locations = XcmOverBridge::bridge_locations(
					origin.clone(),
					Box::new(bridged_asset_hub_location().into()),
				)
				.unwrap();

				// ensure that there's no bridge and lanes in the storage
				assert_eq!(Bridges::<TestRuntime, ()>::get(locations.lane_id), None);
				assert_eq!(
					lanes_manager.active_inbound_lane(locations.lane_id).map(drop),
					Err(LanesManagerError::UnknownInboundLane)
				);
				assert_eq!(
					lanes_manager.active_outbound_lane(locations.lane_id).map(drop),
					Err(LanesManagerError::UnknownOutboundLane)
				);

				// give enough funds to the sovereign account of the bridge origin
				let bridge_owner_account = fund_origin_sovereign_account(
					&locations,
					expected_reserve + existential_deposit,
				);
				assert_eq!(
					Balances::free_balance(&bridge_owner_account),
					expected_reserve + existential_deposit
				);
				assert_eq!(Balances::reserved_balance(&bridge_owner_account), 0);

				// now open the bridge
				assert_ok!(XcmOverBridge::open_bridge(
					origin,
					Box::new(locations.bridge_destination_relative_location.into()),
				));

				// ensure that everything has been set up in the runtime storage
				assert_eq!(
					BridgesByLocalOrigin::<TestRuntime, ()>::get(Box::new(
						locations.bridge_origin_universal_location.into()
					))
					.to_vec(),
					vec![locations.lane_id],
				);
				assert_eq!(
					Bridges::<TestRuntime, ()>::get(locations.lane_id),
					Some(Bridge {
						bridge_origin_relative_location: Box::new(
							locations.bridge_origin_relative_location.into()
						),
						state: BridgeState::Opened,
						bridge_owner_account: bridge_owner_account.clone(),
						reserve: expected_reserve,
					}),
				);
				assert_eq!(
					lanes_manager.active_inbound_lane(locations.lane_id).map(|l| l.state()),
					Ok(LaneState::Opened)
				);
				assert_eq!(
					lanes_manager.active_outbound_lane(locations.lane_id).map(|l| l.state()),
					Ok(LaneState::Opened)
				);
				assert_eq!(Balances::free_balance(&bridge_owner_account), existential_deposit);
				assert_eq!(Balances::reserved_balance(&bridge_owner_account), expected_reserve);

				// ensure that the proper event is deposited
				assert_eq!(
					System::events().last(),
					Some(&EventRecord {
						phase: Phase::Initialization,
						event: RuntimeEvent::XcmOverBridge(Event::BridgeOpened {
							lane_id: locations.lane_id,
							local_endpoint: Box::new(locations.bridge_origin_universal_location),
							remote_endpoint: Box::new(
								locations.bridge_destination_universal_location
							),
						}),
						topics: vec![],
					}),
				);
			}
		});
	}

	#[test]
	fn close_bridge_fails_if_bridge_is_misbehaving() {
		run_test(|| {
			let origin = AllowedOpenBridgeOrigin::parent_relay_chain_origin();
			let (_, locations) = mock_open_bridge_from(origin.clone());

			Bridges::<TestRuntime, ()>::mutate_extant(locations.lane_id, |bridge| {
				bridge.state = BridgeState::Misbehaving;
			});

			assert_noop!(
				XcmOverBridge::close_bridge(
					origin,
					Box::new(bridged_asset_hub_location().into()),
					0,
				),
				Error::<TestRuntime, ()>::CannotCloseMisbehavingBridge,
			);
		})
	}

	#[test]
	fn close_bridge_fails_if_origin_is_not_allowed() {
		run_test(|| {
			assert_noop!(
				XcmOverBridge::close_bridge(
					AllowedOpenBridgeOrigin::disallowed_origin(),
					Box::new(bridged_asset_hub_location().into()),
					0,
				),
				sp_runtime::DispatchError::BadOrigin,
			);
		})
	}

	#[test]
	fn close_bridge_fails_if_origin_is_not_relative() {
		run_test(|| {
			assert_noop!(
				XcmOverBridge::close_bridge(
					AllowedOpenBridgeOrigin::parent_relay_chain_universal_origin(),
					Box::new(bridged_asset_hub_location().into()),
					0,
				),
				Error::<TestRuntime, ()>::BridgeLocations(
					BridgeLocationsError::InvalidBridgeOrigin
				),
			);

			assert_noop!(
				XcmOverBridge::close_bridge(
					AllowedOpenBridgeOrigin::sibling_parachain_universal_origin(),
					Box::new(bridged_asset_hub_location().into()),
					0,
				),
				Error::<TestRuntime, ()>::BridgeLocations(
					BridgeLocationsError::InvalidBridgeOrigin
				),
			);
		})
	}

	#[test]
	fn close_bridge_fails_if_its_lanes_are_unknown() {
		run_test(|| {
			let origin = AllowedOpenBridgeOrigin::parent_relay_chain_origin();
			let (_, locations) = mock_open_bridge_from(origin.clone());

			let lanes_manager = LanesManagerOf::<TestRuntime, ()>::new();
			lanes_manager.any_state_inbound_lane(locations.lane_id).unwrap().purge();
			assert_noop!(
				XcmOverBridge::close_bridge(
					origin.clone(),
					Box::new(locations.bridge_destination_relative_location.into()),
					0,
				),
				Error::<TestRuntime, ()>::LanesManager(LanesManagerError::UnknownInboundLane),
			);
			lanes_manager.any_state_outbound_lane(locations.lane_id).unwrap().purge();

			let (_, locations) = mock_open_bridge_from(origin.clone());
			lanes_manager.any_state_outbound_lane(locations.lane_id).unwrap().purge();
			assert_noop!(
				XcmOverBridge::close_bridge(
					origin,
					Box::new(locations.bridge_destination_relative_location.into()),
					0,
				),
				Error::<TestRuntime, ()>::LanesManager(LanesManagerError::UnknownOutboundLane),
			);
		});
	}

	#[test]
	fn close_bridge_works() {
		run_test(|| {
			let origin = AllowedOpenBridgeOrigin::parent_relay_chain_origin();
			let (bridge, locations) = mock_open_bridge_from(origin.clone());
			System::set_block_number(1);

			// remember owner balances
			let free_balance = Balances::free_balance(&bridge.bridge_owner_account);
			let reserved_balance = Balances::reserved_balance(&bridge.bridge_owner_account);

			// enqueue some messages
			for _ in 0..32 {
				enqueue_message(locations.lane_id);
			}

			// now call the `close_bridge`, which will only partially prune messages
			assert_ok!(XcmOverBridge::close_bridge(
				origin.clone(),
				Box::new(locations.bridge_destination_relative_location.into()),
				16,
			),);

			// as a result, the bridge and lanes are switched to the `Closed` state, some messages
			// are pruned, but funds are not unreserved
			let lanes_manager = LanesManagerOf::<TestRuntime, ()>::new();
			assert_eq!(
				BridgesByLocalOrigin::<TestRuntime, ()>::get(Box::new(
					locations.bridge_origin_universal_location.into()
				))
				.to_vec(),
				vec![locations.lane_id]
			);
			assert_eq!(
				Bridges::<TestRuntime, ()>::get(locations.lane_id).map(|b| b.state),
				Some(BridgeState::Closed)
			);
			assert_eq!(
				lanes_manager.any_state_inbound_lane(locations.lane_id).unwrap().state(),
				LaneState::Closed
			);
			assert_eq!(
				lanes_manager.any_state_outbound_lane(locations.lane_id).unwrap().state(),
				LaneState::Closed
			);
			assert_eq!(
				lanes_manager
					.any_state_outbound_lane(locations.lane_id)
					.unwrap()
					.queued_messages()
					.checked_len(),
				Some(16)
			);
			assert_eq!(Balances::free_balance(&bridge.bridge_owner_account), free_balance);
			assert_eq!(Balances::reserved_balance(&bridge.bridge_owner_account), reserved_balance);
			assert_eq!(
				System::events().last(),
				Some(&EventRecord {
					phase: Phase::Initialization,
					event: RuntimeEvent::XcmOverBridge(Event::ClosingBridge {
						lane_id: locations.lane_id,
						pruned_messages: 16,
						enqueued_messages: 16,
					}),
					topics: vec![],
				}),
			);

			// now call the `close_bridge` again, which will only partially prune messages
			assert_ok!(XcmOverBridge::close_bridge(
				origin.clone(),
				Box::new(locations.bridge_destination_relative_location.into()),
				8,
			),);

			// nothing is changed (apart from the pruned messages)
			assert_eq!(
				BridgesByLocalOrigin::<TestRuntime, ()>::get(Box::new(
					locations.bridge_origin_universal_location.into()
				))
				.to_vec(),
				vec![locations.lane_id]
			);
			assert_eq!(
				Bridges::<TestRuntime, ()>::get(locations.lane_id).map(|b| b.state),
				Some(BridgeState::Closed)
			);
			assert_eq!(
				lanes_manager.any_state_inbound_lane(locations.lane_id).unwrap().state(),
				LaneState::Closed
			);
			assert_eq!(
				lanes_manager.any_state_outbound_lane(locations.lane_id).unwrap().state(),
				LaneState::Closed
			);
			assert_eq!(
				lanes_manager
					.any_state_outbound_lane(locations.lane_id)
					.unwrap()
					.queued_messages()
					.checked_len(),
				Some(8)
			);
			assert_eq!(Balances::free_balance(&bridge.bridge_owner_account), free_balance);
			assert_eq!(Balances::reserved_balance(&bridge.bridge_owner_account), reserved_balance);
			assert_eq!(
				System::events().last(),
				Some(&EventRecord {
					phase: Phase::Initialization,
					event: RuntimeEvent::XcmOverBridge(Event::ClosingBridge {
						lane_id: locations.lane_id,
						pruned_messages: 8,
						enqueued_messages: 8,
					}),
					topics: vec![],
				}),
			);

			// now call the `close_bridge` again that will prune all remaining messages and the
			// bridge
			assert_ok!(XcmOverBridge::close_bridge(
				origin,
				Box::new(locations.bridge_destination_relative_location.into()),
				9,
			),);

			// there's no traces of bridge in the runtime storage and funds are unreserved
			assert_eq!(
				BridgesByLocalOrigin::<TestRuntime, ()>::get(Box::new(
					locations.bridge_origin_universal_location.into()
				))
				.to_vec(),
				vec![]
			);
			assert_eq!(Bridges::<TestRuntime, ()>::get(locations.lane_id).map(|b| b.state), None);
			assert_eq!(
				lanes_manager.any_state_inbound_lane(locations.lane_id).map(drop),
				Err(LanesManagerError::UnknownInboundLane)
			);
			assert_eq!(
				lanes_manager.any_state_outbound_lane(locations.lane_id).map(drop),
				Err(LanesManagerError::UnknownOutboundLane)
			);
			assert_eq!(
				Balances::free_balance(&bridge.bridge_owner_account),
				free_balance + reserved_balance
			);
			assert_eq!(Balances::reserved_balance(&bridge.bridge_owner_account), 0);
			assert_eq!(
				System::events().last(),
				Some(&EventRecord {
					phase: Phase::Initialization,
					event: RuntimeEvent::XcmOverBridge(Event::BridgePruned {
						lane_id: locations.lane_id,
						pruned_messages: 8,
					}),
					topics: vec![],
				}),
			);
		});
	}

	#[test]
	fn report_misbehavior_fails_if_origin_is_not_signed() {
		run_test(|| {
			assert_noop!(
				XcmOverBridge::report_misbehavior(
					RuntimeOrigin::root(),
					LaneId::new(1, 2),
					BridgeMisbehavior::TooManyQueuedOutboundMessages,
				),
				sp_runtime::DispatchError::BadOrigin,
			);
		})
	}

	#[test]
	fn report_misbehavior_fails_for_unknown_bridge() {
		run_test(|| {
			let origin = AllowedOpenBridgeOrigin::parent_relay_chain_origin();
			let (_, locations) = mock_open_bridge_from(origin);

			// report valid misbehavior fails, because the bridge is not registered
			for _ in 0..=TestBridgeLimits::get().max_queued_outbound_messages {
				enqueue_message(locations.lane_id);
			}

			Bridges::<TestRuntime, ()>::remove(locations.lane_id);
			assert_noop!(
				XcmOverBridge::report_misbehavior(
					RuntimeOrigin::signed([128u8; 32].into()),
					locations.lane_id,
					BridgeMisbehavior::TooManyQueuedOutboundMessages,
				),
				Error::<TestRuntime, ()>::UnknownBridge,
			);
		});
	}

	#[test]
	fn report_misbehavior_fails_for_misbehaving_bridge() {
		run_test(|| {
			let origin = AllowedOpenBridgeOrigin::parent_relay_chain_origin();
			let (_, locations) = mock_open_bridge_from(origin);

			// report valid misbehavior fails, because the bridge is not registered
			for _ in 0..=TestBridgeLimits::get().max_queued_outbound_messages {
				enqueue_message(locations.lane_id);
			}

			Bridges::<TestRuntime, ()>::mutate_extant(locations.lane_id, |bridge| {
				bridge.state = BridgeState::Misbehaving;
			});
			assert_noop!(
				XcmOverBridge::report_misbehavior(
					RuntimeOrigin::signed([128u8; 32].into()),
					locations.lane_id,
					BridgeMisbehavior::TooManyQueuedOutboundMessages,
				),
				Error::<TestRuntime, ()>::AlreadyMisbehaving,
			);
		});
	}

	#[test]
	fn report_misbehavior_allowed_for_closed_bridges() {
		run_test(|| {
			let origin = AllowedOpenBridgeOrigin::parent_relay_chain_origin();
			let (_, locations) = mock_open_bridge_from(origin);

			// report valid misbehavior fails, because the bridge is not registered
			for _ in 0..=TestBridgeLimits::get().max_queued_outbound_messages {
				enqueue_message(locations.lane_id);
			}

			Bridges::<TestRuntime, ()>::mutate_extant(locations.lane_id, |bridge| {
				bridge.state = BridgeState::Closed;
			});
			assert_ok!(XcmOverBridge::report_misbehavior(
				RuntimeOrigin::signed([128u8; 32].into()),
				locations.lane_id,
				BridgeMisbehavior::TooManyQueuedOutboundMessages,
			),);
		});
	}

	#[test]
	fn report_misbehavior_fails_for_unknown_outbound_lane() {
		run_test(|| {
			let origin = AllowedOpenBridgeOrigin::parent_relay_chain_origin();
			let (_, locations) = mock_open_bridge_from(origin);

			// report valid misbehavior fails, because the outbound lane is not registered
			for _ in 0..=TestBridgeLimits::get().max_queued_outbound_messages {
				enqueue_message(locations.lane_id);
			}

			LanesManagerOf::<TestRuntime, ()>::new()
				.active_outbound_lane(locations.lane_id)
				.unwrap()
				.purge();
			assert_noop!(
				XcmOverBridge::report_misbehavior(
					RuntimeOrigin::signed([128u8; 32].into()),
					locations.lane_id,
					BridgeMisbehavior::TooManyQueuedOutboundMessages,
				),
				Error::<TestRuntime, ()>::LanesManager(LanesManagerError::UnknownOutboundLane),
			);
		});
	}

	#[test]
	fn report_misbehavior_fails_for_closed_outbound_lane() {
		run_test(|| {
			let origin = AllowedOpenBridgeOrigin::parent_relay_chain_origin();
			let (_, locations) = mock_open_bridge_from(origin);

			// report valid misbehavior fails, because the outbound lane is closed
			for _ in 0..=TestBridgeLimits::get().max_queued_outbound_messages {
				enqueue_message(locations.lane_id);
			}

			LanesManagerOf::<TestRuntime, ()>::new()
				.active_outbound_lane(locations.lane_id)
				.unwrap()
				.set_state(LaneState::Closed);
			assert_noop!(
				XcmOverBridge::report_misbehavior(
					RuntimeOrigin::signed([128u8; 32].into()),
					locations.lane_id,
					BridgeMisbehavior::TooManyQueuedOutboundMessages,
				),
				Error::<TestRuntime, ()>::LanesManager(LanesManagerError::ClosedOutboundLane),
			);
		});
	}

	#[test]
	fn report_too_many_queued_outbound_messages_fails() {
		run_test(|| {
			let origin = AllowedOpenBridgeOrigin::parent_relay_chain_origin();
			let (_, locations) = mock_open_bridge_from(origin);

			for _ in 0..TestBridgeLimits::get().max_queued_outbound_messages {
				enqueue_message(locations.lane_id);
				assert_noop!(
					XcmOverBridge::report_misbehavior(
						RuntimeOrigin::signed([128u8; 32].into()),
						locations.lane_id,
						BridgeMisbehavior::TooManyQueuedOutboundMessages,
					),
					Error::<TestRuntime, ()>::InvalidMisbehaviorReport,
				);
			}
		})
	}

	#[test]
	fn report_too_many_queued_outbound_messages_works() {
		run_test(|| {
			let origin = AllowedOpenBridgeOrigin::parent_relay_chain_origin();
			let (bridge, locations) = mock_open_bridge_from(origin);
			System::set_block_number(1);

			// remember owner and reporter balances
			let reporter: AccountId = [128u8; 32].into();
			let free_balance = Balances::free_balance(&bridge.bridge_owner_account);
			let reserved_balance = Balances::reserved_balance(&bridge.bridge_owner_account);
			Balances::mint_into(&reporter, ExistentialDeposit::get()).unwrap();

			// enqueue enough messages to get penalized
			for _ in 0..=TestBridgeLimits::get().max_queued_outbound_messages {
				enqueue_message(locations.lane_id);
			}

			// report misbehavior
			assert_ok!(XcmOverBridge::report_misbehavior(
				RuntimeOrigin::signed(reporter.clone()),
				locations.lane_id,
				BridgeMisbehavior::TooManyQueuedOutboundMessages,
			),);

			// ensure that the bridge is in the proper state
			ensure_misbehavior_state(
				reporter,
				locations.clone(),
				free_balance,
				reserved_balance,
				Penalty::get(),
			);

			// ensure that the proper event is deposited
			assert_eq!(
				System::events().last(),
				Some(&EventRecord {
					phase: Phase::Initialization,
					event: RuntimeEvent::XcmOverBridge(Event::BridgeMisbehaving {
						lane_id: locations.lane_id,
						misbehavior: BridgeMisbehavior::TooManyQueuedOutboundMessages,
					}),
					topics: vec![],
				}),
			);
		})
	}

	#[test]
	fn resume_misbehaving_bridges_fails_if_origin_is_not_signed() {
		run_test(|| {
			assert_noop!(
				XcmOverBridge::resume_misbehaving_bridges(
					RuntimeOrigin::root(),
					Box::new(parent_relay_chain_universal_location().into()),
				),
				sp_runtime::DispatchError::BadOrigin,
			);
		})
	}

	#[test]
	fn resume_misbehaving_bridges_fails_if_bridge_is_unknown() {
		run_test(|| {
			BridgesByLocalOrigin::<TestRuntime, ()>::insert(
				VersionedInteriorMultiLocation::from(parent_relay_chain_universal_location()),
				BoundedVec::try_from(vec![LaneId::new(1, 2)]).unwrap(),
			);

			assert_noop!(
				XcmOverBridge::resume_misbehaving_bridges(
					RuntimeOrigin::signed([42u8; 32].into()),
					Box::new(parent_relay_chain_universal_location().into()),
				),
				Error::<TestRuntime, ()>::UnknownBridge,
			);
		})
	}

	#[test]
	fn resume_misbehaving_bridges_fails_if_outbound_lane_is_unknown() {
		run_test(|| {
			let (_, locations) =
				mock_open_bridge_from(AllowedOpenBridgeOrigin::parent_relay_chain_origin());
			Bridges::<TestRuntime, ()>::mutate_extant(locations.lane_id, |bridge| {
				bridge.state = BridgeState::Misbehaving;
			});

			let lanes_manager = LanesManagerOf::<TestRuntime, ()>::new();
			lanes_manager.active_outbound_lane(locations.lane_id).unwrap().purge();

			assert_noop!(
				XcmOverBridge::resume_misbehaving_bridges(
					RuntimeOrigin::signed([42u8; 32].into()),
					Box::new(parent_relay_chain_universal_location().into()),
				),
				Error::<TestRuntime, ()>::LanesManager(LanesManagerError::UnknownOutboundLane),
			);
		})
	}

	#[test]
	fn resume_misbehaving_bridges_fails_if_there_are_no_misbehaving_bridges() {
		run_test(|| {
			let (_, _) =
				mock_open_bridge_from(AllowedOpenBridgeOrigin::parent_relay_chain_origin());
			assert_noop!(
				XcmOverBridge::resume_misbehaving_bridges(
					RuntimeOrigin::signed([42u8; 32].into()),
					Box::new(parent_relay_chain_universal_location().into()),
				),
				Error::<TestRuntime, ()>::NoMisbehavingBridges,
			);
		})
	}

	#[test]
	fn resume_misbehaving_bridges_fails_if_reporter_fails_to_replenish_owner_account() {
		run_test(|| {
			let (_, locations) =
				mock_open_bridge_from(AllowedOpenBridgeOrigin::parent_relay_chain_origin());
			Bridges::<TestRuntime, ()>::mutate_extant(locations.lane_id, |bridge| {
				bridge.state = BridgeState::Misbehaving;
			});

			assert_noop!(
				XcmOverBridge::resume_misbehaving_bridges(
					RuntimeOrigin::signed([42u8; 32].into()),
					Box::new(parent_relay_chain_universal_location().into()),
				),
				DispatchError::Token(TokenError::FundsUnavailable),
			);
		})
	}

	#[test]
	fn resume_misbehaving_bridges_fails_if_it_fails_to_reopen_local_channel_with_owner() {
		// no way to actually test that, but following (successful) tests show that funds
		// are actually reserved on the bridge owner account
	}

	#[test]
	fn resume_misbehaving_bridges_works_with_single_misbehaving_bridge() {
		run_test(|| {
			let origin = AllowedOpenBridgeOrigin::parent_relay_chain_origin();
			let (_, locations) =
				mock_open_bridge_from_with(origin.clone(), bridged_parachain_location());
			let (misbehaving_bridge, misbehaving_locations) = mock_open_bridge_from(origin);
			Bridges::<TestRuntime, ()>::mutate_extant(misbehaving_locations.lane_id, |bridge| {
				bridge.state = BridgeState::Misbehaving;
			});
			let lanes_manager = LanesManagerOf::<TestRuntime, ()>::new();
			lanes_manager
				.active_outbound_lane(misbehaving_locations.lane_id)
				.unwrap()
				.set_state(LaneState::Closed);

			let associated_owner_account: AccountId = [42u8; 32].into();
			Balances::mint_into(
				&associated_owner_account,
				ExistentialDeposit::get() + Penalty::get(),
			)
			.unwrap();

			let pre_free_funds_on_owner =
				Balances::free_balance(&misbehaving_bridge.bridge_owner_account);
			let pre_reserved_funds_on_owner =
				Balances::reserved_balance(&misbehaving_bridge.bridge_owner_account);
			let pre_free_funds_on_associated = Balances::free_balance(&associated_owner_account);
			let pre_reserved_funds_on_associated =
				Balances::reserved_balance(&associated_owner_account);

			assert_ok!(XcmOverBridge::resume_misbehaving_bridges(
				RuntimeOrigin::signed(associated_owner_account.clone()),
				Box::new(parent_relay_chain_universal_location().into()),
			),);

			// ensure that the bridge and accounts are in the proper state
			assert_eq!(
				Bridges::<TestRuntime, ()>::get(locations.lane_id).map(|b| b.state),
				Some(BridgeState::Opened)
			);
			assert_eq!(
				lanes_manager.any_state_outbound_lane(locations.lane_id).unwrap().state(),
				LaneState::Opened
			);
			assert_eq!(
				Bridges::<TestRuntime, ()>::get(misbehaving_locations.lane_id).map(|b| b.state),
				Some(BridgeState::Opened)
			);
			assert_eq!(
				lanes_manager
					.any_state_outbound_lane(misbehaving_locations.lane_id)
					.unwrap()
					.state(),
				LaneState::Opened
			);
			assert_eq!(
				Balances::free_balance(&misbehaving_bridge.bridge_owner_account),
				pre_free_funds_on_owner
			);
			assert_eq!(
				Balances::reserved_balance(&misbehaving_bridge.bridge_owner_account),
				pre_reserved_funds_on_owner + Penalty::get()
			);
			assert_eq!(
				Balances::free_balance(&associated_owner_account),
				pre_free_funds_on_associated - Penalty::get()
			);
			assert_eq!(
				Balances::reserved_balance(&associated_owner_account),
				pre_reserved_funds_on_associated
			);
		})
	}

	#[test]
	fn resume_misbehaving_bridges_works_with_multiple_misbehaving_bridges() {
		run_test(|| {
			let origin = AllowedOpenBridgeOrigin::parent_relay_chain_origin();
			let (bridge1, locations1) =
				mock_open_bridge_from_with(origin.clone(), bridged_parachain_location());
			let (bridge2, locations2) = mock_open_bridge_from(origin);
			assert_eq!(bridge1.bridge_owner_account, bridge2.bridge_owner_account);
			assert_ne!(locations1.lane_id, locations2.lane_id);

			Bridges::<TestRuntime, ()>::mutate_extant(locations1.lane_id, |bridge| {
				bridge.state = BridgeState::Misbehaving;
			});
			Bridges::<TestRuntime, ()>::mutate_extant(locations2.lane_id, |bridge| {
				bridge.state = BridgeState::Misbehaving;
			});
			let lanes_manager = LanesManagerOf::<TestRuntime, ()>::new();
			lanes_manager
				.active_outbound_lane(locations1.lane_id)
				.unwrap()
				.set_state(LaneState::Closed);
			lanes_manager
				.active_outbound_lane(locations2.lane_id)
				.unwrap()
				.set_state(LaneState::Closed);

			let associated_owner_account: AccountId = [42u8; 32].into();
			Balances::mint_into(
				&associated_owner_account,
				ExistentialDeposit::get() + 2 * Penalty::get(),
			)
			.unwrap();

			let pre_free_funds_on_owner = Balances::free_balance(&bridge1.bridge_owner_account);
			let pre_reserved_funds_on_owner =
				Balances::reserved_balance(&bridge1.bridge_owner_account);
			let pre_free_funds_on_associated = Balances::free_balance(&associated_owner_account);
			let pre_reserved_funds_on_associated =
				Balances::reserved_balance(&associated_owner_account);

			assert_ok!(XcmOverBridge::resume_misbehaving_bridges(
				RuntimeOrigin::signed(associated_owner_account.clone()),
				Box::new(parent_relay_chain_universal_location().into()),
			),);

			// ensure that the bridge and accounts are in the proper state
			assert_eq!(
				Bridges::<TestRuntime, ()>::get(locations1.lane_id).map(|b| b.state),
				Some(BridgeState::Opened)
			);
			assert_eq!(
				lanes_manager.any_state_outbound_lane(locations1.lane_id).unwrap().state(),
				LaneState::Opened
			);
			assert_eq!(
				Bridges::<TestRuntime, ()>::get(locations2.lane_id).map(|b| b.state),
				Some(BridgeState::Opened)
			);
			assert_eq!(
				lanes_manager.any_state_outbound_lane(locations2.lane_id).unwrap().state(),
				LaneState::Opened
			);
			assert_eq!(
				Balances::free_balance(&bridge1.bridge_owner_account),
				pre_free_funds_on_owner
			);
			assert_eq!(
				Balances::reserved_balance(&bridge1.bridge_owner_account),
				pre_reserved_funds_on_owner + 2 * Penalty::get()
			);
			assert_eq!(
				Balances::free_balance(&associated_owner_account),
				pre_free_funds_on_associated - 2 * Penalty::get()
			);
			assert_eq!(
				Balances::reserved_balance(&associated_owner_account),
				pre_reserved_funds_on_associated
			);
		})
	}
}
