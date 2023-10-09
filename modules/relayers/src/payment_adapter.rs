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
	source_chain::{DeliveryConfirmationPayments, RelayersRewards},
	target_chain::DeliveryPayments,
	LaneId, MessageNonce,
};
use bp_relayers::{RewardsAccountOwner, RewardsAccountParams};
use bp_runtime::Chain;
use frame_support::{sp_runtime::SaturatedConversion, weights::Weight};
use sp_arithmetic::traits::{Saturating, Zero};
use sp_std::{collections::vec_deque::VecDeque, marker::PhantomData, ops::RangeInclusive};

/// Adapter that allows relayers pallet to be used as a delivery+dispatch payment mechanism
/// for the messages pallet.
pub struct DeliveryConfirmationPaymentsAdapter<T, MI>(PhantomData<(T, MI)>);

impl<T, MI> DeliveryConfirmationPayments<T::AccountId>
	for DeliveryConfirmationPaymentsAdapter<T, MI>
where
	T: Config + pallet_bridge_messages::Config<MI>,
	MI: 'static,
{
	type Error = &'static str;

	fn pay_reward(
		lane_id: LaneId,
		messages_relayers: VecDeque<bp_messages::UnrewardedRelayer<T::AccountId>>,
		confirmation_relayer: &T::AccountId,
		received_range: &RangeInclusive<bp_messages::MessageNonce>,
	) -> MessageNonce {
		let relayers_rewards =
			bp_messages::calc_relayers_rewards::<T::AccountId>(messages_relayers, received_range);
		let rewarded_relayers = relayers_rewards.len();

		register_relayers_rewards::<T>(
			confirmation_relayer,
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

impl<T, MI> DeliveryPayments<T::AccountId> for DeliveryConfirmationPaymentsAdapter<T, MI>
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

				// if relayer is not willoing to work on that lane anymore, we don't want to do
				// anything here
				let wants_to_work_on_lane = Pallet::<T>::registered_relayer(&relayer)
					.map(|registration| registration.lanes().contains(&lane_id))
					.unwrap_or(false);
				if wants_to_work_on_lane {
					return Err(())
				}

				if !lane_relayers.next_set_try_push(relayer, *relayer_in_active_set.reward()) {
					return Err(())
				}
			}

			Ok(())
		});
	}
}

// Update rewards to given relayers, optionally rewarding confirmation relayer.
fn register_relayers_rewards<T: Config>(
	_confirmation_relayer: &T::AccountId,
	relayers_rewards: RelayersRewards<T::AccountId>,
	reward_account: RewardsAccountParams,
) {
	for (relayer, messages) in relayers_rewards {
		// TODO: THIS IS WRONG - `LaneRelayers` is for relayers, which are delivering messages from BRIDGED to THIS
		// chain and `register_relayers_rewards` is called for relayers that are delivering messages from THIS
		// to BRIDGED chain
		let message_delivery_reward = LaneRelayers::<T>::get(reward_account.lane_id())
			.and_then(|lane_relayers| {
				lane_relayers
					.active_relayers()
					.iter()
					.filter(|r| *r.relayer() == relayer)
					.map(|r| *r.reward())
					.next()
			})
			.unwrap_or_else(Zero::zero); // TODO: should be default reward, specified by bridge owner OR pallet-wide-default
		let relayer_reward =
			T::Reward::saturated_from(messages).saturating_mul(message_delivery_reward);

		Pallet::<T>::register_relayer_reward(reward_account, &relayer, relayer_reward);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{mock::*, RelayerRewards};
	use bp_relayers::LaneRelayersSet;

	const RELAYER_1: ThisChainAccountId = 1;
	const RELAYER_2: ThisChainAccountId = 2;
	const RELAYER_3: ThisChainAccountId = 3;

	fn relayers_rewards() -> RelayersRewards<ThisChainAccountId> {
		vec![(RELAYER_1, 2), (RELAYER_2, 3)].into_iter().collect()
	}

	fn set_expected_rewards(relayers: &[ThisChainAccountId], reward: ThisChainBalance) {
		LaneRelayers::<TestRuntime>::mutate(
			test_reward_account_param().lane_id(),
			|maybe_lane_relayers| {
				let mut lane_relayers =
					maybe_lane_relayers.take().unwrap_or_else(|| LaneRelayersSet::empty(0));
				for relayer in relayers {
					assert!(lane_relayers.next_set_try_push(*relayer, reward));
				}
				lane_relayers.activate_next_set(1);
				*maybe_lane_relayers = Some(lane_relayers)
			},
		);
	}

	#[test]
	fn confirmation_relayer_is_rewarded_if_it_has_also_delivered_messages() {
		run_test(|| {
			set_expected_rewards(&[RELAYER_1, RELAYER_2, RELAYER_3], 50);
			register_relayers_rewards::<TestRuntime>(
				&RELAYER_2,
				relayers_rewards(),
				test_reward_account_param(),
			);

			assert_eq!(
				RelayerRewards::<TestRuntime>::get(RELAYER_1, test_reward_account_param()),
				Some(100)
			);
			assert_eq!(
				RelayerRewards::<TestRuntime>::get(RELAYER_2, test_reward_account_param()),
				Some(150)
			);
		});
	}

	#[test]
	fn confirmation_relayer_is_not_rewarded_if_it_has_not_delivered_any_messages() {
		run_test(|| {
			set_expected_rewards(&[RELAYER_1, RELAYER_2, RELAYER_3], 50);
			register_relayers_rewards::<TestRuntime>(
				&RELAYER_3,
				relayers_rewards(),
				test_reward_account_param(),
			);

			assert_eq!(
				RelayerRewards::<TestRuntime>::get(RELAYER_1, test_reward_account_param()),
				Some(100)
			);
			assert_eq!(
				RelayerRewards::<TestRuntime>::get(RELAYER_2, test_reward_account_param()),
				Some(150)
			);
			assert_eq!(
				RelayerRewards::<TestRuntime>::get(RELAYER_3, test_reward_account_param()),
				None
			);
		});
	}
}
