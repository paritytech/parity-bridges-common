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
//!    shall hold enough funds to pay for the bridge (see `BridgePayment`);
//!
//! 3) the sibling parachain opens the bridge by sending XCM `Transact` instruction
//!    with the `open_bridge` call. The `BridgePayment` amount is reserved
//!    on the sovereign account of sibling parachain;
//!
//! 4) at the other side of the bridge, the same thing (1, 2, 3) happens. Parachains that
//!    need to connect over the bridge need to coordinate the moment when they start sending
//!    messages over the bridge. Otherwise they may lose funds and/or messages;
//!
//! 5) when the bridge is opened, anyone can watch for bridge rules (TODO!!!) violations.
//!    If something goes wrong with the bridge, he may call the `report_misbehavior` method.
//     During the call, the outbound lane is immediately `Closed`. The bridge itself switched
//!    to the `Closing` state. A `PenaltyPayment` portion of reserved funds is paid to the
//!    reporter. After `BridgeCloseDelay`, the caller may call the `close_bridge` to get his
//!    funds back;
//!
//! 6) when either side wants to close the bridge, it sends the XCM `Transact` instruction
//!    with the `request_bridge_closure` call. The bridge stays opened for `BridgeCloseDelay`
//!    blocks. This delay exists to give both chain users enough time to properly close
//!    their bridge - e.g. withdraw funds from the bridged chain and so on. After this
//!    delay passes, either side may send another XCM `Transact` instruction with the
//!    `close_bridge` call to actually close the bridge (at its side) and get its reserved
//!    funds back.

#![cfg_attr(not(feature = "std"), no_std)]

use bp_runtime::BalanceOf;
use frame_support::traits::ReservableCurrency;
use frame_system::Config as SystemConfig;
use pallet_bridge_messages::{Config as BridgeMessagesConfig, ThisChainOf};
use xcm::prelude::*;
use xcm_builder::ensure_is_remote;
use xcm_executor::traits::ConvertLocation;

