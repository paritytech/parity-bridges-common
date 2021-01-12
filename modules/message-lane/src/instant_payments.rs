// Copyright 2019-2020 Parity Technologies (UK) Ltd.
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

//! Implementation of `MessageDeliveryAndDispatchPayment` trait on top of `Currency` trait.
//! All payments are instant.

use bp_message_lane::{
	source_chain::{MessageDeliveryAndDispatchPayment, RelayersRewards, Sender},
	MessageNonce,
};
use codec::Encode;
use frame_support::traits::{Currency as CurrencyT, ExistenceRequirement, Get};
use num_traits::Zero;
use sp_runtime::traits::Saturating;
use sp_std::fmt::Debug;

/// Instant message payments made in given currency. Until claimed, fee is stored in special
/// 'relayers-fund' account.
///
/// Additionaly, confirmation transaction submitter (`_confirmation_relayer`) is reimbursed
/// with the confirmation rewards (part of message fee, reserved to pay for delivery confirmation).
pub struct InstantCurrencyPayments<AccountId, Currency, GetConfirmationFee> {
	_phantom: sp_std::marker::PhantomData<(AccountId, Currency, GetConfirmationFee)>,
}

impl<AccountId, Currency, GetConfirmationFee> MessageDeliveryAndDispatchPayment<AccountId, Currency::Balance>
	for InstantCurrencyPayments<AccountId, Currency, GetConfirmationFee>
where
	AccountId: Debug + Default + Encode + PartialEq,
	Currency: CurrencyT<AccountId>,
	Currency::Balance: From<MessageNonce>,
	GetConfirmationFee: Get<Currency::Balance>,
{
	type Error = &'static str;

	fn pay_delivery_and_dispatch_fee(
		submitter: &Sender<AccountId>,
		fee: &Currency::Balance,
		relayer_fund_account: &AccountId,
	) -> Result<(), Self::Error> {
		match submitter {
			Sender::Signed(submitter) => {
				Currency::transfer(submitter, relayer_fund_account, *fee, ExistenceRequirement::AllowDeath)
					.map_err(Into::into)
			}
			Sender::Root => {
				Err("Sending messages from Root account is not supported yet. See GitHub issue #559 for more.")
			}
			Sender::None => {
				Err("Sending messages from None account is not supported yet. See GitHub issue #559 for more.")
			}
		}
	}

	fn pay_relayers_rewards(
		confirmation_relayer: &AccountId,
		relayers_rewards: RelayersRewards<AccountId, Currency::Balance>,
		relayer_fund_account: &AccountId,
	) {
		pay_relayers_rewards::<Currency, _>(
			confirmation_relayer,
			relayers_rewards,
			relayer_fund_account,
			GetConfirmationFee::get(),
		);
	}
}

/// Pay rewards to given relayers, optionally rewarding confirmation relayer.
fn pay_relayers_rewards<Currency, AccountId>(
	confirmation_relayer: &AccountId,
	relayers_rewards: RelayersRewards<AccountId, Currency::Balance>,
	relayer_fund_account: &AccountId,
	confirmation_fee: Currency::Balance,
) where
	AccountId: Debug + Default + Encode + PartialEq,
	Currency: CurrencyT<AccountId>,
	Currency::Balance: From<u64>,
{
	// reward every relayer except `confirmation_relayer`
	let mut confirmation_relayer_reward = Currency::Balance::zero();
	for (relayer, reward) in relayers_rewards {
		let mut relayer_reward = reward.reward;

		if relayer != *confirmation_relayer {
			// If delivery confirmation is submitted by other relayer, let's deduct confirmation fee
			// from relayer reward.
			//
			// If confirmation fee has been increased (or if it was the only component of message fee),
			// then messages relayer may receive zero reward.
			let mut confirmation_reward = confirmation_fee.saturating_mul(reward.messages.into());
			if confirmation_reward > relayer_reward {
				confirmation_reward = relayer_reward;
			}
			relayer_reward = relayer_reward.saturating_sub(confirmation_reward);
			confirmation_relayer_reward = confirmation_relayer_reward.saturating_add(confirmation_reward);
		} else {
			// If delivery confirmation is submitted by this relayer, let's add confirmation fee
			// from other relayers to this relayer reward.
			confirmation_relayer_reward = confirmation_relayer_reward.saturating_add(reward.reward);
			continue;
		}

		pay_relayer_reward::<Currency, _>(relayer_fund_account, &relayer, relayer_reward);
	}

	// finally - pay reward to confirmation relayer
	pay_relayer_reward::<Currency, _>(relayer_fund_account, confirmation_relayer, confirmation_relayer_reward);
}

