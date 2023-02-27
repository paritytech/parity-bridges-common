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

//! Primitives of messages module.

#![warn(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]

use bp_messages::LaneId;
use bp_runtime::{ChainId, StorageDoubleMapKeyProvider};
use frame_support::{Blake2_128Concat, Identity};
use scale_info::TypeInfo;
use sp_runtime::{
	codec::{Codec, Decode, Encode, EncodeLike, MaxEncodedLen},
	traits::AccountIdConversion,
	TypeId,
};
use sp_std::{fmt::Debug, marker::PhantomData};

/// The owner of the sovereign account that should pay the rewards.
///
/// More details in the documentation for [`RewardsAccountParams`].
#[derive(Copy, Clone, Debug, Decode, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
pub enum RewardsAccountOwner {
	/// The rewards should be payed from the sovereign account of this parachain at the bridge hub.
	ThisChain,
	/// The rewards should be payed from the sovereign account of the bridged parachain at the
	/// bridge hub.
	BridgedChain,
}

/// Structure used to identify the account that pays a reward to the relayer.
///
/// A bridge connects 2 bridge hubs: one attached to this relay chain and one attached to the
/// bridged relay chain. A messages lane between the 2 bridge hubs connects 2 parachains, one
/// attached to this relay chain and one attached to the bridged relay chain. Each of these 2
/// parachains have a sovereign account at each bridge hub. Each of the sovereign accounts will pay
/// rewards for different operations.
#[derive(Copy, Clone, Debug, Decode, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
pub struct RewardsAccountParams {
	lane_id: LaneId,
	bridged_chain_id: ChainId,
	owner: RewardsAccountOwner,
}

impl RewardsAccountParams {
	/// Create a new instance of `RewardsAccountParams`.
	pub const fn new(
		lane_id: LaneId,
		bridged_chain_id: ChainId,
		owner: RewardsAccountOwner,
	) -> Self {
		Self { lane_id, bridged_chain_id, owner }
	}
}

impl TypeId for RewardsAccountParams {
	const TYPE_ID: [u8; 4] = *b"brap";
}

/// Reward payment procedure.
pub trait PaymentProcedure<Relayer, Reward> {
	/// Error that may be returned by the procedure.
	type Error: Debug;

	/// Pay reward to the relayer for serving given message lane.
	fn pay_reward(
		relayer: &Relayer,
		lane_id: RewardsAccountParams,
		reward: Reward,
	) -> Result<(), Self::Error>;
}

impl<Relayer, Reward> PaymentProcedure<Relayer, Reward> for () {
	type Error = &'static str;

	fn pay_reward(_: &Relayer, _: RewardsAccountParams, _: Reward) -> Result<(), Self::Error> {
		Ok(())
	}
}

/// Reward payment procedure that does `balances::transfer` call from the account, derived from
/// given lane.
pub struct PayLaneRewardFromAccount<T, Relayer>(PhantomData<(T, Relayer)>);

impl<T, Relayer> PayLaneRewardFromAccount<T, Relayer>
where
	Relayer: Decode + Encode,
{
	/// Return account that pay rewards for serving given lane.
	pub fn lane_rewards_account(lane_id: RewardsAccountParams) -> Relayer {
		lane_id.into_sub_account_truncating(b"bridge-lane")
	}
}

impl<T, Relayer> PaymentProcedure<Relayer, T::Balance> for PayLaneRewardFromAccount<T, Relayer>
where
	T: frame_support::traits::fungible::Transfer<Relayer>,
	Relayer: Decode + Encode,
{
	type Error = sp_runtime::DispatchError;

	fn pay_reward(
		relayer: &Relayer,
		lane_id: RewardsAccountParams,
		reward: T::Balance,
	) -> Result<(), Self::Error> {
		T::transfer(&Self::lane_rewards_account(lane_id), relayer, reward, false).map(drop)
	}
}

/// Can be use to access the runtime storage key within the `RelayerRewards` map of the relayers
/// pallet.
pub struct RelayerRewardsKeyProvider<AccountId, Reward>(PhantomData<(AccountId, Reward)>);

impl<AccountId, Reward> StorageDoubleMapKeyProvider for RelayerRewardsKeyProvider<AccountId, Reward>
where
	AccountId: Codec + EncodeLike,
	Reward: Codec + EncodeLike,
{
	const MAP_NAME: &'static str = "RelayerRewards";

	type Hasher1 = Blake2_128Concat;
	type Key1 = AccountId;
	type Hasher2 = Identity;
	type Key2 = RewardsAccountParams;
	type Value = Reward;
}

#[cfg(test)]
mod tests {
	use super::*;
	use bp_messages::LaneId;
	use sp_runtime::testing::H256;

	#[test]
	fn different_lanes_are_using_different_accounts() {
		assert_eq!(
			PayLaneRewardFromAccount::<(), H256>::lane_rewards_account(RewardsAccountParams::new(
				LaneId([0, 0, 0, 0]),
				*b"test",
				RewardsAccountOwner::ThisChain
			)),
			hex_literal::hex!("627261700000000074657374006272696467652d6c616e650000000000000000")
				.into(),
		);

		assert_eq!(
			PayLaneRewardFromAccount::<(), H256>::lane_rewards_account(RewardsAccountParams::new(
				LaneId([0, 0, 0, 1]),
				*b"test",
				RewardsAccountOwner::ThisChain
			)),
			hex_literal::hex!("627261700000000174657374006272696467652d6c616e650000000000000000")
				.into(),
		);
	}

	#[test]
	fn different_directions_are_using_different_accounts() {
		assert_eq!(
			PayLaneRewardFromAccount::<(), H256>::lane_rewards_account(RewardsAccountParams::new(
				LaneId([0, 0, 0, 0]),
				*b"test",
				RewardsAccountOwner::ThisChain
			)),
			hex_literal::hex!("627261700000000074657374006272696467652d6c616e650000000000000000")
				.into(),
		);

		assert_eq!(
			PayLaneRewardFromAccount::<(), H256>::lane_rewards_account(RewardsAccountParams::new(
				LaneId([0, 0, 0, 0]),
				*b"test",
				RewardsAccountOwner::BridgedChain
			)),
			hex_literal::hex!("627261700000000074657374016272696467652d6c616e650000000000000000")
				.into(),
		);
	}
}