pub use pallet::*;

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
		/// Runtime's universal location.
		type UniversalLocation: Get<InteriorMultiLocation>;
		/// Bridged network id.
		type BridgedNetworkId: Get<NetworkId>; // TODO: it must be a part of header chain instead of `ChainId`
		/// Associated messages pallet instance that bridges us with the
		/// `BridgedNetworkId` consensus.
		type BridgeMessagesPalletInstance: 'static;

		/// A set of XCM locations within local consensus system that are allowed to open
		/// bridges with remote destinations.
		type AllowedOpenBridgeOrigin: EnsureOrigin<<Self as SystemConfig>::RuntimeOrigin, Success = MultiLocation>;
		/// A converter between a multi-location and a sovereign account.
		type BridgeOriginAccountIdConverter: ConvertLocation<Self::AccountId>;

		/// Amount of this chain native tokens that is reserved on the sibling parachain account
		/// when bridge open request is registered.
		type BridgePayment: Get<BalanceOf<ThisChainOf<Self, Self::BridgeMessagesPalletInstance>>>;
		/// Currency used to paid for bridge registration.
		type NativeCurrency: ReservableCurrency;

		/// Delay before bridge is closed.
		type BridgeCloseDelay: Get<Self::BlockNumber>;
	}

	/// An alias for the associated messages pallet.
	type MessagesPalletOf<T, I> = pallet_bridge_messages::Pallet::<T, <T as Config<I>>::BridgeMessagesPalletInstance>;

	#[pallet::pallet]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::call]
	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		/// Open a bridge between two locations.
		///
		/// The caller must be within the `AllowedOpenBridgeOrigin` filter (presumably: a sibling
		/// parachain or a parent relay chain). The `bridge_destination_location` must be a
		/// destination within the consensus of the `T::BridgedNetworkId` network.
		///
		/// The `BridgePayment` amount is reserved on the caller account. This reserve
		/// is unreserved after bridge is closed.
		///
		/// The states after this call: bridge is `Opened`, outbound lane is `Opened`, inbound lane
		/// is `Opened`.
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::zero())]
		pub fn open_bridge(
			origin: OriginFor<T>,
			bridge_destination_location: Box<MultiLocation>, // TODO: versioned?
		) -> DispatchResult {
			// check that the origin is able to open bridges
			let bridge_origin_location = T::UniversalLocation::get()
				.within_global(AllowedOpenBridgeOrigin::ensure_origin(origin)?);

			// ensure that we support the remote destination
			let (remote_network, _) = ensure_is_remote(this_location, *bridge_destination_location)
				.map_err(|_| Error::<T, I>::LocalDestination)?;
			ensure!(
				remote_network == T::BridgedNetworkId::get(),
				Error::<T, I>::InvalidRemoteDestination
			);

			// reserve balance on the parachain sovereign account
			let bridge_origin_account = T::BridgeOriginAccountIdConverter::convert_location(&bridge_origin_location);
			let payment = T::BridgePayment::get();
			T::NativeCurrency::reserve(&bridge_origin_account, payment.clone())
				.map_err(|_| Error::<T, I>::FailedToReserveBridgePayment)?;

			// we know that the `bridge_destination_location` starts from the `GlobalConsensus`
			// (see `ensure_is_remote`) and we know that the `UniversalLocation` is also within the
			// `GlobalConsensus` (given the proper pallet configuration). So we know that the lane id
			// will be the same on both ends of the bridge
			let lane_id = LaneId::new(bridge_origin_location, bridge_destination_location);

			// save bridge metadata
			Bridges::<T, I>::try_mutate(
				lane_id,
				|bridge| match bridge {
					Some(_) => Err(Error::<T, I>::BridgeAlreadyRegistered),
					None => Ok(Bridge { state: BridgeState::Opened, payment }),
				},
			)?;

			// insert new lane in the opened status. Hopefully, we'll never see the `*LaneAlreadyRegistered`
			// error - it'll mean that the storage is corrupted in some way.
			MessagesPalletOf::<T, I>::create_inbound_lane(lane_id, LaneState::Opened)
				.map_err(|_| Error::<T, I>::InboundLaneAlreadyRegistered)?;
			MessagesPalletOf::<T, I>::create_outbound_lane(lane_id, LaneState::Opened)
				.map_err(|_| Error::<T, I>::OutboundLaneAlreadyRegistered)?;

			Ok(())
		}

		/// Request previously opened bridge closure.
		///
		/// Can only be called by the "owner" of this side of the bridge. Both inbound and outbound
		/// lanes at this side of the bridge are immediately switched to `Closing` state. Lanes may
		/// still send/receive new messages, however this state is reported to the other side of
		/// the bridge, given that relays are active. When this happens, lanes at the other side also
		/// switching their state to `Closing`.
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
			bridge_destination_location: Box<MultiLocation>, // TODO: versioned?
		) -> DispatchResult {
			// check that the origin is able to close bridge
			let bridge_origin_location = T::UniversalLocation::get()
				.within_global(AllowedOpenBridgeOrigin::ensure_origin(origin)?);

			// update bridge metadata
			let lane_id = LaneId::new(bridge_origin_location, bridge_destination_location);
			let bridge = BridgeMetadata::<T, I>::try_mutate_exists(
				lane_id,
				|bridge| match bridge {
					Some(bridge) if bridge.state != BridgeState::Opened => Err(Error::<T, I>::AlreadyClosedBridge),
					Some(mut bridge) => {
						let may_close_at = T::block_number().saturating_add(T::BridgeCloseDelay::get());
						bridge.state = LaneState::Closing(may_close_at);
						Ok(bridge)
					},
					None => Err(Error::<T, I>::UnknownBridge),
				}
			)?;

			// update lanes state. Under normal circumstances, following calls shall never fail
			MessagesPalletOf::<T, I>::update_inbound_lane_state(lane_id, Some(LaneState::Opened), LaneState::Closing)
				.map_err(|lane_state| match lane_state {
					Some(_) => Error::<T, I>::InboundLaneAlreadyClosed,
					None => Error::<T, I>::UnknownInboundLane,
				})?;
			MessagesPalletOf::<T, I>::update_outbound_lane_state(lane_id, Some(LaneState::Opened), LaneState::Closing)
				.map_err(|lane_state| match lane_state {
					Some(_) => Error::<T, I>::OutboundLaneAlreadyClosed,
					None => Error::<T, I>::UnknownOutboundLane,
				})?;

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
			sibling_location: Box<MultiLocation>, // TODO: versioned?
			remote_destination: Box<MultiLocation>, // TODO: versioned?
			mut may_prune_messages: MessageNonce,
		) -> DispatchResult {
			// check that the origin is able to close bridge
			let bridge_origin_location = T::UniversalLocation::get()
				.within_global(AllowedOpenBridgeOrigin::ensure_origin(origin)?);

			// TODO: may do refund here, if bridge/lanes are already closed + for messages that are not pruned

			// update bridge metadata - this also guarantees that the bridge is in the proper state
			let lane_id = LaneId::new(bridge_origin_location, bridge_destination_location);
			let bridge = BridgeMetadata::<T, I>::try_mutate_exists(
				lane_id,
				|bridge| match bridge {
					Some(bridge) if bridge.state == BridgeState::Opened => Err(Error::<T, I>::CannotCloseOpenedBridge),
					Some(bridge) => {
						bridge.state = BridgeState::Closed;
						Ok(bridge)
					},
					None => Err(Error::<T, I>:UnknownBridge),
				}
			)?;

			// now prune queued messages
			let mut outbound_lane = MessagesPalletOf::<T, I>::outbound_lane(lane_id);
			ensure!(
				matches!(outbound_lane.state(), LaneState::Closing || LaneState::Closed),
				Error::<T, I>::CannotCloseOpenedLane,
			);
			for message_nonce in outbound_lane.queued_messages() {
				if may_prune_messages == 0 {
					break;
				}

				outbound_lane.remove_message(&message_nonce);
				may_prune_messages -= 1;
			}

			// if there are outbound messages in the queue, just update states and early exit
			if !outbound_lane.queued_messages().is_empty() {
				// update lanes state. Under normal circumstances, following calls shall never fail
				MessagesPalletOf::<T, I>::update_inbound_lane_state(lane_id, None, LaneState::Closed)
					.map_err(|_| Error::<T, I>::UnknownInboundLane)?;
				MessagesPalletOf::<T, I>::update_outbound_lane_state(lane_id, None, LaneState::Closed)
					.map_err(|_| Error::<T, I>::UnknownOutboundLane)?;

				return Ok(());
			}

			// else we have pruned all messages, so lanes and the bridge itself may gone
			MessagesPalletOf::<T, I>::remove_inbound_lane(lane_id)
				.map_err(|_| Error::<T, I>::UnknownInboundLane)?;
			MessagesPalletOf::<T, I>::remove_outbound_lane(lane_id)
				.map_err(|_| Error::<T, I>::UnknownOutboundLane)?;
			BridgeMetadata::<T, I>::remove(lane_id);

			// unreserve remaining amount
			let bridge_origin_account = T::BridgeOriginAccountIdConverter::convert_location(&bridge_origin_location);
			let failed_to_unreserve = T::NativeCurrency::reserve(&bridge_origin_account, bridge.payment);
			if !failed_to_unreserve.is_zero() {
				// TODO: log
			}

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
			let reporter = ensure_signed(origin);

			match misbehavior {
				BridgeMisbehavior::TooManyQueuedMessages => {
					let outbound_lane = MessagesPalletOf::<T, I>::outbound_lane(lane_id);
					let queued_messages = outbound_lane.queued_messages().checked_len().unwrap_or(0);
					let max_queued_messages = T::MisbehaviorConfig::get().max_queued_messages;
					if queued_messages > max_queued_messages {
						Self::on_misbehavior(reporter, lane_id);
					}
				}
			}
		}
	}

	#[pallet::error]
	pub enum Error<T, I = ()> {
		/// Remote bridge destination is local to this consensus.
		CannotBridgeWithLocalDestination,
		/// Remote bridge destination is unreachable via this pallet instance.
		UnreachableBridgeDestination,
		/// The bridge origin can't pay the required amount for opening the bridge.
		FailedToReserveBridgePayment,
		/// The bridge is already registered in this pallet.
		BridgeAlreadyRegistered,
		/// The inbound lane is already registered in the associated messages pallet.
		InboundLaneAlreadyRegistered,
		/// The outbound lane is already registered in the associated messages pallet.
		OutboundLaneAlreadyRegistered,
		/// Trying to access unknown bridge.
		UnknownBridge,
		/// Trying to close already closed bridge.
		AlreadyClosedBridge,
	}

	#[pallet::genesis_config]
	#[derive(DefaultNoBound)]
	pub struct GenesisConfig<T: Config<I>, I: 'static = ()> {
		/// Opened lanes.
		///
		/// The same set of lanes must be duplicated in the genesis config of the associated
		/// messages pallet. The lanes are "opened" with zero payment registered.
		pub opened_lanes: Vec<LaneId>,
	}

	#[pallet::genesis_build]
	impl<T: Config<I>, I: 'static> GenesisBuild<T, I> for GenesisConfig<T, I> {
		fn build(&self) {
			for lane_id in &self.opened_lanes {
				LaneMetadata::<T, I>::insert(lane_id, LaneMetadata {
					payment: Zero::zero(),
				});
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn my_test() {}
}