/// Transfer funds from relayers fund account to given relayer.
fn pay_relayer_reward<Currency, AccountId>(
	relayer_fund_account: &AccountId,
	relayer_account: &AccountId,
	reward: Currency::Balance,
) where
	AccountId: Debug,
	Currency: CurrencyT<AccountId>,
{
	if reward.is_zero() {
		return;
	}

	let pay_result = Currency::transfer(
		relayer_fund_account,
		relayer_account,
		reward,
		ExistenceRequirement::AllowDeath,
	);

	// we can't actually do anything here, because rewards are paid as a part of unrelated transaction
	match pay_result {
		Ok(_) => frame_support::debug::trace!(
			target: "runtime",
			"Rewarded relayer {:?} with {:?}",
			relayer_account,
			reward,
		),
		Err(error) => frame_support::debug::trace!(
			target: "runtime",
			"Failed to pay relayer {:?} reward {:?}: {:?}",
			relayer_account,
			reward,
			error,
		),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{run_test, AccountId as TestAccountId, Balance as TestBalance, TestRuntime};
	use bp_message_lane::source_chain::RelayerRewards;

	type Balances = pallet_balances::Module<TestRuntime>;

	const RELAYER_1: TestAccountId = 1;
	const RELAYER_2: TestAccountId = 2;
	const RELAYER_3: TestAccountId = 3;
	const RELAYERS_FUND_ACCOUNT: TestAccountId = crate::mock::ENDOWED_ACCOUNT;

	fn relayers_rewards() -> RelayersRewards<TestAccountId, TestBalance> {
		vec![
			(
				RELAYER_1,
				RelayerRewards {
					reward: 100,
					messages: 2,
				},
			),
			(
				RELAYER_2,
				RelayerRewards {
					reward: 100,
					messages: 3,
				},
			),
		]
		.into_iter()
		.collect()
	}

	#[test]
	fn confirmation_relayer_is_rewarded_if_it_has_also_delivered_messages() {
		run_test(|| {
			pay_relayers_rewards::<Balances, _>(&RELAYER_2, relayers_rewards(), &RELAYERS_FUND_ACCOUNT, 10);

			assert_eq!(Balances::free_balance(&RELAYER_1), 80);
			assert_eq!(Balances::free_balance(&RELAYER_2), 120);
		});
	}

	#[test]
	fn confirmation_relayer_is_rewarded_if_it_has_not_delivered_any_delivered_messages() {
		run_test(|| {
			pay_relayers_rewards::<Balances, _>(&RELAYER_3, relayers_rewards(), &RELAYERS_FUND_ACCOUNT, 10);

			assert_eq!(Balances::free_balance(&RELAYER_1), 80);
			assert_eq!(Balances::free_balance(&RELAYER_2), 70);
			assert_eq!(Balances::free_balance(&RELAYER_3), 50);
		});
	}

	#[test]
	fn only_confirmation_relayer_is_rewarded_if_confirmation_fee_has_significantly_increased() {
		run_test(|| {
			pay_relayers_rewards::<Balances, _>(&RELAYER_3, relayers_rewards(), &RELAYERS_FUND_ACCOUNT, 1000);

			assert_eq!(Balances::free_balance(&RELAYER_1), 0);
			assert_eq!(Balances::free_balance(&RELAYER_2), 0);
			assert_eq!(Balances::free_balance(&RELAYER_3), 200);
		});
	}
}
