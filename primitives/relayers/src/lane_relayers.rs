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

//! Bridge lane relayers.

use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{traits::Get, BoundedVec, RuntimeDebug};

/// A relayer and the reward that it wants to receive for delivering a single message.
#[derive(Clone, Decode, Encode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(AccountId, Reward))]
pub struct RelayerAndReward<AccountId, Reward> {
	/// A relayer account identifier.
	relayer: AccountId,
	/// A reward that is paid to relayer for delivering a single message.
	reward: Reward,
}

/// A set of relayers that have explicitly registered themselves at a given lane.
///
/// Every relayer inside this set receives additional priority boost when it submits
/// message delivers messages at given lane. The boost only happens inside the slot,
/// assigned to relayer.
///
/// The set is required to change periodically (at `next_set_may_enact_at`). An interval, when
/// the same relayers set is active is called epoch. Every relayer in the epoch is guaranteed
/// to have at least one slot, but epochs may have differrent lengths.
///
/// We change the set to guarantee that inactive relayers are removed from the set eventually
/// and are replaced by active relayers. The relayer will be scheduled for autoremoval if it
/// has not delivered any messages during previous epoch.
///
/// Relayers are bargaining for the place in the set by offering lower reward for delivering
/// messages. Relayer, which agress to get a lower reward will likely to replace a "more greedy"
/// relayer in the [`Self::next_set`].
#[derive(Clone, Decode, Encode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(AccountId, BlockNumber, Reward))]
pub struct LaneRelayers<AccountId, BlockNumber, Reward, MaxLaneRelayers: Get<u32>> {
	/// Number of block, where the active set has been enacted.
	enacted_at: BlockNumber,
	/// Number of block, where the active set may be replaced with the [`Self::next_set`].
	///
	/// We do not allow immediate changes of the [`Self::next_set`], because relayers
	/// may change it so that they are always assigned the current slot.
	next_set_may_enact_at: BlockNumber,
	/// An active set of lane relayers.
	///
	/// It is a circular queue. Every relayer in the queue is assigned the slot (fixed number
	/// of blocks), starting from [`Self::enacted_at`]. Once the slot of last relayer ends,
	/// next slot will be assigned to the first relayer and so on.
	active_set: BoundedVec<RelayerAndReward<AccountId, Reward>, MaxLaneRelayers>,
	/// Next set of lane relayers.
	///
	/// It is a bounded priority queue. Relayers that are working for larger reward are replaced
	/// with relayers, that are working for smaller reward.
	next_set: BoundedVec<RelayerAndReward<AccountId, Reward>, MaxLaneRelayers>,
}

impl<AccountId, BlockNumber, Reward, MaxLaneRelayers> LaneRelayers<AccountId, BlockNumber, Reward, MaxLaneRelayers> where
	AccountId: PartialOrd,	
	Reward: Clone + Copy + Ord,
	MaxLaneRelayers: Get<u32>,
{
	/// Try insert relayer into next set.
	///
	/// Returns `true` if relayer has been added to the set and false otherwise.
	pub fn next_set_try_push(&mut self, relayer: AccountId, reward: Reward) -> bool {
		// first, remove existing entry for the same relayer from the set
		self.next_set.retain(|entry| entry.relayer != relayer);
		// now try to insert new entry into the queue
		self.next_set.force_insert_keep_left(
			self.select_position_in_next_set(reward),
			RelayerAndReward { relayer, reward },
		).is_ok()
	}

	fn select_position_in_next_set(&self, reward: Reward) -> usize {
		// we need to insert new entry **after** the last entry with the same `reward`. Otherwise it may be used
		// to push relayers our of the queue
		let mut initial_position = self
			.next_set
			.binary_search_by_key(&reward, |entry| entry.reward)
			.unwrap_or_else(|position| position);
		while self.next_set.get(initial_position + 1).map(|entry| entry.reward == reward).unwrap_or(false) {
			initial_position += 1;
		}
		initial_position
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	const MAX_LANE_RELAYERS: u32 = 4;
	type TestLaneRelayers = LaneRelayers<u64, u64, u64, ConstU32<MAX_LANE_RELAYERS>>;

	#[test]
	fn next_set_try_push_works() {
		let mut relayers: TestLaneRelayers = LaneRelayers {
			enacted_at: 0,
			next_set_may_enact_at: 100,
			active_set: vec![].try_into().unwrap(),
			next_set: vec![].try_into().unwrap(),
		};

		// first `MAX_LANE_RELAYERS` are added
	}
}
