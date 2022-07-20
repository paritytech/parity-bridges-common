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

//! Runtime module that is used to store relayer rewards and (in the future) to
//! coordinate relations between relayers.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

use bp_relayers::PaymentProcedure;
use sp_arithmetic::traits::AtLeast32BitUnsigned;
use sp_std::marker::PhantomData;

pub use pallet::*;
pub use payment_adapter::MessageDeliveryAndDispatchPaymentAdapter;

mod mock;
mod payment_adapter;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		/// Type of relayer reward.
		type Reward: AtLeast32BitUnsigned + Copy + Parameter + MaxEncodedLen;
		/// Pay rewards adapter.
		type PaymentProcedure: PaymentProcedure<Self::AccountId, Self::Reward>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Claim accumulated rewards.
		#[pallet::weight(0)] // TODO: weights
		pub fn claim_rewards(origin: OriginFor<T>) -> DispatchResult {
			let relayer = ensure_signed(origin)?;

			RelayerRewards::<T>::try_mutate_exists(&relayer, |maybe_reward| -> DispatchResult {
				let reward = maybe_reward.take().ok_or(Error::<T>::NoRewardForRelayer)?;
				T::PaymentProcedure::pay_reward(&relayer, reward.clone()).map_err(|e| {
					log::trace!(
						target: "runtime::bridge-relayers",
						"Failed to pay rewards to {:?}: {:?}",
						relayer,
						e,
					);
					Error::<T>::FailedToPayReward
				})?;

				Self::deposit_event(Event::<T>::RewardPaid { relayer: relayer.clone(), reward });
				Ok(())
			})
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Reward has been paid to the relayer.
		RewardPaid {
			/// Relayer account that has been rewarded.
			relayer: T::AccountId,
			/// Reward amount.
			reward: T::Reward,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// No reward can be claimed by given relayer.
		NoRewardForRelayer,
		/// Reward payment procedure has failed.
		FailedToPayReward,
	}

	/// Map of the relayer => accumulated reward.
	#[pallet::storage]
	pub type RelayerRewards<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, T::Reward, OptionQuery>;
}

#[cfg(test)]
mod tests {
	use super::*;
	use mock::*;

	use frame_support::{assert_noop, assert_ok};
	use sp_runtime::DispatchError;

	#[test]
	fn root_cant_claim_anything() {
		run_test(|| {
			assert_noop!(
				Pallet::<TestRuntime>::claim_rewards(Origin::root()),
				DispatchError::BadOrigin,
			);
		});
	}

	#[test]
	fn relayer_cant_claim_if_no_reward_exists() {
		run_test(|| {
			assert_noop!(
				Pallet::<TestRuntime>::claim_rewards(Origin::signed(REGULAR_RELAYER)),
				Error::<TestRuntime>::NoRewardForRelayer,
			);
		});
	}

	#[test]
	fn relayer_cant_claim_if_payment_procedure_fails() {
		run_test(|| {
			RelayerRewards::<TestRuntime>::insert(FAILING_RELAYER, 100);
			assert_noop!(
				Pallet::<TestRuntime>::claim_rewards(Origin::signed(FAILING_RELAYER)),
				Error::<TestRuntime>::FailedToPayReward,
			);
		});
	}

	#[test]
	fn relayer_can_claim_reward() {
		run_test(|| {
			RelayerRewards::<TestRuntime>::insert(REGULAR_RELAYER, 100);
			assert_ok!(Pallet::<TestRuntime>::claim_rewards(Origin::signed(REGULAR_RELAYER)));
			assert_eq!(RelayerRewards::<TestRuntime>::get(REGULAR_RELAYER), None);
		});
	}
}
