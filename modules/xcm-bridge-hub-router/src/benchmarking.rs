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

//! XCM bridge hub router pallet benchmarks.

#![cfg(feature = "runtime-benchmarks")]

use crate::{
	Bridges, CongestionFeeFactor, RelievingBridges, SuspendedMessages, ToBridgeHubTicket,
	MINIMAL_DELIVERY_FEE_FACTOR,
};

use bp_xcm_bridge_hub_router::BridgeId;
use codec::Decode;
use frame_benchmarking::benchmarks_instance_pallet;
use frame_support::traits::Hooks;
use sp_runtime::traits::Zero;
use xcm::latest::prelude::*;

/// Pallet we're benchmarking here.
pub struct Pallet<T: Config<I>, I: 'static = ()>(crate::Pallet<T, I>);

/// Trait that must be implemented by runtime to be able to benchmark pallet properly.
pub trait Config<I: 'static>: crate::Config<I> {
	/// Fill up queue so it becomes congested.
	fn make_congested();
	/// Prepare a valid ticket for `Self::ToBridgeHubSender`.
	fn to_bridge_hub_ticket() -> ToBridgeHubTicket<Self, I>;
}

benchmarks_instance_pallet! {
	where_clause {
		where
			ToBridgeHubTicket<T, I>: Decode,
	}

	on_initialize_when_non_congested {
		CongestionFeeFactor::<T, I>::put(MINIMAL_DELIVERY_FEE_FACTOR + MINIMAL_DELIVERY_FEE_FACTOR);
	}: {
		crate::Pallet::<T, I>::on_initialize(Zero::zero())
	}

	on_initialize_when_congested {
		CongestionFeeFactor::<T, I>::put(MINIMAL_DELIVERY_FEE_FACTOR + MINIMAL_DELIVERY_FEE_FACTOR);
		T::make_congested();
	}: {
		crate::Pallet::<T, I>::on_initialize(Zero::zero())
	}

	to_bridge_hub_deliver_weight {
		let ticket = T::to_bridge_hub_ticket();
	}: {
		T::ToBridgeHubSender::deliver(ticket).expect("Invalid ticket")
	}

	bridge_read_weight {
		// since we are using `MaxEncodedLen` approach, we don't care about actual value of `Bridge`
	}: {
		Bridges::<T, I>::get(BridgeId::new(&Here.into(), &Here.into()))
	}

	relieving_bridges_read_weight {
		// since we are using `MaxEncodedLen` approach, we don't care about actual value of `RelievingBridges`
	}: {
		RelievingBridges::<T, I>::get()
	}

	suspended_message_read_weight {
		// since we are using `MaxEncodedLen` approach, we don't care about actual value of `SuspendedMessage`
	}: {
		SuspendedMessages::<T, I>::get(BridgeId::new(&Here.into(), &Here.into()), 1)
	}
}
