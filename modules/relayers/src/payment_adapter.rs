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

//! Code that allows relayers pallet to be used as a payment mechanism for the messages pallet.

use crate::{Config, LaneRelayers, Pallet};

use bp_messages::{
	source_chain::{DeliveryConfirmationPayments, RelayersRewardsAtSource},
	target_chain::DeliveryPayments,
	LaneId, MessageNonce,
};
use bp_relayers::{RewardAtSource, RewardsAccountOwner, RewardsAccountParams};
use bp_runtime::Chain;
use frame_support::weights::Weight;
use sp_arithmetic::traits::UniqueSaturatedFrom;
use sp_runtime::{
	traits::{Get, UniqueSaturatedInto},
	Saturating,
};
use sp_std::{collections::vec_deque::VecDeque, marker::PhantomData, ops::RangeInclusive};

/// Adapter that allows relayers pallet to be used as a delivery+dispatch payment mechanism
/// for the messages pallet.
///
/// This adapter uses 1:1 mapping of `RewardAtSource` to `T::Reward`. The reward for delivering
/// a single message, will never be larger than the `MaxRewardPerMessage`.
pub struct DeliveryConfirmationPaymentsAdapter<T, MI, MaxRewardPerMessage>(
	PhantomData<(T, MI, MaxRewardPerMessage)>,
);

impl<T, MI, MaxRewardPerMessage> DeliveryConfirmationPayments<T::AccountId>
	for DeliveryConfirmationPaymentsAdapter<T, MI, MaxRewardPerMessage>
where
	T: Config + pallet_bridge_messages::Config<MI>,
	MI: 'static,
	T::Reward: UniqueSaturatedFrom<RewardAtSource> + UniqueSaturatedFrom<MessageNonce>,
	MaxRewardPerMessage: Get<T::Reward>,
{
	type Error = &'static str;

	fn pay_reward(
		lane_id: LaneId,
		messages_relayers: VecDeque<bp_messages::UnrewardedRelayer<T::AccountId>>,
		_confirmation_relayer: &T::AccountId,
		received_range: &RangeInclusive<bp_messages::MessageNonce>,
	) -> MessageNonce {
		let relayers_rewards =
			bp_messages::calc_relayers_rewards_at_source::<T::AccountId, T::Reward>(
				messages_relayers,
				received_range,
				|messages, reward_per_message| {
					let reward_per_message = sp_std::cmp::min(
						MaxRewardPerMessage::get(),
						reward_per_message.unique_saturated_into(),
					);

					T::Reward::unique_saturated_from(messages).saturating_mul(reward_per_message)
				},
			);
		let rewarded_relayers = relayers_rewards.len();

		register_relayers_rewards::<T>(
			relayers_rewards,
			RewardsAccountParams::new(
				lane_id,
				T::BridgedChain::ID,
				RewardsAccountOwner::BridgedChain,
			),
		);

		rewarded_relayers as _
	}
}

impl<T, MI, MaxRewardPerMessage> DeliveryPayments<T::AccountId>
	for DeliveryConfirmationPaymentsAdapter<T, MI, MaxRewardPerMessage>
where
	T: Config + pallet_bridge_messages::Config<MI>,
	MI: 'static,
{
	type Error = &'static str;

	fn pay_reward(
		lane_id: LaneId,
		relayer: T::AccountId,
		_total_messages: MessageNonce,
		valid_messages: MessageNonce,
		_actual_weight: Weight,
	) {
		if valid_messages == 0 {
			return
		}

		let _ = LaneRelayers::<T>::try_mutate(lane_id, |maybe_lane_relayers| {
			if let Some(lane_relayers) = maybe_lane_relayers {
				// if relayer is NOT in the active set, we don't want to do anything here
				let relayer_in_active_set = lane_relayers
					.active_relayers()
					.iter()
					.filter(|r| *r.relayer() == relayer)
					.next()
					.cloned();
				let relayer_in_active_set = match relayer_in_active_set {
					Some(relayer_in_active_set) => relayer_in_active_set,
					None => return Err(()),
				};

				// if relayer is already in the active set, we don't want to do anything here
				let is_in_next_set = lane_relayers
					.next_relayers()
					.iter()
					.filter(|r| *r.relayer() == relayer)
					.next()
					.is_some();
				if is_in_next_set {
					return Err(())
				}

				// if relayer is not willing to work on that lane anymore, we don't want to do
				// anything here
				let wants_to_work_on_lane = Pallet::<T>::registered_relayer(&relayer)
					.map(|registration| registration.lanes().contains(&lane_id))
					.unwrap_or(false);
				if wants_to_work_on_lane {
					return Err(())
				}

				if !lane_relayers.next_set_try_push(relayer, relayer_in_active_set.reward()) {
					return Err(())
				}
			}

			Ok(())
		});
	}
}

// Update rewards to given relayers, optionally rewarding confirmation relayer.
fn register_relayers_rewards<T: Config>(
	relayers_rewards: RelayersRewardsAtSource<T::AccountId, T::Reward>,
	reward_account: RewardsAccountParams,
) where
	T::Reward: UniqueSaturatedFrom<RewardAtSource>,
{
	for (relayer, relayer_reward) in relayers_rewards {
		Pallet::<T>::register_relayer_reward(reward_account, &relayer, relayer_reward);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{mock::*, RelayerRewards};

	const RELAYER_1: ThisChainAccountId = 1;
	const RELAYER_2: ThisChainAccountId = 2;
	const RELAYER_3: ThisChainAccountId = 3;

	fn relayers_rewards() -> RelayersRewardsAtSource<ThisChainAccountId, ThisChainBalance> {
		vec![(RELAYER_1, 2), (RELAYER_2, 3)].into_iter().collect()
	}

	#[test]
	fn register_relayers_rewards_works() {
		run_test(|| {
			register_relayers_rewards::<TestRuntime>(
				relayers_rewards(),
				test_reward_account_param(),
			);

			assert_eq!(
				RelayerRewards::<TestRuntime>::get(RELAYER_1, test_reward_account_param()),
				Some(2)
			);
			assert_eq!(
				RelayerRewards::<TestRuntime>::get(RELAYER_2, test_reward_account_param()),
				Some(3)
			);
			assert_eq!(
				RelayerRewards::<TestRuntime>::get(RELAYER_3, test_reward_account_param()),
				None
			);
		});
	}
}
