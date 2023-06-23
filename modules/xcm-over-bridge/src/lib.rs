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
//! 2) the sibling parachain funds its sovereign parachain account at this bridge hub. It
//!    shall hold enough funds to pay for the bridge (see `BridgeReserve`);
//!
//! 3) the sibling parachain opens the bridge by sending XCM `Transact` instruction
//!    with the `open_bridge` call. The `BridgeReserve` amount is reserved
//!    on the sovereign account of sibling parachain;
//!
//! 4) at the other side of the bridge, the same thing (1, 2, 3) happens. Parachains that
//!    need to connect over the bridge need to coordinate the moment when they start sending
//!    messages over the bridge. Otherwise they may lose funds and/or messages;
//!
//! 5) when the bridge is opened, anyone can watch for bridge rules (TODO!!!) violations.
//!    If something goes wrong with the bridge, he may call the `report_misbehavior` method.
//     During the call, the outbound lane is immediately `Closed`. The bridge itself switched
//!    to the `Closing` state. A `Penalty` is paid (out of reserved funds) to the reporter. After
//!    `BridgeCloseDelay`, the caller may call the `close_bridge` to get his funds back;
//!
//! 6) when either side wants to close the bridge, it sends the XCM `Transact` instruction
//!    with the `request_bridge_closure` call. The bridge stays opened for `BridgeCloseDelay`
//!    blocks. This delay exists to give both chain users enough time to properly close
//!    their bridge - e.g. withdraw funds from the bridged chain and so on. After this
//!    delay passes, either side may send another XCM `Transact` instruction with the
//!    `close_bridge` call to actually close the bridge (at its side) and get its reserved
//!    funds back.

#![cfg_attr(not(feature = "std"), no_std)]

use bp_messages::{LaneId, LaneState, MessageNonce};
use bp_runtime::{AccountIdOf, BalanceOf, BlockNumberOf, RangeInclusiveExt};
use bp_xcm_over_bridge::{Bridge, BridgeLimits, BridgeMisbehavior, BridgeState};
use frame_support::traits::{tokens::BalanceStatus, Currency, ReservableCurrency};
use frame_system::Config as SystemConfig;
use pallet_bridge_messages::{Config as BridgeMessagesConfig, LanesManagerError};
use sp_runtime::{traits::Zero, Saturating};
use xcm::prelude::*;
use xcm_builder::ensure_is_remote;
use xcm_executor::traits::ConvertLocation;

pub use pallet::*;

