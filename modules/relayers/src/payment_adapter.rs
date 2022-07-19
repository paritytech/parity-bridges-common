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

//! Code that allows relayers pallet to be used as a delivery+dispatch payment mechanism
//! for the messages pallet.

use crate::{Config, RelayerRewards};

use bp_messages::{
	source_chain::{MessageDeliveryAndDispatchPayment, RelayersRewards},
	LaneId, MessageKey, MessageNonce, UnrewardedRelayer,
};
use frame_support::traits::Get;
use num_traits::SaturatingAdd;
use pallet_bridge_messages::OutboundMessages;
use sp_arithmetic::traits::{Bounded, Saturating, Zero};
use sp_std::{collections::vec_deque::VecDeque, marker::PhantomData, ops::RangeInclusive};

/// Adapter that allows relayers pallet to be used as a delivery+dispatch payment mechanism
/// for the messages pallet.
pub struct MessageDeliveryAndDispatchPaymentAdapter<T, MessagesInstance, GetConfirmationFee>(
	PhantomData<(T, MessagesInstance, GetConfirmationFee)>,
);

impl<T, MessagesInstance, GetConfirmationFee>
	MessageDeliveryAndDispatchPayment<T::Origin, T::AccountId, T::Reward>
	for MessageDeliveryAndDispatchPaymentAdapter<T, MessagesInstance, GetConfirmationFee>
where
	T: Config + pallet_bridge_messages::Config<MessagesInstance, OutboundMessageFee = T::Reward>,
	MessagesInstance: 'static,
	GetConfirmationFee: Get<T::Reward>,
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
		lane_id: bp_messages::LaneId,
		messages_relayers: VecDeque<bp_messages::UnrewardedRelayer<T::AccountId>>,
		confirmation_relayer: &T::AccountId,
		received_range: &RangeInclusive<bp_messages::MessageNonce>,
	) {
		let relayers_rewards = calc_relayers_rewards::<T, MessagesInstance>(
			lane_id,
			messages_relayers,
			received_range,
		);
		if !relayers_rewards.is_empty() {
			pay_relayers_rewards::<T>(
				confirmation_relayer,
				relayers_rewards,
				GetConfirmationFee::get(),
			);
		}
	}
}

/// Calculate the relayers rewards
fn calc_relayers_rewards<T, MessagesInstance>(
	lane_id: LaneId,
	messages_relayers: VecDeque<UnrewardedRelayer<T::AccountId>>,
	received_range: &RangeInclusive<MessageNonce>,
) -> RelayersRewards<T::AccountId, T::OutboundMessageFee>
where
	T: frame_system::Config + pallet_bridge_messages::Config<MessagesInstance>,
	MessagesInstance: 'static,
{
	// remember to reward relayers that have delivered messages
	// this loop is bounded by `T::MaxUnrewardedRelayerEntriesAtInboundLane` on the bridged chain
	let mut relayers_rewards: RelayersRewards<_, T::OutboundMessageFee> = RelayersRewards::new();
	for entry in messages_relayers {
		let nonce_begin = sp_std::cmp::max(entry.messages.begin, *received_range.start());
		let nonce_end = sp_std::cmp::min(entry.messages.end, *received_range.end());

		// loop won't proceed if current entry is ahead of received range (begin > end).
		// this loop is bound by `T::MaxUnconfirmedMessagesAtInboundLane` on the bridged chain
		let mut relayer_reward = relayers_rewards.entry(entry.relayer).or_default();
		for nonce in nonce_begin..nonce_end + 1 {
			let message_data =
				OutboundMessages::<T, MessagesInstance>::get(MessageKey { lane_id, nonce });
			if let Some(message_data) = message_data {
				relayer_reward.reward = relayer_reward.reward.saturating_add(&message_data.fee);
				relayer_reward.messages += 1;
			} else {
				log::trace!(
					target: "T::bridge-relayers",
					"Missing delivered message from the storage: {:?}/{}",
					lane_id,
					nonce,
				);
			}
		}
	}
	relayers_rewards
}

// Update rewards to given relayers, optionally rewarding confirmation relayer.
fn pay_relayers_rewards<T: Config>(
	confirmation_relayer: &T::AccountId,
	relayers_rewards: RelayersRewards<T::AccountId, T::Reward>,
	confirmation_fee: T::Reward,
) {
	// reward every relayer except `confirmation_relayer`
	let mut confirmation_relayer_reward = T::Reward::zero();
	for (relayer, reward) in relayers_rewards {
		let mut relayer_reward = reward.reward;

		if relayer != *confirmation_relayer {
			// If delivery confirmation is submitted by other relayer, let's deduct confirmation fee
			// from relayer reward.
			//
			// If confirmation fee has been increased (or if it was the only component of message
			// fee), then messages relayer may receive zero reward.
			let mut confirmation_reward = T::Reward::try_from(reward.messages)
				.unwrap_or_else(|_| Bounded::max_value())
				.saturating_mul(confirmation_fee);
			if confirmation_reward > relayer_reward {
				confirmation_reward = relayer_reward;
			}
			relayer_reward = relayer_reward.saturating_sub(confirmation_reward);
			confirmation_relayer_reward =
				confirmation_relayer_reward.saturating_add(confirmation_reward);
		} else {
			// If delivery confirmation is submitted by this relayer, let's add confirmation fee
			// from other relayers to this relayer reward.
			confirmation_relayer_reward = confirmation_relayer_reward.saturating_add(reward.reward);
			continue
		}

		pay_relayer_reward::<T>(&relayer, relayer_reward);
	}

	// finally - pay reward to confirmation relayer
	pay_relayer_reward::<T>(confirmation_relayer, confirmation_relayer_reward);
}

