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

//! Bridge relayers slashing scheme.
//!
//! We are boosting priority of message delivery transactions for free (see
//! [`crate::priority_calculator`]). This encourages relayers to include more messages to their
//! delivery transactions. At the same time, we are not verifying storage proofs before boosting
//! priority. Instead, we simply trust relayer, when it says that transaction delivers `N` messages.
//!
//! This opens a hole for malicious relayers, which may spam our tx pool with high-priority
//! transactions with a low cost. To alleviate that, we require relayers to make some stake
//! that will guarantee that they are not cheating. If delivery transaction fails, they will
//! get slashed.

use bp_relayers::{PayRewardFromAccount, RewardsAccountParams};
use codec::Codec;
use frame_support::traits::{tokens::BalanceStatus, ReservableCurrency};
use pallet_bridge_relayers::Config as RelayersConfig;
use sp_runtime::{
	traits::{Get, Zero},
	DispatchResult,
};
use sp_std::{fmt::Debug, marker::PhantomData};

/// Relayer stake-and-slash mechanism, used to guarantee that the message delivery
/// transaction is correct.
pub trait DeliveryStakeAndSlash<AccountId> {
	/// Returns true if relayer has enough balance for reserving the stake amount.
	fn can_reserve(relayer: &AccountId) -> bool;
	/// Reserve the stake that guarantees that the message delivery transaction is valid.
	fn reserve(relayer: &AccountId) -> DispatchResult;
	/// Unreserve the relayer stake made by the `reserve` call.
	fn unreserve(relayer: &AccountId);
	/// Slash the previously reserved relayer balance and and send funds to given beneficiary.
	fn repatriate_reserved(relayer: &AccountId, beneficiary: RewardsAccountParams);
}

/// Stake-and-slash implementation for runtimes that are using `pallet_balances` to withdraw
/// transaction fee and pay rewards to relayers.
pub struct DeliveryStakeAndSlashFromBalance<AccountId, Currency, Relayers, Stake>(
	PhantomData<(AccountId, Currency, Relayers, Stake)>,
);

impl<AccountId, Currency, Relayers, Stake> DeliveryStakeAndSlash<AccountId>
	for DeliveryStakeAndSlashFromBalance<AccountId, Currency, Relayers, Stake>
