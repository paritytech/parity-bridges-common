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
//!    with the `request_bridge_with` call. The `BridgePayment` amount is reserved
//!    on the sovereign account of sibling parachain;
//!
//! 4) at the other side of the bridge, the same thing (1, 2, 3) happens. Parachains that
//!    need to connect over the bridge need to coordinate the moment when they start sending
//!    messages over the bridge. Otherwise they may lose funds and/or messages;
//!
//! 5) when the bridge is opened, the pallet keeps track of bridge rules violations. If
//!    seomthing goes wrong with the bridge, the bridge closed and reserved amounts are
//!    withdrawn from sovereign accounts (TODO!!!);
//!
//! 6) when either side wants to close the bridge, it sends the XCM `Transact` instruction
//!    with the `close_bridge_with` call. The outbound lane state is immediately changed
//!    to the `Closed`. It means that the messages pallet will start to reject all outbound
//!    messages. However, we give some time for queued messages (at both sides of the bridge)
//!    to be delivered. So inbound lane closes only after `BridgeCloseTimeout` blocks. The
//!    reserved amount is also unreserved after this timeout.

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
	}

	/// An alias for the associated messages pallet.
	type MessagesPalletOf<T, I> = pallet_bridge_messages::Pallet::<T, <T as Config<I>>::BridgeMessagesPalletInstance>;

	#[pallet::pallet]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::call]
	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		/// Request for opening a bridge between two locations.
		///
		/// The caller must be a sibling parachain, so the XCMP channel between bridge hub and this
		/// parachain must already be opened before. The `remote_destination` must be a destination
		/// within the consensus of the `T::BridgedNetworkId` network.
		///
		/// The `BridgePayment` amount is reserved on the sibling parachain account. This reserve
		/// is unreserved after bridge is closed or if request is canceled.
		///
		/// This call registers bridge open request. To actually open it, it must be approved by
		/// the privileged origin.
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::zero())]
		pub fn request_bridge_with(
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
					None => Ok(Bridge { state: LaneState::Opened, payment }),
				},
			)?;

			// insert new lane in the opened status. Hopefully, we'll never see the `*LaneAlreadyRegistered`
			// error - it'll mean that the storage is corrupted in some way.
			MessagesPalletOf::<T, I>::create_inbound_lane(lane_id)
				.map_err(|_| Error::<T, I>::InboundLaneAlreadyRegistered)?;
			MessagesPalletOf::<T, I>::create_outbound_lane(lane_id)
				.map_err(|_| Error::<T, I>::OutboundLaneAlreadyRegistered)?;

			Ok(())
		}

		/// Close registered bridge.
		///
		/// This call cannot be used to close already opened bridges - the bridge must be closed.
		#[pallet::call_index(1)]
		#[pallet::weight(Weight::zero())]
		pub fn cancel_bridge_request(
			origin: OriginFor<T>,
			bridge_destination_location: Box<MultiLocation>, // TODO: versioned?
		) -> DispatchResult {
			// check that the origin is able to open bridges
			let bridge_origin_location = T::UniversalLocation::get()
				.within_global(AllowedOpenBridgeOrigin::ensure_origin(origin)?);

			// get bridge metadata
			let lane_id = LaneId::new(bridge_origin_location, bridge_destination_location);
			let bridge = BridgeMetadata::<T, I>::try_mutate_exists(
				lane_id,
				|bridge| match bridge {
					Some(bridge) if bridge.state == LaneState::Closed => Err(Error::<T, I>::AlreadyClosedBridge),
					Some(mut bridge) => {
						MessagesPalletOf::<T, I>::close_outbound_lane(lane_id)
							.map_err(|_| Error::<T, I>::UnknownOutboundLane);
						bridge.state = LaneState::Closed;
						Ok(bridge)
					},
					None => Err(Error::<T, I>:UnknownBridge),
				}
			)?;
		}

		/// Open previously requested bridge between sibling parachain and some parachain at the
		/// other side of the bridge.
		///
		/// The caller must be a parent relay chain.
		#[pallet::call_index(2)]
		#[pallet::weight(Weight::zero())]
		pub fn open_bridge(
			origin: OriginFor<T>,
			sibling_location: Box<MultiLocation>, // TODO: versioned?
			remote_destination: Box<MultiLocation>, // TODO: versioned?
		) -> DispatchResult {
			// request must be approved by the relay chain governance
			let _ = ensure_relay(<T as Config<I>>::RuntimeOrigin::from(origin))?; // TODO: is this a relay chain governance origin???

			// we don't do any additional checks here, because if there's registered
			// request to open a bridge, we know that locations are correct
			let lane_id = LaneId::new(sibling_location, remote_destination);

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
