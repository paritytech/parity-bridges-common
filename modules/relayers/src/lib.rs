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

use bp_relayers::PaymentProcedure;
use sp_std::{collections::vec_deque::VecDeque, marker::PhantomData, ops::RangeInclusive};

pub use pallet::*;

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
		type Reward: Parameter + MaxEncodedLen;
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

				Self::deposit_event(Event::<T>::RewardPaid {
					relayer: relayer.clone(),
					reward
				});
				Ok(())
			})
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Reward has been paid to the relayer.
		RewardPaid { relayer: T::AccountId, reward: T::Reward },
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
	pub type RelayerRewards<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, T::Reward, OptionQuery>;
}

/// Adapter that allows relayers pallet to be used as a delivery+dispatch payment mechanism
/// for the messages pallet.
pub struct MessageDeliveryAndDispatchPaymentAdapter<T, MessagesInstance>(PhantomData<(T, MessagesInstance)>);

impl<T, MessagesInstance> bp_messages::source_chain::MessageDeliveryAndDispatchPayment<T::Origin, T::AccountId, T::Reward> for MessageDeliveryAndDispatchPaymentAdapter<T, MessagesInstance> where
	T: Config + pallet_bridge_messages::Config<MessagesInstance>,
	MessagesInstance: 'static,
{
	type Error = &'static str;

	fn pay_delivery_and_dispatch_fee(
		_submitter: &T::Origin,
		_fee: &T::Reward,
	) -> Result<(), Self::Error> {
		// nothing shall happen here, because XCM deals with fee payment (planned to be burnt?
		// or transferred to the treasury?)
		Ok(())
	}

	fn pay_relayers_rewards(
		_lane_id: bp_messages::LaneId,
		_messages_relayers: VecDeque<bp_messages::UnrewardedRelayer<T::AccountId>>,
		_confirmation_relayer: &T::AccountId,
		_received_range: &RangeInclusive<bp_messages::MessageNonce>,
	) {
		// TODO: deal with confirmation relayer
		// TODO: read every message from the messages pallet and insert/update RelayerRewards entries
		unimplemented!("TODO")
	}
}
