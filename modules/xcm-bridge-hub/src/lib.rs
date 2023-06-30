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
use bp_xcm_bridge_hub::{Bridge, BridgeLimits, BridgeMisbehavior, BridgeState};
use frame_support::traits::{tokens::BalanceStatus, Currency, ReservableCurrency};
use frame_system::Config as SystemConfig;
use pallet_bridge_messages::{Config as BridgeMessagesConfig, LanesManagerError};
use sp_runtime::{traits::Zero, Saturating};
use xcm::prelude::*;
use xcm_builder::ensure_is_remote;
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
					Some(bridge) => match bridge.state {
						BridgeState::Opened => Err(Error::<T, I>::CannotCloseOpenedBridge),
						BridgeState::Closed => Ok(bridge.clone()),
						BridgeState::Closing(may_close_at)
							if frame_system::Pallet::<T>::block_number() >= may_close_at =>
						{
							bridge.state = BridgeState::Closed;
							Ok(bridge.clone())
						},
						BridgeState::Closing(_) => Err(Error::<T, I>::CannotCloseOpenedBridge),
					},
					None => Err(Error::<T, I>::UnknownBridge),
				})?;

			// ensure that bridge lanes are either closing or closed
			let lanes_manager = LanesManagerOf::<T, I>::new();
			let mut inbound_lane = lanes_manager
				.any_state_inbound_lane(locations.lane_id)
				.map_err(Into::<Error<T, I>>::into)?;
			ensure!(
				matches!(inbound_lane.state(), LaneState::Closing | LaneState::Closed),
				Error::<T, I>::CannotCloseOpenedLane,
			);
			let mut outbound_lane = lanes_manager
				.any_state_outbound_lane(locations.lane_id)
				.map_err(Into::<Error<T, I>>::into)?;
			ensure!(
				matches!(outbound_lane.state(), LaneState::Closing | LaneState::Closed),
				Error::<T, I>::CannotCloseOpenedLane,
			);

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

				// deposit the `ClosingBridge` event
				Self::deposit_event(Event::<T, I>::ClosingBridge {
					lane_id: locations.lane_id,
					pruned_messages,
				});

				return Ok(())
			}

			// else we have pruned all messages, so lanes and the bridge itself may gone
			inbound_lane.purge();
			outbound_lane.purge();
			Bridges::<T, I>::remove(locations.lane_id);

			// unreserve remaining amount
			let failed_to_unreserve =
				T::NativeCurrency::unreserve(&bridge.bridge_owner_account, bridge.reserve);
			if !failed_to_unreserve.is_zero() {
				// TODO: log
			}

			// deposit the `BridgePruned` event
			Self::deposit_event(Event::<T, I>::BridgePruned {
				lane_id: locations.lane_id,
				pruned_messages,
			});

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
			// TODO: move this function to primitives, because we will want the same function
			//       at the sibling/parent chain in the `pallet-xcm-bridge-hub-router` pallet
			// TODO: update this function to drop everything below the parachain level from
			//       `bridge_destination_universal_location` - i.e. bridge with
			//       `X3(GlobalConsensus(Kusama), Parachain(1000), AccountId32([0u8; 32])`
			//       must use the same lane as `X2(GlobalConsensus(Kusama), Parachain(1000)`.
			//       The router at the target chain will handle that.


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

			// update bridge metadata (we know it exists, because it has been read at the beginning
			// of this method)
			Bridges::<T, I>::mutate_extant(lane_id, |bridge| {
				bridge.reserve = updated_reserve;
			});

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
			/// Number of pruned messages during the close call.
			pruned_messages: MessageNonce,
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

	use frame_support::{assert_noop, assert_ok, traits::fungible::Mutate};
	use frame_system::{EventRecord, Phase};

	fn fund_origin_sovereign_account(locations: &BridgeLocations, balance: Balance) -> AccountId {
		let bridge_owner_account =
			LocationToAccountId::convert_location(&locations.bridge_origin_relative_location)
				.unwrap();
		Balances::mint_into(&bridge_owner_account, balance).unwrap();
		bridge_owner_account
	}

	fn mock_open_bridge_from(
		origin: RuntimeOrigin,
	) -> (BridgeOf<TestRuntime, ()>, BridgeLocations) {
		let reserve = BridgeReserve::get();
		let locations =
			XcmOverBridge::bridge_locations(origin, Box::new(bridged_asset_hub_location()), false)
				.unwrap();
		let bridge_owner_account =
			fund_origin_sovereign_account(&locations, reserve + ExistentialDeposit::get());
		Balances::reserve(&bridge_owner_account, reserve).unwrap();

		let bridge = Bridge { state: BridgeState::Opened, bridge_owner_account, reserve };
		Bridges::<TestRuntime, ()>::insert(locations.lane_id, bridge.clone());

		let lanes_manager = LanesManagerOf::<TestRuntime, ()>::new();
		lanes_manager.create_inbound_lane(locations.lane_id).unwrap();
		lanes_manager.create_outbound_lane(locations.lane_id).unwrap();

		(bridge, locations)
	}

	fn mock_closing_bridge_from(
		origin: RuntimeOrigin,
	) -> (BridgeOf<TestRuntime, ()>, BridgeLocations, BlockNumber) {
		let (mut bridge, locations) = mock_open_bridge_from(origin);

		let may_close_at = 1 + BridgeCloseDelay::get();
		bridge.state = BridgeState::Closing(may_close_at);
		Bridges::<TestRuntime, ()>::insert(locations.lane_id, bridge.clone());

		let lanes_manager = LanesManagerOf::<TestRuntime, ()>::new();
		lanes_manager
			.inbound_lane(locations.lane_id)
			.unwrap()
			.set_state(LaneState::Closing);
		lanes_manager
			.outbound_lane(locations.lane_id)
			.unwrap()
			.set_state(LaneState::Closing);

		(bridge, locations, may_close_at)
	}

	fn enqueue_message(lane: LaneId) {
		let lanes_manager = LanesManagerOf::<TestRuntime, ()>::new();
		lanes_manager.outbound_lane(lane).unwrap().send_message(vec![42]).unwrap();
	}

	#[test]
	fn open_bridge_fails_if_origin_is_not_allowed() {
		run_test(|| {
			assert_noop!(
				XcmOverBridge::open_bridge(
					AllowedOpenBridgeOrigin::disallowed_origin(),
					Box::new(bridged_asset_hub_location()),
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
					Box::new(bridged_asset_hub_location()),
				),
				Error::<TestRuntime, ()>::InvalidBridgeOrigin,
			);

			assert_noop!(
				XcmOverBridge::open_bridge(
					AllowedOpenBridgeOrigin::sibling_parachain_universal_origin(),
					Box::new(bridged_asset_hub_location()),
				),
				Error::<TestRuntime, ()>::InvalidBridgeOrigin,
			);
		})
	}

	#[test]
	fn open_bridge_fails_if_destination_is_not_remote() {
		run_test(|| {
			assert_noop!(
				XcmOverBridge::open_bridge(
					AllowedOpenBridgeOrigin::parent_relay_chain_origin(),
					Box::new(MultiLocation {
						parents: 2,
						interior: X2(
							GlobalConsensus(RelayNetwork::get()),
							Parachain(BRIDGED_ASSET_HUB_ID)
						)
					}),
				),
				Error::<TestRuntime, ()>::CannotBridgeWithLocalDestination,
			);
		});
	}

	#[test]
	fn open_bridge_fails_if_outside_of_bridged_consensus() {
		run_test(|| {
			assert_noop!(
				XcmOverBridge::open_bridge(
					AllowedOpenBridgeOrigin::parent_relay_chain_origin(),
					Box::new(MultiLocation {
						parents: 2,
						interior: X2(
							GlobalConsensus(NonBridgedRelayNetwork::get()),
							Parachain(BRIDGED_ASSET_HUB_ID)
						)
					}),
				),
				Error::<TestRuntime, ()>::UnreachableRemoteDestination,
			);
		});
	}

	#[test]
	fn open_bridge_fails_if_origin_has_no_sovereign_account() {
		run_test(|| {
			assert_noop!(
				XcmOverBridge::open_bridge(
					AllowedOpenBridgeOrigin::origin_without_sovereign_account(),
					Box::new(bridged_asset_hub_location()),
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
					Box::new(bridged_asset_hub_location()),
				),
				Error::<TestRuntime, ()>::FailedToReserveBridgeReserve,
			);
		});
	}

	#[test]
	fn open_bridge_fails_if_it_already_exists() {
		run_test(|| {
			let origin = AllowedOpenBridgeOrigin::parent_relay_chain_origin();
			let locations = XcmOverBridge::bridge_locations(
				origin.clone(),
				Box::new(bridged_asset_hub_location()),
				false,
			)
			.unwrap();
			fund_origin_sovereign_account(
				&locations,
				BridgeReserve::get() + ExistentialDeposit::get(),
			);

			Bridges::<TestRuntime, ()>::insert(
				locations.lane_id,
				Bridge {
					state: BridgeState::Opened,
					bridge_owner_account: [0u8; 32].into(),
					reserve: 0,
				},
			);

			assert_noop!(
				XcmOverBridge::open_bridge(origin, Box::new(bridged_asset_hub_location()),),
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
				Box::new(bridged_asset_hub_location()),
				false,
			)
			.unwrap();
			fund_origin_sovereign_account(
				&locations,
				BridgeReserve::get() + ExistentialDeposit::get(),
			);

			let lanes_manager = LanesManagerOf::<TestRuntime, ()>::new();

			lanes_manager.create_inbound_lane(locations.lane_id).unwrap();
			assert_noop!(
				XcmOverBridge::open_bridge(origin.clone(), Box::new(bridged_asset_hub_location()),),
				Error::<TestRuntime, ()>::InboundLaneAlreadyExists,
			);

			lanes_manager.inbound_lane(locations.lane_id).unwrap().purge();
			lanes_manager.create_outbound_lane(locations.lane_id).unwrap();
			assert_noop!(
				XcmOverBridge::open_bridge(origin, Box::new(bridged_asset_hub_location()),),
				Error::<TestRuntime, ()>::OutboundLaneAlreadyExists,
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

				// ensure that the proper event is deposited
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

	#[test]
	fn request_bridge_closure_fails_if_origin_is_not_allowed() {
		run_test(|| {
			assert_noop!(
				XcmOverBridge::request_bridge_closure(
					AllowedOpenBridgeOrigin::disallowed_origin(),
					Box::new(bridged_asset_hub_location()),
				),
				sp_runtime::DispatchError::BadOrigin,
			);
		})
	}

	#[test]
	fn request_bridge_closure_fails_if_origin_is_not_relative() {
		run_test(|| {
			assert_noop!(
				XcmOverBridge::request_bridge_closure(
					AllowedOpenBridgeOrigin::parent_relay_chain_universal_origin(),
					Box::new(bridged_asset_hub_location()),
				),
				Error::<TestRuntime, ()>::InvalidBridgeOrigin,
			);

			assert_noop!(
				XcmOverBridge::request_bridge_closure(
					AllowedOpenBridgeOrigin::sibling_parachain_universal_origin(),
					Box::new(bridged_asset_hub_location()),
				),
				Error::<TestRuntime, ()>::InvalidBridgeOrigin,
			);
		})
	}

	#[test]
	fn request_bridge_closure_fails_if_bridge_is_already_closing() {
		run_test(|| {
			let origin = AllowedOpenBridgeOrigin::parent_relay_chain_origin();
			let (_, locations) = mock_open_bridge_from(origin.clone());

			Bridges::<TestRuntime, ()>::mutate_extant(locations.lane_id, |bridge| {
				bridge.state = BridgeState::Closing(0xFFFF);
			});

			assert_noop!(
				XcmOverBridge::request_bridge_closure(
					origin,
					locations.bridge_destination_universal_location,
				),
				Error::<TestRuntime, ()>::BridgeAlreadyClosed,
			);
		})
	}

	#[test]
	fn request_bridge_closure_fails_if_bridge_is_already_closed() {
		run_test(|| {
			let origin = AllowedOpenBridgeOrigin::parent_relay_chain_origin();
			let (_, locations) = mock_open_bridge_from(origin.clone());

			Bridges::<TestRuntime, ()>::mutate_extant(locations.lane_id, |bridge| {
				bridge.state = BridgeState::Closed;
			});

			assert_noop!(
				XcmOverBridge::request_bridge_closure(
					origin,
					locations.bridge_destination_universal_location,
				),
				Error::<TestRuntime, ()>::BridgeAlreadyClosed,
			);
		})
	}

	#[test]
	fn request_bridge_closure_fails_if_its_lanes_are_missing() {
		run_test(|| {
			let origin = AllowedOpenBridgeOrigin::parent_relay_chain_origin();
			let (_, locations) = mock_open_bridge_from(origin.clone());

			let lanes_manager = LanesManagerOf::<TestRuntime, ()>::new();
			lanes_manager.inbound_lane(locations.lane_id).unwrap().purge();
			assert_noop!(
				XcmOverBridge::request_bridge_closure(
					origin.clone(),
					locations.bridge_destination_universal_location,
				),
				Error::<TestRuntime, ()>::UnknownInboundLane,
			);

			lanes_manager.outbound_lane(locations.lane_id).unwrap().purge();
			let (_, locations) = mock_open_bridge_from(origin.clone());
			lanes_manager.outbound_lane(locations.lane_id).unwrap().purge();
			assert_noop!(
				XcmOverBridge::request_bridge_closure(
					origin.clone(),
					locations.bridge_destination_universal_location,
				),
				Error::<TestRuntime, ()>::UnknownOutboundLane,
			);
		})
	}

	#[test]
	fn request_bridge_closure_fails_if_its_lanes_are_closed() {
		run_test(|| {
			let origin = AllowedOpenBridgeOrigin::parent_relay_chain_origin();
			let (_, locations) = mock_open_bridge_from(origin.clone());

			let lanes_manager = LanesManagerOf::<TestRuntime, ()>::new();
			lanes_manager
				.inbound_lane(locations.lane_id)
				.unwrap()
				.set_state(LaneState::Closed);
			assert_noop!(
				XcmOverBridge::request_bridge_closure(
					origin.clone(),
					locations.bridge_destination_universal_location.clone(),
				),
				Error::<TestRuntime, ()>::ClosedInboundLane,
			);
			lanes_manager
				.any_state_inbound_lane(locations.lane_id)
				.unwrap()
				.set_state(LaneState::Opened);

			lanes_manager
				.outbound_lane(locations.lane_id)
				.unwrap()
				.set_state(LaneState::Closed);
			assert_noop!(
				XcmOverBridge::request_bridge_closure(
					origin.clone(),
					locations.bridge_destination_universal_location,
				),
				Error::<TestRuntime, ()>::ClosedOutboundLane,
			);
		});
	}

	#[test]
	fn request_bridge_closure_works() {
		run_test(|| {
			let origin = AllowedOpenBridgeOrigin::parent_relay_chain_origin();
			let (bridge, locations) = mock_open_bridge_from(origin.clone());
			System::set_block_number(1);

			// remember owner balances
			let free_balance = Balances::free_balance(&bridge.bridge_owner_account);
			let reserved_balance = Balances::reserved_balance(&bridge.bridge_owner_account);

			// start closing the bridge
			assert_ok!(XcmOverBridge::request_bridge_closure(
				origin.clone(),
				locations.bridge_destination_universal_location.clone(),
			),);

			// check that state of the bridge and its lanes have changed
			let lanes_manager = LanesManagerOf::<TestRuntime, ()>::new();
			let may_close_at = 1 + BridgeCloseDelay::get();
			assert_eq!(
				Bridges::<TestRuntime, ()>::get(locations.lane_id).map(|b| b.state),
				Some(BridgeState::Closing(may_close_at))
			);
			assert_eq!(
				lanes_manager.inbound_lane(locations.lane_id).unwrap().state(),
				LaneState::Closing
			);
			assert_eq!(
				lanes_manager.outbound_lane(locations.lane_id).unwrap().state(),
				LaneState::Closing
			);

			// check that balances have not changed
			assert_eq!(Balances::free_balance(&bridge.bridge_owner_account), free_balance);
			assert_eq!(Balances::reserved_balance(&bridge.bridge_owner_account), reserved_balance);

			// finally - check runtime events
			assert_eq!(
				System::events().last(),
				Some(&EventRecord {
					phase: Phase::Initialization,
					event: RuntimeEvent::XcmOverBridge(Event::BridgeClosureRequested {
						lane_id: locations.lane_id,
						may_close_at,
					}),
					topics: vec![],
				}),
			);
		});
	}

	#[test]
	fn close_bridge_fails_if_origin_is_not_allowed() {
		run_test(|| {
			assert_noop!(
				XcmOverBridge::close_bridge(
					AllowedOpenBridgeOrigin::disallowed_origin(),
					Box::new(bridged_asset_hub_location()),
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
					Box::new(bridged_asset_hub_location()),
					0,
				),
				Error::<TestRuntime, ()>::InvalidBridgeOrigin,
			);

			assert_noop!(
				XcmOverBridge::close_bridge(
					AllowedOpenBridgeOrigin::sibling_parachain_universal_origin(),
					Box::new(bridged_asset_hub_location()),
					0,
				),
				Error::<TestRuntime, ()>::InvalidBridgeOrigin,
			);
		})
	}

	#[test]
	fn close_bridge_fails_if_bridge_is_opened() {
		run_test(|| {
			let origin = AllowedOpenBridgeOrigin::parent_relay_chain_origin();
			let (_, locations) = mock_open_bridge_from(origin.clone());

			assert_noop!(
				XcmOverBridge::close_bridge(
					origin,
					locations.bridge_destination_universal_location,
					0,
				),
				Error::<TestRuntime, ()>::CannotCloseOpenedBridge,
			);
		});
	}

	#[test]
	fn close_bridge_fails_if_bridge_is_still_closing() {
		run_test(|| {
			let origin = AllowedOpenBridgeOrigin::parent_relay_chain_origin();
			let (_, locations, _) = mock_closing_bridge_from(origin.clone());

			assert_noop!(
				XcmOverBridge::close_bridge(
					origin,
					locations.bridge_destination_universal_location,
					0,
				),
				Error::<TestRuntime, ()>::CannotCloseOpenedBridge,
			);
		});
	}

	#[test]
	fn close_bridge_fails_if_its_lanes_are_unknown() {
		run_test(|| {
			let origin = AllowedOpenBridgeOrigin::parent_relay_chain_origin();
			let (_, locations, may_close_at) = mock_closing_bridge_from(origin.clone());
			System::set_block_number(may_close_at);

			let lanes_manager = LanesManagerOf::<TestRuntime, ()>::new();
			lanes_manager.any_state_inbound_lane(locations.lane_id).unwrap().purge();
			assert_noop!(
				XcmOverBridge::close_bridge(
					origin.clone(),
					locations.bridge_destination_universal_location,
					0,
				),
				Error::<TestRuntime, ()>::UnknownInboundLane,
			);
			lanes_manager.any_state_outbound_lane(locations.lane_id).unwrap().purge();

			let (_, locations, _) = mock_closing_bridge_from(origin.clone());
			lanes_manager.any_state_outbound_lane(locations.lane_id).unwrap().purge();
			assert_noop!(
				XcmOverBridge::close_bridge(
					origin,
					locations.bridge_destination_universal_location,
					0,
				),
				Error::<TestRuntime, ()>::UnknownOutboundLane,
			);
		});
	}

	#[test]
	fn close_bridge_fails_if_its_lanes_are_opened() {
		run_test(|| {
			let origin = AllowedOpenBridgeOrigin::parent_relay_chain_origin();
			let (_, locations, may_close_at) = mock_closing_bridge_from(origin.clone());
			System::set_block_number(may_close_at);

			let lanes_manager = LanesManagerOf::<TestRuntime, ()>::new();
			lanes_manager
				.any_state_inbound_lane(locations.lane_id)
				.unwrap()
				.set_state(LaneState::Opened);
			assert_noop!(
				XcmOverBridge::close_bridge(
					origin.clone(),
					locations.bridge_destination_universal_location.clone(),
					0,
				),
				Error::<TestRuntime, ()>::CannotCloseOpenedLane,
			);
			lanes_manager
				.any_state_inbound_lane(locations.lane_id)
				.unwrap()
				.set_state(LaneState::Closing);

			lanes_manager
				.any_state_outbound_lane(locations.lane_id)
				.unwrap()
				.set_state(LaneState::Opened);
			assert_noop!(
				XcmOverBridge::close_bridge(
					origin,
					locations.bridge_destination_universal_location,
					0,
				),
				Error::<TestRuntime, ()>::CannotCloseOpenedLane,
			);
		});
	}

	#[test]
	fn close_bridge_works() {
		run_test(|| {
			let origin = AllowedOpenBridgeOrigin::parent_relay_chain_origin();
			let (bridge, locations, may_close_at) = mock_closing_bridge_from(origin.clone());
			System::set_block_number(may_close_at);

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
				locations.bridge_destination_universal_location.clone(),
				16,
			),);

			// as a result, the bridge and lanes are switched to the `Closed` state, some messages
			// are pruned, but funds are not unreserved
			let lanes_manager = LanesManagerOf::<TestRuntime, ()>::new();
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
					}),
					topics: vec![],
				}),
			);

			// now call the `close_bridge` again, which will only partially prune messages
			assert_ok!(XcmOverBridge::close_bridge(
				origin.clone(),
				locations.bridge_destination_universal_location.clone(),
				8,
			),);

			// nothing is changed (apart from the pruned messages)
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
					}),
					topics: vec![],
				}),
			);

			// now call the `close_bridge` again that will prune all remaining messages and the
			// bridge
			assert_ok!(XcmOverBridge::close_bridge(
				origin.clone(),
				locations.bridge_destination_universal_location.clone(),
				9,
			),);

			// there's no traces of bridge in the runtime storage and funds are unreserved
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
			let (_, locations) = mock_open_bridge_from(origin.clone());

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
	fn report_misbehavior_fails_for_closed_bridge() {
		run_test(|| {
			let origin = AllowedOpenBridgeOrigin::parent_relay_chain_origin();
			let (_, locations) = mock_open_bridge_from(origin.clone());

			// report valid misbehavior fails, because the bridge is not registered
			for _ in 0..=TestBridgeLimits::get().max_queued_outbound_messages {
				enqueue_message(locations.lane_id);
			}

			Bridges::<TestRuntime, ()>::mutate_extant(locations.lane_id, |bridge| {
				bridge.state = BridgeState::Closed;
			});
			assert_noop!(
				XcmOverBridge::report_misbehavior(
					RuntimeOrigin::signed([128u8; 32].into()),
					locations.lane_id,
					BridgeMisbehavior::TooManyQueuedOutboundMessages,
				),
				Error::<TestRuntime, ()>::BridgeAlreadyClosed,
			);
		});
	}

	#[test]
	fn report_misbehavior_allowrd_for_closing_bridges() {
		run_test(|| {
			let origin = AllowedOpenBridgeOrigin::parent_relay_chain_origin();
			let (_, locations) = mock_open_bridge_from(origin.clone());

			// report valid misbehavior fails, because the bridge is not registered
			for _ in 0..=TestBridgeLimits::get().max_queued_outbound_messages {
				enqueue_message(locations.lane_id);
			}

			Bridges::<TestRuntime, ()>::mutate_extant(locations.lane_id, |bridge| {
				bridge.state = BridgeState::Closing(100);
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
			let (_, locations) = mock_open_bridge_from(origin.clone());

			// report valid misbehavior fails, because the outbound lane is not registered
			for _ in 0..=TestBridgeLimits::get().max_queued_outbound_messages {
				enqueue_message(locations.lane_id);
			}

			LanesManagerOf::<TestRuntime, ()>::new()
				.outbound_lane(locations.lane_id)
				.unwrap()
				.purge();
			assert_noop!(
				XcmOverBridge::report_misbehavior(
					RuntimeOrigin::signed([128u8; 32].into()),
					locations.lane_id,
					BridgeMisbehavior::TooManyQueuedOutboundMessages,
				),
				Error::<TestRuntime, ()>::UnknownOutboundLane,
			);
		});
	}

	#[test]
	fn report_misbehavior_fails_for_closed_outbound_lane() {
		run_test(|| {
			let origin = AllowedOpenBridgeOrigin::parent_relay_chain_origin();
			let (_, locations) = mock_open_bridge_from(origin.clone());

			// report valid misbehavior fails, because the outbound lane is closed
			for _ in 0..=TestBridgeLimits::get().max_queued_outbound_messages {
				enqueue_message(locations.lane_id);
			}

			LanesManagerOf::<TestRuntime, ()>::new()
				.outbound_lane(locations.lane_id)
				.unwrap()
				.set_state(LaneState::Closed);
			assert_noop!(
				XcmOverBridge::report_misbehavior(
					RuntimeOrigin::signed([128u8; 32].into()),
					locations.lane_id,
					BridgeMisbehavior::TooManyQueuedOutboundMessages,
				),
				Error::<TestRuntime, ()>::ClosedOutboundLane,
			);
		});
	}

	#[test]
	fn report_too_many_queued_outbound_messages_fails() {
		run_test(|| {
			let origin = AllowedOpenBridgeOrigin::parent_relay_chain_origin();
			let (_, locations) = mock_open_bridge_from(origin.clone());

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
			let (bridge, locations) = mock_open_bridge_from(origin.clone());
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

			// check that the bridge and its lanes are in the Closing/Closed state
			let lanes_manager = LanesManagerOf::<TestRuntime, ()>::new();
			let may_close_at = 1 + BridgeCloseDelay::get();
			assert_eq!(
				Bridges::<TestRuntime, ()>::get(locations.lane_id).map(|b| b.state),
				Some(BridgeState::Closing(may_close_at))
			);
			assert_eq!(
				lanes_manager.inbound_lane(locations.lane_id).unwrap().state(),
				LaneState::Closing
			);
			assert_eq!(
				lanes_manager.any_state_outbound_lane(locations.lane_id).unwrap().state(),
				LaneState::Closed
			);

			// check that reporter balance is increased, bridge owner balance is decreased and
			// we have saved this fact in the bridge meta
			assert_eq!(Balances::free_balance(&bridge.bridge_owner_account), free_balance);
			assert_eq!(
				Balances::reserved_balance(&bridge.bridge_owner_account),
				reserved_balance - Penalty::get()
			);
			assert_eq!(
				Balances::free_balance(&reporter),
				ExistentialDeposit::get() + Penalty::get()
			);
			assert_eq!(Balances::reserved_balance(&reporter), 0);
			assert_eq!(
				Bridges::<TestRuntime, ()>::get(locations.lane_id).map(|b| b.reserve),
				Some(reserved_balance - Penalty::get())
			);
		})
	}
}
/*
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
*/
