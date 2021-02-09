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
use sp_runtime::traits::{Header as HeaderT, One};
use sp_std::vec::Vec;

#[cfg(test)]
mod mock;

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
		type WindowLength: Get<Self::BlockNumber>;

		/// The maximum number of requests allowed in a given WindowLength.
		#[pallet::constant]
		type MaxRequests: Get<u32>;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(_n: T::BlockNumber) -> frame_support::weights::Weight {
			// Could update `elapsed_time` here
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

			let current_block_number = <frame_system::Module<T>>::block_number();
			let elapsed_time = current_block_number - Self::current_window_start();

			dbg!(current_block_number);
			dbg!(elapsed_time);

			// Check if we've exceeded our window length since last time we ran
			let elapsed_time = if elapsed_time >= T::WindowLength::get() {
				// Bump the window start block to the next window
				<CurrentWindowStart<T>>::mutate(|start| *start += T::WindowLength::get());
				<PreviousWindowReqCount<T>>::put(Self::current_request_count());
				<CurrentWindowReqCount<T>>::put(0);

				// Need to re-calculate our elapsed time
				dbg!(current_block_number - Self::current_window_start())
			} else {
				elapsed_time
			};

			let previous_count: T::BlockNumber = Self::previous_request_count().into();
			let current_count: T::BlockNumber = Self::current_request_count().into();

			dbg!(previous_count);
			dbg!(current_count);

			let request_count: T::BlockNumber =
				previous_count * ((T::WindowLength::get() - elapsed_time) / T::WindowLength::get()) + current_count;

			dbg!(request_count);

			ensure!(
				request_count < T::MaxRequests::get().into(),
				<Error<T>>::TooManyRequests
			);

			<CurrentWindowReqCount<T>>::mutate(|count| *count += 1);

			Ok(().into())
		}
	}

	#[pallet::error]
	pub enum Error<T> {
		/// There are too many requests for the current window to handle.
		TooManyRequests,
	}

	/// The number of requests in the previous window.
	#[pallet::storage]
	#[pallet::getter(fn previous_request_count)]
	pub(super) type PreviousWindowReqCount<T: Config> = StorageValue<_, u32, ValueQuery>;

	/// The number of requests in the current window.
	#[pallet::storage]
	#[pallet::getter(fn current_request_count)]
	pub(super) type CurrentWindowReqCount<T: Config> = StorageValue<_, u32, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn current_window_start)]
	pub(super) type CurrentWindowStart<T: Config> = StorageValue<_, T::BlockNumber, ValueQuery>;
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{run_test, Origin, TestRuntime};
	use codec::Encode;
	use frame_support::{assert_err, assert_ok};

	fn next_block() {
		let current_number = frame_system::Module::<TestRuntime>::block_number();
		frame_system::Module::<TestRuntime>::set_block_number(current_number + 1);
	}

	//pub const WindowLength: u32 = 2;
	//pub const MaxRequests: u32 = 2;

	#[test]
	fn it_works() {
		run_test(|| {
			next_block();
			assert_eq!(frame_system::Module::<TestRuntime>::block_number(), 1);
		})
	}

	#[test]
	fn allows_requests_when_under_limit() {
		run_test(|| {
			next_block();
			assert_ok!(Module::<TestRuntime>::dispatch_call(Origin::signed(1)));

			next_block();
			assert_ok!(Module::<TestRuntime>::dispatch_call(Origin::signed(1)));

			next_block();
			assert_ok!(Module::<TestRuntime>::dispatch_call(Origin::signed(1)));
		})
	}

	#[test]
	fn disallows_requests_after_limit_is_hit() {
		run_test(|| {
			next_block();
			assert_ok!(Module::<TestRuntime>::dispatch_call(Origin::signed(1)));
			assert_ok!(Module::<TestRuntime>::dispatch_call(Origin::signed(1)));
			assert_err!(
				Module::<TestRuntime>::dispatch_call(Origin::signed(1)),
				<Error<TestRuntime>>::TooManyRequests
			);
		})
	}

	#[test]
	fn disallows_requests_after_limit_is_hit_across_different_blocks() {
		run_test(|| {
			next_block();
			assert_ok!(Module::<TestRuntime>::dispatch_call(Origin::signed(1)));

			next_block();
			assert_ok!(Module::<TestRuntime>::dispatch_call(Origin::signed(1)));
			assert_err!(
				Module::<TestRuntime>::dispatch_call(Origin::signed(1)),
				<Error<TestRuntime>>::TooManyRequests
			);
		})
	}

	#[test]
	fn transitions_between_windows() {
		run_test(|| {
			next_block();
			assert_ok!(Module::<TestRuntime>::dispatch_call(Origin::signed(1)));
			next_block();

			next_block();

			next_block();
			assert_ok!(Module::<TestRuntime>::dispatch_call(Origin::signed(1)));
			assert_ok!(Module::<TestRuntime>::dispatch_call(Origin::signed(1)));
		})
	}

	#[ignore]
	#[test]
	fn transitions_between_multiple_missed_windows() {
		run_test(|| {
			next_block();
			assert_ok!(Module::<TestRuntime>::dispatch_call(Origin::signed(1)));
			next_block();

			next_block();
			next_block();

			next_block();
			assert_ok!(Module::<TestRuntime>::dispatch_call(Origin::signed(1)));
			assert_ok!(Module::<TestRuntime>::dispatch_call(Origin::signed(1)));
		})
	}

	#[ignore]
	#[test]
	fn template() {
		run_test(|| {})
	}
}
