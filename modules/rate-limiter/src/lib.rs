// Copyright 2021 Parity Technologies (UK) Ltd.
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

//! A pallet for limiting the number of times a Call can be dispatched in a given number of time.

#![cfg_attr(not(feature = "std"), no_std)]
// Runtime-generated enums
#![allow(clippy::large_enum_variant)]

use frame_support::{dispatch::DispatchError, ensure, traits::Get};
use frame_system::ensure_signed;
use sp_runtime::traits::Header as HeaderT;
use sp_std::vec::Vec;

// #[cfg(test)]
// mod mock;

// Re-export in crate namespace for `construct_runtime!`
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The length of time over which requests should be tracked.
		#[pallet::constant]
		type WindowLength: Get<u32>;

		/// The maximum number of requests allowed in a given WindowLength.
		#[pallet::constant]
		type MaxRequests: Get<u32>;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(_n: T::BlockNumber) -> frame_support::weights::Weight {
			todo!()
		}

		fn on_finalize(_n: T::BlockNumber) {
			todo!()
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(0)]
		pub fn dispatch_call(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let _ = ensure_signed(origin)?;
			Ok(().into())
		}
	}

	#[pallet::error]
	pub enum Error<T> {
		/// There are too many requests for the current window to handle.
		TooManyRequests,
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{run_test, test_header, Origin, TestRuntime};
	use codec::Encode;
	use frame_support::{assert_err, assert_ok};

	#[test]
	fn it_works() {
		run_test(|| {})
	}
}
