// Copyright (C) Parity Technologies (UK) Ltd.
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

use crate::{DeliveryFeeFactor, MINIMAL_DELIVERY_FEE_FACTOR};

use frame_benchmarking::v2::*;
use frame_support::traits::Hooks;
use sp_runtime::traits::Zero;

/// Pallet we're benchmarking here.
pub struct Pallet<T: Config<I>, I: 'static = ()>(crate::Pallet<T, I>);

/// Trait that must be implemented by runtime to be able to benchmark pallet properly.
pub trait Config<I: 'static>: crate::Config<I> {
	/// Fill up queue so it becomes congested.
	fn make_congested();
}

#[instance_benchmarks]
mod benchmarks {
	use super::*;

	/// Benchmark `on_initialize` when the bridge is not congested.
	#[benchmark]
	fn on_initialize_when_non_congested() {
		DeliveryFeeFactor::<T, I>::put(MINIMAL_DELIVERY_FEE_FACTOR + MINIMAL_DELIVERY_FEE_FACTOR);

		#[block]
		{
			crate::Pallet::<T, I>::on_initialize(Zero::zero());
		}
	}

	/// Benchmark `on_initialize` when the bridge is congested.
	#[benchmark]
	fn on_initialize_when_congested() {
		DeliveryFeeFactor::<T, I>::put(MINIMAL_DELIVERY_FEE_FACTOR + MINIMAL_DELIVERY_FEE_FACTOR);
		T::make_congested();

		#[block]
		{
			crate::Pallet::<T, I>::on_initialize(Zero::zero());
		}
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::TestRuntime);
}