mod mock;

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
		/// Bridged network id.
		type BridgedNetworkId: Get<NetworkId>; // TODO: it must be a part of header chain instead of `ChainId`
		/// Associated messages pallet instance that bridges us with the
		/// `BridgedNetworkId` consensus.
		type BridgeMessagesPalletInstance: 'static;

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
		type BridgeReserve: Get<BalanceOf<ThisChainOf<Self, I>>>;
		/// Currency used to paid for bridge registration.
		type NativeCurrency: ReservableCurrency<Self::AccountId>;

		/// Delay before bridge is closed.
		type BridgeCloseDelay: Get<Self::BlockNumber>;

		/// Bridge limits that the refular bridge must not exceed.
		type BridgeLimits: Get<BridgeLimits>;
		/// The penality (in chain native tokens) that is paid from the bridge reserve to the bridge
		/// misbehavior reporter.
		type Penalty: Get<BalanceOf<ThisChainOf<Self, I>>>;
	}

	/// An alias for the bridge metadata.
	pub type BridgeOf<T, I> = Bridge<ThisChainOf<T, I>>;
	/// An alias for the bridge state.
	pub type BridgeStateOf<T, I> = BridgeState<BlockNumberOf<ThisChainOf<T, I>>>;
	/// An alias for the this chain.
	pub type ThisChainOf<T, I> =
		pallet_bridge_messages::ThisChainOf<T, <T as Config<I>>::BridgeMessagesPalletInstance>;
	/// An alias for the associated lanes manager.
	pub type LanesManagerOf<T, I> =
		pallet_bridge_messages::LanesManager<T, <T as Config<I>>::BridgeMessagesPalletInstance>;

	/// Locations of bridge endpoints at both sides of the bridge.
	pub struct BridgeLocations {
		/// Relative location of this side of the bridge.
		pub bridge_origin_relative_location: Box<MultiLocation>,
		/// Universal (unique) location of this side of the bridge.
		pub bridge_origin_universal_location: Box<MultiLocation>,
		/// Universal (unique) location of other side of the bridge.
		pub bridge_destination_universal_location: Box<MultiLocation>,
		/// An identifier of the dedicated bridge message lane.
		pub lane_id: LaneId,
	}

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
		/// parachain or a parent relay chain). The `bridge_destination_universal_location` must be
		/// a destination within the consensus of the `T::BridgedNetworkId` network.
		///
		/// The `BridgeReserve` amount is reserved on the caller account. This reserve
		/// is unreserved after bridge is closed.
		///
		/// The states after this call: bridge is `Opened`, outbound lane is `Opened`, inbound lane
		/// is `Opened`.
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::zero())]
		pub fn open_bridge(
			origin: OriginFor<T>,
			bridge_destination_universal_location: Box<MultiLocation>, // TODO: versioned?
		) -> DispatchResult {
			// check and compute required bridge locations
			let locations =
				Self::bridge_locations(origin, bridge_destination_universal_location, true)?;

			// reserve balance on the parachain sovereign account
			let reserve = T::BridgeReserve::get();
			let bridge_owner_account = T::BridgeOriginAccountIdConverter::convert_location(
				&locations.bridge_origin_relative_location,
			)
			.ok_or(Error::<T, I>::InvalidBridgeOriginAccount)?;
			T::NativeCurrency::reserve(&bridge_owner_account, reserve.clone())
				.map_err(|_| Error::<T, I>::FailedToReserveBridgeReserve)?;

			// save bridge metadata
			Bridges::<T, I>::try_mutate(locations.lane_id, |bridge| match bridge {
				Some(_) => Err(Error::<T, I>::BridgeAlreadyExists),
				None => Ok(*bridge = Some(BridgeOf::<T, I> {
					state: BridgeStateOf::<T, I>::Opened,
					bridge_owner_account,
					reserve,
				})),
			})?;

			// create new lanes. Under normal circumstances, following calls shall never fail
			let lanes_manager = LanesManagerOf::<T, I>::new();
			lanes_manager
				.create_inbound_lane(locations.lane_id)
				.map_err(Into::<Error<T, I>>::into)?;
			lanes_manager
				.create_outbound_lane(locations.lane_id)
				.map_err(Into::<Error<T, I>>::into)?;

			// deposit `BridgeOpened` event
			Self::deposit_event(Event::<T, I>::BridgeOpened {
				lane_id: locations.lane_id,
				local_endpoint: locations.bridge_origin_universal_location,
				remote_endpoint: locations.bridge_destination_universal_location,
			});

			Ok(())
		}

		/// Request previously opened bridge closure.
		///
		/// Can only be called by the "owner" of this side of the bridge. Both inbound and outbound
		/// lanes at this side of the bridge are immediately switched to `Closing` state. Lanes may
		/// still send/receive new messages, however this state is reported to the other side of
		/// the bridge, given that relays are active. When this happens, lanes at the other side
		/// also switching their state to `Closing`.
		///
		/// After `T::BridgeCloseDelay` blocks, the caller may issue the `close_bridge` call to
		/// actually close the bridge and claim reserved funds.
		///
		/// The states after this call: bridge is `Closing` in `BridgeCloseDelay` blocks, outbound
		/// lane is `Closing`, inbound lane is `Closing`.
		#[pallet::call_index(1)]
		#[pallet::weight(Weight::zero())]
		pub fn request_bridge_closure(
			origin: OriginFor<T>,
			bridge_destination_universal_location: Box<MultiLocation>, // TODO: versioned?
		) -> DispatchResult {
			// compute required bridge locations
			let locations =
				Self::bridge_locations(origin, bridge_destination_universal_location, false)?;

			// update bridge metadata
			let may_close_at =
				Self::start_closing_the_bridge(locations.lane_id, false, LaneState::Closing)?;

			// deposit the `BridgeClosureRequested` event
			Self::deposit_event(Event::<T, I>::BridgeClosureRequested {
				lane_id: locations.lane_id,
				may_close_at,
			});

			Ok(())
		}

		/// Try to close the bridge.
		///
		/// Can only be called by the "owner" of this side of the bridge. Can only be called if
		/// bridge has switched to `Closing` state at least `T::BridgeCloseDelay` blocks ago.
		///
		/// Closed bridge is a bridge without any traces in the runtime storage. So this method
		/// first tries to prune all queued messages at the outbound lane. When there are no
		/// outbound messages left, outbound and inbound lanes are pruned. After that, funds
		/// are returned back to the owner of this side of the bridge.
		///
		/// The number of messages that we may prune in a single call is limited by the
		/// `may_prune_messages` argument. If there are more messages in the queue, the method
		/// prunes exactly `may_prune_messages` and exits early. The caller may call it again
		/// until outbound queue is depleted and get his funds back.
		///
		/// The states after this call: everything is either `Closed`, or pruned from the
		/// runtime storage..
		#[pallet::call_index(2)]
		#[pallet::weight(Weight::zero())]
		pub fn close_bridge(
			origin: OriginFor<T>,
			bridge_destination_universal_location: Box<MultiLocation>, // TODO: versioned?
			may_prune_messages: MessageNonce,
		) -> DispatchResult {
			// compute required bridge locations
			let locations =
				Self::bridge_locations(origin, bridge_destination_universal_location, false)?;

			// TODO: may do refund here, if bridge/lanes are already closed + for messages that are
			// not pruned

			// update bridge metadata - this also guarantees that the bridge is in the proper state
			let bridge =
				Bridges::<T, I>::try_mutate_exists(locations.lane_id, |bridge| match bridge {
					Some(bridge) if bridge.state == BridgeState::Opened =>
						Err(Error::<T, I>::CannotCloseOpenedBridge),
					Some(bridge) => {
						bridge.state = BridgeState::Closed;
						Ok(bridge.clone())
					},
					None => Err(Error::<T, I>::UnknownBridge),
				})?;

			// now prune queued messages
			let lanes_manager = LanesManagerOf::<T, I>::new();
			let mut pruned_messages = 0;
			let mut outbound_lane = lanes_manager
				.any_state_outbound_lane(locations.lane_id)
				.map_err(Into::<Error<T, I>>::into)?;
			ensure!(
				matches!(outbound_lane.state(), LaneState::Closing | LaneState::Closed),
				Error::<T, I>::CannotCloseOpenedLane,
			);
			for message_nonce in outbound_lane.queued_messages() {
				if pruned_messages == may_prune_messages {
					break
				}

				outbound_lane.remove_message(message_nonce);
				pruned_messages += 1;
			}

			// if there are outbound messages in the queue, just update states and early exit
			if !outbound_lane.queued_messages().is_empty() {
				// update lanes state. Under normal circumstances, following calls shall never fail
				lanes_manager
					.any_state_inbound_lane(locations.lane_id)
					.map_err(Into::<Error<T, I>>::into)?
					.set_state(LaneState::Closed);
				outbound_lane.set_state(LaneState::Closed);

				// deposit the `ClosingBridge` event
				Self::deposit_event(Event::<T, I>::ClosingBridge {
					lane_id: locations.lane_id,
					pruned_messages,
				});

				return Ok(())
			}

			// else we have pruned all messages, so lanes and the bridge itself may gone
			lanes_manager
				.any_state_inbound_lane(locations.lane_id)
				.map_err(Into::<Error<T, I>>::into)?
				.purge();
			outbound_lane.purge();
			Bridges::<T, I>::remove(locations.lane_id);

			// unreserve remaining amount
			let failed_to_unreserve =
				T::NativeCurrency::unreserve(&bridge.bridge_owner_account, bridge.reserve);
			if !failed_to_unreserve.is_zero() {
				// TODO: log
			}

			// deposit the `BridgePruned` event
			Self::deposit_event(Event::<T, I>::BridgePruned { lane_id: locations.lane_id });

			Ok(())
		}

		/// Report bridge misbehavior.
		#[pallet::call_index(3)]
		#[pallet::weight(Weight::zero())]
		pub fn report_misbehavior(
			origin: OriginFor<T>,
			lane_id: LaneId,
			misbehavior: BridgeMisbehavior,
		) -> DispatchResult {
			let reporter = ensure_signed(origin)?;
			let lanes_manager = LanesManagerOf::<T, I>::new();

			// check report
			match misbehavior {
				BridgeMisbehavior::TooManyQueuedOutboundMessages => {
					let outbound_lane =
						lanes_manager.outbound_lane(lane_id).map_err(Into::<Error<T, I>>::into)?;
					let queued_messages =
						outbound_lane.queued_messages().checked_len().unwrap_or(0);
					let max_queued_messages = T::BridgeLimits::get().max_queued_outbound_messages;
					if queued_messages <= max_queued_messages {
						return Err(Error::<T, I>::InvalidMisbehaviorReport.into())
					}
				},
			}

			// fine bridge owner and close the bridge
			let may_close_at = Self::on_misbehavior(reporter, lane_id)?;

			// deposit the `BridgeMisbehaving` event
			Self::deposit_event(Event::<T, I>::BridgeMisbehaving {
				lane_id,
				misbehavior,
				may_close_at,
			});

			Ok(())
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
			bridge_destination_universal_location: Box<MultiLocation>,
			is_open_bridge_request: bool,
		) -> Result<BridgeLocations, sp_runtime::DispatchError> {
			// get locations of endpoint, located at this side of the bridge
			let this_location = T::UniversalLocation::get();
			let bridge_origin_relative_location =
				Box::new(T::AllowedOpenBridgeOrigin::ensure_origin(origin)?);
			let bridge_origin_universal_location = Box::new(
				this_location
					.within_global(*bridge_origin_relative_location)
					.map(|interior| MultiLocation::new(0, interior))
					.map_err(|_| Error::<T, I>::InvalidBridgeOrigin)?,
			);

			// if we are jsut opening the bridge, let's ensure that the
			// `bridge_destination_universal_location` is correct and is within bridged consensus.
			// We can ignore this check if bridge is already closed, because lane is already opened
			// and all checks have happened during opening
			if is_open_bridge_request {
				let (remote_network, _) =
					ensure_is_remote(this_location, *bridge_destination_universal_location)
						.map_err(|_| Error::<T, I>::CannotBridgeWithLocalDestination)?;
				ensure!(
					remote_network == T::BridgedNetworkId::get(),
					Error::<T, I>::UnreachableRemoteDestination
				);
			}

			// we know that the `bridge_destination_universal_location` starts from the
			// `GlobalConsensus` (see `ensure_is_remote`) and we know that the `UniversalLocation`
			// is also within the `GlobalConsensus` (given the proper pallet configuration). So we
			// know that the lane id will be the same on both ends of the bridge
			let lane_id = LaneId::new(
				*bridge_origin_universal_location,
				*bridge_destination_universal_location,
			);

			Ok(BridgeLocations {
				bridge_origin_relative_location,
				bridge_origin_universal_location,
				bridge_destination_universal_location,
				lane_id,
			})
		}

		/// Start closing the bridge. Returns block at which bridge can be actually closed.
		fn start_closing_the_bridge(
			lane_id: LaneId,
			allow_closing_bridges: bool,
			outbound_lane_state: LaneState,
		) -> Result<BlockNumberOf<ThisChainOf<T, I>>, Error<T, I>> {
			// update bridge metadata
			let may_close_at =
				Bridges::<T, I>::try_mutate_exists(lane_id, |bridge| match bridge {
					Some(bridge) => match bridge.state {
						BridgeState::Opened => {
							let may_close_at = frame_system::Pallet::<T>::block_number()
								.saturating_add(T::BridgeCloseDelay::get());
							bridge.state = BridgeState::Closing(may_close_at);
							Ok(may_close_at)
						},
						BridgeState::Closing(may_close_at) => {
							if !allow_closing_bridges {
								return Err(Error::<T, I>::BridgeAlreadyClosed)
							}

							Ok(may_close_at)
						},
						BridgeState::Closed => Err(Error::<T, I>::BridgeAlreadyClosed),
					},
					None => Err(Error::<T, I>::UnknownBridge),
				})?;

			// update lanes state. Under normal circumstances, following calls shall never fail
			let lanes_manager = LanesManagerOf::<T, I>::new();
			lanes_manager
				.inbound_lane(lane_id)
				.map_err(Into::<Error<T, I>>::into)?
				.set_state(LaneState::Closing);
			lanes_manager
				.outbound_lane(lane_id)
				.map_err(Into::<Error<T, I>>::into)?
				.set_state(outbound_lane_state);

			Ok(may_close_at)
		}

		/// Actions when misbehavior is detected.
		fn on_misbehavior(
			reporter: T::AccountId,
			lane_id: LaneId,
		) -> Result<BlockNumberOf<ThisChainOf<T, I>>, Error<T, I>> {
			// transfer penalty to the reporter
			let bridge = Bridges::<T, I>::get(lane_id).ok_or(Error::<T, I>::UnknownBridge)?;
			T::NativeCurrency::repatriate_reserved(
				&bridge.bridge_owner_account,
				&reporter,
				T::Penalty::get(),
				BalanceStatus::Free,
			); // TODO: process result

			// start closing the bridge
			Self::start_closing_the_bridge(lane_id, true, LaneState::Closed)
		}
	}

	/// All registered bridges.
	#[pallet::storage]
	pub type Bridges<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Identity, LaneId, BridgeOf<T, I>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config<I>, I: 'static = ()> {
		/// The bridge between two locations has been opened.
		BridgeOpened {
			/// Bridge and its lane identifier.
			lane_id: LaneId,
			/// Bridge endpoint location within local consensus.
			local_endpoint: Box<MultiLocation>,
			/// Bridge endpoint location within remote consensus.
			remote_endpoint: Box<MultiLocation>,
		},
		/// Bridge closure has been requested.
		BridgeClosureRequested {
			/// Bridge and its lane identifier.
			lane_id: LaneId,
			/// Block where the bridge may be actually closed.
			may_close_at: T::BlockNumber,
		},
		/// Bridge is going to be closed, but not yet fully pruned from the runtime storage.
		ClosingBridge {
			/// Bridge and its lane identifier.
			lane_id: LaneId,
			/// Number of pruned messages during the close call.
			pruned_messages: MessageNonce,
		},
		/// Bridge has been closed and pruned from the runtime storage. It now may be reopened
		/// again by any participant.
		BridgePruned {
			/// Bridge and its lane identifier.
			lane_id: LaneId,
		},
		/// Bridge is misbehaving.
		BridgeMisbehaving {
			/// Bridge and its lane identifier.
			lane_id: LaneId,
			/// Bridge misbehavior type.
			misbehavior: BridgeMisbehavior,
			/// Block where the bridge may be actually closed.
			may_close_at: T::BlockNumber,
		},
	}

	#[pallet::error]
	pub enum Error<T, I = ()> {
		/// Invalid local bridge origin.
		InvalidBridgeOrigin,
		/// Invalid local bridge origin account.
		InvalidBridgeOriginAccount,
		/// Remote bridge destination is local to this consensus.
		CannotBridgeWithLocalDestination,
		/// Remote bridge destination is unreachable via this pallet instance.
		UnreachableRemoteDestination,
		/// The bridge is already registered in this pallet.
		BridgeAlreadyExists,
		/// Trying to close already closed bridge.
		BridgeAlreadyClosed,
		/// The inbound lane is already registered in the associated messages pallet.
		InboundLaneAlreadyExists,
		/// The outbound lane is already registered in the associated messages pallet.
		OutboundLaneAlreadyExists,
		/// The inbound lane is missing from the associated messages pallet storage.
		UnknownInboundLane,
		/// The outbound lane is missing from the associated messages pallet storage.
		UnknownOutboundLane,
		/// The inbound lane is already closed at the associated messages pallet.
		ClosedInboundLane,
		/// The outbound lane is already closed at the associated messages pallet.
		ClosedOutboundLane,
		/// Trying to access unknown bridge.
		UnknownBridge,
		/// The bridge origin can't pay the required amount for opening the bridge.
		FailedToReserveBridgeReserve,
		/// Cannot close opened bridge. It should be switched to `Closing` state first.
		CannotCloseOpenedBridge,
		/// Cannot close opened lane. It should be switched to `Closing` state first.
		CannotCloseOpenedLane,
		/// Invalid misbehavior report has been submitted.
		InvalidMisbehaviorReport,
	}

	impl<T: Config<I>, I: 'static> From<LanesManagerError> for Error<T, I> {
		fn from(e: LanesManagerError) -> Self {
			match e {
				LanesManagerError::InboundLaneAlreadyExists => Error::InboundLaneAlreadyExists,
				LanesManagerError::OutboundLaneAlreadyExists => Error::OutboundLaneAlreadyExists,
				LanesManagerError::UnknownInboundLane => Error::UnknownInboundLane,
				LanesManagerError::UnknownOutboundLane => Error::UnknownOutboundLane,
				LanesManagerError::ClosedInboundLane => Error::ClosedInboundLane,
				LanesManagerError::ClosedOutboundLane => Error::ClosedOutboundLane,
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use mock::*;

	use frame_support::{assert_ok, traits::fungible::Mutate};
	use frame_system::{EventRecord, Phase};

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
					Box::new(bridged_asset_hub_location()),
					false,
				)
				.unwrap();

				// ensure that there's no bridge and lanes in the storage
				assert_eq!(Bridges::<TestRuntime, ()>::get(locations.lane_id), None);
				assert_eq!(
					lanes_manager.inbound_lane(locations.lane_id).map(drop),
					Err(LanesManagerError::UnknownInboundLane)
				);
				assert_eq!(
					lanes_manager.outbound_lane(locations.lane_id).map(drop),
					Err(LanesManagerError::UnknownOutboundLane)
				);

				// give enough funds to the sovereign account of the bridge origin
				let bridge_owner_account = LocationToAccountId::convert_location(
					&locations.bridge_origin_relative_location,
				)
				.unwrap();
				Balances::mint_into(&bridge_owner_account, expected_reserve + existential_deposit)
					.unwrap();
				assert_eq!(
					Balances::free_balance(&bridge_owner_account),
					expected_reserve + existential_deposit
				);
				assert_eq!(Balances::reserved_balance(&bridge_owner_account), 0);

				// now open the bridge
				assert_ok!(XcmOverBridge::open_bridge(
					origin,
					locations.bridge_destination_universal_location.clone(),
				));

				// ensure that everything has been set up in the runtime storage
				assert_eq!(
					Bridges::<TestRuntime, ()>::get(locations.lane_id),
					Some(Bridge {
						state: BridgeState::Opened,
						bridge_owner_account: bridge_owner_account.clone(),
						reserve: expected_reserve,
					}),
				);
				assert_eq!(
					lanes_manager.inbound_lane(locations.lane_id).map(|l| l.state()),
					Ok(LaneState::Opened)
				);
				assert_eq!(
					lanes_manager.outbound_lane(locations.lane_id).map(|l| l.state()),
					Ok(LaneState::Opened)
				);
				assert_eq!(Balances::free_balance(&bridge_owner_account), existential_deposit);
				assert_eq!(Balances::reserved_balance(&bridge_owner_account), expected_reserve);

				// ensure that teh proper event is deposited
				assert_eq!(
					System::events().last(),
					Some(&EventRecord {
						phase: Phase::Initialization,
						event: RuntimeEvent::XcmOverBridge(Event::BridgeOpened {
							lane_id: locations.lane_id,
							local_endpoint: locations.bridge_origin_universal_location,
							remote_endpoint: locations.bridge_destination_universal_location,
						}),
						topics: vec![],
					}),
				);
			}
		});
	}
}
