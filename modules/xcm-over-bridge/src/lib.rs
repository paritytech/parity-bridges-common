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

//! Module that allows bridge pallets to be used as a transport for XCM.
//!
//! We

#![cfg_attr(not(feature = "std"), no_std)]

use bp_messages::{InboundLaneData, LaneState, OutboundLaneData};
use bp_runtime::BalanceOf;
use cumulus_pallet_xcm::{ensure_sibling_para, Origin as CumulusOrigin};
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
		/// The runtime `Origin` type.
		type RuntimeOrigin: From<<Self as SystemConfig>::RuntimeOrigin>
			+ Into<Result<CumulusOrigin, <Self as Config<I>>::RuntimeOrigin>>;
		/// Associated messages pallet instance.
		type BridgeMessagesPalletInstance: 'static;
		/// Runtime's universal location.
		type UniversalLocation: Get<InteriorMultiLocation>;
		/// Bridged network id.
		type BridgedNetworkId: Get<NetworkId>; // TODO: it must be a part of header chain instead of `ChainId`

		/// Convert from location to account id.
		type AccountIdConverter: ConvertLocation<Self::AccountId>;
		/// Amount of this chain native tokens that is reserved on the sibling parachain account
		/// when bridge open request is registered.
		type BridgePayment: Get<BalanceOf<ThisChainOf<Self, Self::BridgeMessagesPalletInstance>>>;
		/// Currency used to paid for bridge registration.
		type NativeCurrency: ReservableCurrency;
	}

	#[pallet::pallet]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::call]
	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		/// Request for opening a bridge between two locations.
		///
		/// The caller must be a sibling parachain, so the XCMP channel between bridge hub and this
		/// parachain must already be opened before.
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::zero())]
		pub fn request_bridge_with(
			origin: OriginFor<T>,
			remote_destination: Box<MultiLocation>, // TODO: versioned?
		) -> DispatchResult {
			// we only accept requests from sibling parachains
			let sibling_parachain =
				ensure_sibling_para(<T as Config<I>>::RuntimeOrigin::from(origin))?;
			let this_location = T::UniversalLocation::get();
			let sibling_location = this_location.within_global((Parent, Parachain(1001)).into());

			// ensure that we support the remote destination
			let (remote_network, _) = ensure_is_remote(this_location, *remote_destination)
				.map_err(|_| Error::<T, I>::LocalDestination)?;
			ensure!(
				remote_network == T::BridgedNetworkId::get(),
				Error::<T, I>::InvalidRemoteDestination
			);

			// reserve balance on the parachain sovereign account
			let sibling_account = T::AccountIdConverter::convert_location(&sibling_location);
			let payment = T::BridgePayment::get();
			T::NativeCurrency::reserve(&sibling_account, payment.clone())
				.map_err(|_| Error::<T, I>::FailedToReserveBridgePayment)?;

			// insert new lane in the waiting status
			// TODO: replace the lane_id with the ordered (MultiLocation, MultiLocation) pair
			// to avoid collisions
			let bridge_id = OrderedLocationKey::new(sibling_location, remote_destination);
			let lane_id = Self::generate_lane_id(bridge_id);
			pallet_bridge_messages::Pallet::<T, T::BridgeMessagesPalletInstance>::create_lane(lane_id)
				.map_err(|_| Error::<T, I>::LaneAlreadyRegistered)?;

			// save lane metadata
			BridgeMetdata::<T, I>::try_mutate(
				bridge_id,
				|meta| match meta {
					Some(_) => Err(Error::<T, I>::LaneAlreadyRegistered),
					None => Ok(LaneMetadata {
						lane_id,
						payment,
					}),
				},
			)?;

			Ok(())
		}
	}

	#[pallet::error]
	pub enum Error<T, I = ()> {
		/// The other side of the bridge is local to this consensus.
		LocalDestination,
		/// Remote destination is unreachable via this pallet instance.
		InvalidRemoteDestination,
		/// The sibling parachain can't pay the required amount for opening the bridge.
		FailedToReserveBridgePayment,
		/// The lane is already registered.
		LaneAlreadyRegistered,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn my_test() {}
}