where
	AccountId: Codec + Debug,
	Currency: ReservableCurrency<AccountId>,
	Relayers: RelayersConfig<PaymentProcedure = PayRewardFromAccount<Currency, AccountId>>,
	Stake: Get<Currency::Balance>,
{
	fn can_reserve(relayer: &AccountId) -> bool {
		Currency::can_reserve(relayer, Stake::get())
	}

	fn reserve(relayer: &AccountId) -> DispatchResult {
		Currency::reserve(relayer, Stake::get())
	}

	fn unreserve(relayer: &AccountId) {
		let stake = Stake::get();
		let failed_to_unreserve = Currency::unreserve(relayer, stake);
		// in practice this shouldn't happen, because caller (our signed extension from
		// `pre_dispatch`) always call the `reserve` and we reserve enough stake there
		if !failed_to_unreserve.is_zero() {
			log::debug!(
				target: "runtime::bridge",
				"Failed to unreserve relayer {:?}: tried to unreserve {:?}, failed to unreserve {:?}",
				relayer,
				stake,
				failed_to_unreserve,
			);
		}
	}

	fn repatriate_reserved(relayer: &AccountId, beneficiary: RewardsAccountParams) {
		let slash = Stake::get();
		let benificiary_account = Relayers::PaymentProcedure::rewards_account(beneficiary);
		let result = Currency::repatriate_reserved(
			relayer,
			&benificiary_account,
			slash,
			BalanceStatus::Free,
		);
		match result {
			Ok(b) if b.is_zero() => {
				log::trace!(
					target: "runtime::bridge",
					"Relayer account {:?} has been slashed for incorrect message delivery. \
					Benificiary: {:?}, amount: {:?}",
					relayer,
					benificiary_account,
					slash,
				);
			},
			Ok(failed_to_slash) => {
				// in practice this shouldn't happen, because caller (our signed extension from
				// `pre_dispatch`) always call the `reserve` and we reserve enough stake there
				log::trace!(
					target: "runtime::bridge",
					"Relayer account {:?} has been partially slashed for incorrect message delivery. \
					Benificiary: {:?}, slash amount: {:?}, failed to slash: {:?}",
					relayer,
					benificiary_account,
					slash,
					failed_to_slash,
				);
			},
			Err(e) => {
				// TODO: document this. Where?

				// it may fail if there's no beneficiary account. For us it means that this account
				// must exists before we'll deploy the bridge
				log::debug!(
					target: "runtime::bridge",
					"Failed to slash relayer {:?}: {:?}. Maybe benificiary account doesn't exist? \
					Benificiary: {:?}, amount: {:?}, failed to slash: {:?}",
					relayer,
					e,
					benificiary_account,
					slash,
					slash,
				);
			},
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::*;

	use bp_relayers::RewardsAccountOwner;
	use frame_support::traits::fungible::Mutate;

	fn test_stake() -> ThisChainBalance {
		TestStake::get()
	}

	#[test]
	fn can_reserve_works() {
		run_test(|| {
			assert!(!TestDeliveryStakeAndSlash::can_reserve(&1));

			Balances::mint_into(&2, test_stake() - 1).unwrap();
			assert!(!TestDeliveryStakeAndSlash::can_reserve(&2));

			Balances::mint_into(&3, test_stake() * 2).unwrap();
			assert!(TestDeliveryStakeAndSlash::can_reserve(&3));
		})
	}

	#[test]
	fn reserve_works() {
		run_test(|| {
			assert!(TestDeliveryStakeAndSlash::reserve(&1).is_err());
			assert_eq!(Balances::free_balance(&1), 0);
			assert_eq!(Balances::reserved_balance(&1), 0);

			Balances::mint_into(&2, test_stake() - 1).unwrap();
			assert!(TestDeliveryStakeAndSlash::reserve(&2).is_err());
			assert_eq!(Balances::free_balance(&2), test_stake() - 1);
			assert_eq!(Balances::reserved_balance(&2), 0);

			Balances::mint_into(&3, test_stake() * 2).unwrap();
			assert!(TestDeliveryStakeAndSlash::reserve(&3).is_ok());
			assert_eq!(Balances::free_balance(&3), test_stake());
			assert_eq!(Balances::reserved_balance(&3), test_stake());
		})
	}

	#[test]
	fn unreserve_works() {
		run_test(|| {
			TestDeliveryStakeAndSlash::unreserve(&1);
			assert_eq!(Balances::free_balance(&1), 0);
			assert_eq!(Balances::reserved_balance(&1), 0);

			Balances::mint_into(&2, test_stake() * 2).unwrap();
			Balances::reserve(&2, test_stake() / 3).unwrap();
			TestDeliveryStakeAndSlash::unreserve(&2);
			assert_eq!(Balances::free_balance(&2), test_stake() * 2);
			assert_eq!(Balances::reserved_balance(&2), 0);

			Balances::mint_into(&3, test_stake() * 2).unwrap();
			Balances::reserve(&3, test_stake()).unwrap();
			TestDeliveryStakeAndSlash::unreserve(&3);
			assert_eq!(Balances::free_balance(&3), test_stake() * 2);
			assert_eq!(Balances::reserved_balance(&3), 0);
		})
	}

	#[test]
	fn repatriate_reserved_works() {
		run_test(|| {
			let benificiary = RewardsAccountParams::new(
				TEST_LANE_ID,
				TEST_BRIDGED_CHAIN_ID,
				RewardsAccountOwner::ThisChain,
			);
			let benificiary_account = TestPaymentProcedure::rewards_account(benificiary.clone());

			let mut expected_balance = ExistentialDeposit::get();
			Balances::mint_into(&benificiary_account, expected_balance).unwrap();

			TestDeliveryStakeAndSlash::repatriate_reserved(&1, benificiary.clone());
			assert_eq!(Balances::free_balance(&1), 0);
			assert_eq!(Balances::reserved_balance(&1), 0);
			assert_eq!(Balances::free_balance(&benificiary_account), expected_balance);
			assert_eq!(Balances::reserved_balance(&benificiary_account), 0);

			expected_balance += test_stake() / 3;
			Balances::mint_into(&2, test_stake() * 2).unwrap();
			Balances::reserve(&2, test_stake() / 3).unwrap();
			TestDeliveryStakeAndSlash::repatriate_reserved(&2, benificiary.clone());
			assert_eq!(Balances::free_balance(&2), test_stake() * 2 - test_stake() / 3);
			assert_eq!(Balances::reserved_balance(&2), 0);
			assert_eq!(Balances::free_balance(&benificiary_account), expected_balance);
			assert_eq!(Balances::reserved_balance(&benificiary_account), 0);

			expected_balance += test_stake();
			Balances::mint_into(&3, test_stake() * 2).unwrap();
			Balances::reserve(&3, test_stake()).unwrap();
			TestDeliveryStakeAndSlash::repatriate_reserved(&3, benificiary.clone());
			assert_eq!(Balances::free_balance(&3), test_stake());
			assert_eq!(Balances::reserved_balance(&3), 0);
			assert_eq!(Balances::free_balance(&benificiary_account), expected_balance);
			assert_eq!(Balances::reserved_balance(&benificiary_account), 0);
		})
	}

	#[test]
	fn repatriate_reserved_doesnt_work_when_benificiary_account_is_missing() {
		run_test(|| {
			let benificiary = RewardsAccountParams::new(
				TEST_LANE_ID,
				TEST_BRIDGED_CHAIN_ID,
				RewardsAccountOwner::ThisChain,
			);
			let benificiary_account = TestPaymentProcedure::rewards_account(benificiary.clone());

			Balances::mint_into(&3, test_stake() * 2).unwrap();
			Balances::reserve(&3, test_stake()).unwrap();
			TestDeliveryStakeAndSlash::repatriate_reserved(&3, benificiary.clone());
			assert_eq!(Balances::free_balance(&3), test_stake());
			assert_eq!(Balances::reserved_balance(&3), test_stake());
			assert_eq!(Balances::free_balance(&benificiary_account), 0);
			assert_eq!(Balances::reserved_balance(&benificiary_account), 0);
		});
	}
}