/// Remember that the relayer shall be paid reward.
fn pay_relayer_reward<T: Config>(relayer: &T::AccountId, reward: T::Reward) {
	if reward.is_zero() {
		return
	}

	RelayerRewards::<T>::mutate(relayer, |old_reward: &mut Option<T::Reward>| {
		let new_reward = old_reward.unwrap_or_else(Zero::zero).saturating_add(reward);
		log::trace!(
			target: "T::bridge-relayers",
			"Relayer {:?} can now claim reward: {:?}",
			relayer,
			new_reward,
		);
		*old_reward = Some(new_reward);
	});
}

#[cfg(test)]
mod tests {
	/* tests from instant payments:

	use super::*;
	use crate::mock::{
		run_test, AccountId as TestAccountId, Balance as TestBalance, Origin, TestRuntime,
	};
	use bp_messages::source_chain::RelayerRewards;

	type Balances = pallet_balances::Pallet<TestRuntime>;

	const RELAYER_1: TestAccountId = 1;
	const RELAYER_2: TestAccountId = 2;
	const RELAYER_3: TestAccountId = 3;
	const RELAYERS_FUND_ACCOUNT: TestAccountId = crate::mock::ENDOWED_ACCOUNT;

	fn relayers_rewards() -> RelayersRewards<TestAccountId, TestBalance> {
		vec![
			(RELAYER_1, RelayerRewards { reward: 100, messages: 2 }),
			(RELAYER_2, RelayerRewards { reward: 100, messages: 3 }),
		]
		.into_iter()
		.collect()
	}

	#[test]
	fn pay_delivery_and_dispatch_fee_fails_on_non_zero_fee_and_unknown_payer() {
		frame_support::parameter_types! {
			const GetConfirmationFee: TestBalance = 0;
		};

		run_test(|| {
			let result = InstantCurrencyPayments::<
				TestRuntime,
				(),
				Balances,
				GetConfirmationFee,
			>::pay_delivery_and_dispatch_fee(
				&Origin::root(),
				&100,
				&RELAYERS_FUND_ACCOUNT,
			);
			assert_eq!(result, Err(NON_ZERO_MESSAGE_FEE_CANT_BE_PAID_BY_NONE));
		});
	}

	#[test]
	fn pay_delivery_and_dispatch_succeeds_on_zero_fee_and_unknown_payer() {
		frame_support::parameter_types! {
			const GetConfirmationFee: TestBalance = 0;
		};

		run_test(|| {
			let result = InstantCurrencyPayments::<
				TestRuntime,
				(),
				Balances,
				GetConfirmationFee,
			>::pay_delivery_and_dispatch_fee(
				&Origin::root(),
				&0,
				&RELAYERS_FUND_ACCOUNT,
			);
			assert!(result.is_ok());
		});
	}

	#[test]
	fn confirmation_relayer_is_rewarded_if_it_has_also_delivered_messages() {
		run_test(|| {
			pay_relayers_rewards::<Balances, _>(
				&RELAYER_2,
				relayers_rewards(),
				&RELAYERS_FUND_ACCOUNT,
				10,
			);

			assert_eq!(Balances::free_balance(&RELAYER_1), 80);
			assert_eq!(Balances::free_balance(&RELAYER_2), 120);
		});
	}

	#[test]
	fn confirmation_relayer_is_rewarded_if_it_has_not_delivered_any_delivered_messages() {
		run_test(|| {
			pay_relayers_rewards::<Balances, _>(
				&RELAYER_3,
				relayers_rewards(),
				&RELAYERS_FUND_ACCOUNT,
				10,
			);

			assert_eq!(Balances::free_balance(&RELAYER_1), 80);
			assert_eq!(Balances::free_balance(&RELAYER_2), 70);
			assert_eq!(Balances::free_balance(&RELAYER_3), 50);
		});
	}

	#[test]
	fn only_confirmation_relayer_is_rewarded_if_confirmation_fee_has_significantly_increased() {
		run_test(|| {
			pay_relayers_rewards::<Balances, _>(
				&RELAYER_3,
				relayers_rewards(),
				&RELAYERS_FUND_ACCOUNT,
				1000,
			);

			assert_eq!(Balances::free_balance(&RELAYER_1), 0);
			assert_eq!(Balances::free_balance(&RELAYER_2), 0);
			assert_eq!(Balances::free_balance(&RELAYER_3), 200);
		});
	}*/
}