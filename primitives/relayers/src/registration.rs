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

//! Bridge relayers registration and slashing scheme.
//!
//! There is an option to add a refund-relayer signed extension that will compensate
//! relayer costs of the message delivery and confirmation transactions (as well as
//! required finality proofs). This extension boosts priority of message delivery
//! transactions, based on the number of bundled messages. So transaction with more
//! messages has larger priority than the transaction with less messages.
//! See `bridge_runtime_common::priority_calculator` for details;
//!
//! This encourages relayers to include more messages to their delivery transactions.
//! At the same time, we are not verifying storage proofs before boosting
//! priority. Instead, we simply trust relayer, when it says that transaction delivers
//! `N` messages.
//!
//! This allows relayers to submit transactions which declare large number of bundled
//! transactions to receive priority boost for free, potentially pushing actual delivery
//! transactions from the block (or even transaction queue). Such transactions are
//! not free, but their cost is relatively small.
//!
//! To alleviate that, we only boost transactions of relayers that have some stake
//! that guarantees that their transactions are valid. Such relayers get priority
//! for free, but they risk to lose their stake.

use crate::RewardsAccountParams;

use bp_messages::LaneId;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{CloneNoBound, PartialEqNoBound, RuntimeDebugNoBound};
use scale_info::TypeInfo;
use sp_arithmetic::traits::BaseArithmetic;
use sp_runtime::{
	traits::{Get, Zero},
	BoundedVec, DispatchError, DispatchResult, Saturating,
};
use sp_std::fmt::Debug;

/// Relayer registration.
#[derive(CloneNoBound, Decode, Encode, Eq, PartialEqNoBound, RuntimeDebugNoBound, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(BlockNumber, Balance, MaxLanesPerRelayer))]
pub struct Registration<BlockNumber: Clone + Debug + PartialEq, Balance: Clone + Debug + PartialEq, MaxLanesPerRelayer: Get<u32>> {
	/// The last block number, where this registration is considered active.
	///
	/// Relayer has an option to renew his registration (this may be done before it
	/// is spoiled as well). Starting from block `valid_till + 1`, relayer may `deregister`
	/// himself and get his stake back.
	///
	/// Please keep in mind that priority boost stops working some blocks before the
	/// registration ends (see [`StakeAndSlash::RequiredRegistrationLease`]).
	pub valid_till: BlockNumber,
	/// Active relayer stake, which is mapped to the relayer reserved balance.
	///
	/// If `stake` is less than the [`StakeAndSlash::RequiredStake`] plus additional
	/// [`StakeAndSlash::RequiredStake`] for every entry in the `lanes` vector, the
	/// registration is considered inactive even if `valid_till + 1` is not yet reached.
	pub stake: Balance,
	/// All lanes, where relayer has explicitly registered itself for additional
	/// priority boost.
	///
	/// Relayer pays additional [`StakeAndSlash::RequiredStake`] for every lane.
	///
	/// The entry in this vector does not guarantee that the relayer is actually in
	/// the active or the next set of relayers at given lane. It only says that the
	/// relayer has tried to register at the lane.
	pub lanes: BoundedVec<LaneId, MaxLanesPerRelayer>,
}

impl<BlockNumber: Clone + Copy + Debug + PartialEq + PartialOrd + Saturating, Balance: BaseArithmetic + Clone + Debug + PartialEq + Zero, MaxLanesPerRelayer: Get<u32>> Registration<BlockNumber, Balance, MaxLanesPerRelayer> {
	/// Creates new empty registration that ends at given block.
	pub fn new(valid_till: BlockNumber) -> Self {
		Registration { valid_till, stake: Zero::zero(), lanes: BoundedVec::new() }
	}

	/// Returns minimal stake that the relayer need to have in reserve to be
	/// considered active.
	pub fn required_stake(
		&self,
		base_stake: Balance,
		stake_per_lane: Balance,
	) -> Balance {
		stake_per_lane
			.saturating_mul(Balance::try_from(self.lanes.len()).unwrap_or(Balance::max_value()))
			.saturating_add(base_stake)
	}

	/// Returns `true` if registration is active. In other words, if registration
	///
	/// - has stake larger or equal to required;
	///
	/// - is valid for another `required_registration_lease` blocks.
	pub fn is_active(
		&self,
		base_stake: Balance,
		stake_per_lane: Balance,
		current_block_number: BlockNumber,
		required_registration_lease: BlockNumber,
	) -> bool {
		// registration is inactive if relayer stake is less than required
		if self.stake < self.required_stake(base_stake, stake_per_lane) {
			return false
		}

		// registration is inactive if it ends soon
		let remaining_lease = self
			.valid_till
			.saturating_sub(current_block_number);
		if remaining_lease <= required_registration_lease {
			return false
		}

		true
	}
}

/// Relayer stake-and-slash mechanism.
pub trait StakeAndSlash<AccountId, BlockNumber, Balance> {
	/// The stake that the relayer must have to have its transactions boosted.
	type RequiredStake: Get<Balance>;
	/// Required **remaining** registration lease to be able to get transaction priority boost.
	///
	/// If the difference between registration's `valid_till` and the current block number
	/// is less than the `RequiredRegistrationLease`, it becomes inactive and relayer transaction
	/// won't get priority boost. This period exists, because priority is calculated when
	/// transaction is placed to the queue (and it is reevaluated periodically) and then some time
	/// may pass before transaction will be included into the block.
	type RequiredRegistrationLease: Get<BlockNumber>;

	/// Reserve the given amount at relayer account.
	fn reserve(relayer: &AccountId, amount: Balance) -> DispatchResult;
	/// `Unreserve` the given amount from relayer account.
	///
	/// Returns amount that we have failed to `unreserve`.
	fn unreserve(relayer: &AccountId, amount: Balance) -> Balance;
	/// Slash up to `amount` from reserved balance of account `relayer` and send funds to given
	/// `beneficiary`.
	///
	/// Returns `Ok(_)` with non-zero balance if we have failed to repatriate some portion of stake.
	fn repatriate_reserved(
		relayer: &AccountId,
		beneficiary: RewardsAccountParams,
		amount: Balance,
	) -> Result<Balance, DispatchError>;
}

impl<AccountId, BlockNumber, Balance> StakeAndSlash<AccountId, BlockNumber, Balance> for ()
where
	Balance: Default + Zero,
	BlockNumber: Default,
{
	type RequiredStake = ();
	type RequiredRegistrationLease = ();

	fn reserve(_relayer: &AccountId, _amount: Balance) -> DispatchResult {
		Ok(())
	}

	fn unreserve(_relayer: &AccountId, _amount: Balance) -> Balance {
		Zero::zero()
	}

	fn repatriate_reserved(
		_relayer: &AccountId,
		_beneficiary: RewardsAccountParams,
		_amount: Balance,
	) -> Result<Balance, DispatchError> {
		Ok(Zero::zero())
	}
}
