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

pub use bp_messages::RewardAtSource;

use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{Get, Zero},
	BoundedVec, RuntimeDebug,
};

/// A relayer and the reward that it wants to receive for delivering a single message.
#[derive(Clone, Decode, Encode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct RelayerAndReward<AccountId> {
	/// A relayer account identifier.
	relayer: AccountId,
	/// A reward that is paid to relayer for delivering a single message.
	reward: RewardAtSource,
}

impl<AccountId> RelayerAndReward<AccountId> {
	/// Return relayer account identifier.
	pub fn relayer(&self) -> &AccountId {
		&self.relayer
	}

	/// Return expected relayer reward.
	pub fn reward(&self) -> RewardAtSource {
		self.reward
	}
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
#[scale_info(skip_type_params(MaxRelayersPerLane))]
pub struct LaneRelayersSet<AccountId, BlockNumber, MaxRelayersPerLane: Get<u32>> {
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
	active_set: BoundedVec<RelayerAndReward<AccountId>, MaxRelayersPerLane>,
	/// Next set of lane relayers.
	///
	/// It is a bounded priority queue. Relayers that are working for larger reward are replaced
	/// with relayers, that are working for smaller reward.
	next_set: BoundedVec<RelayerAndReward<AccountId>, MaxRelayersPerLane>,
}

impl<AccountId, BlockNumber, MaxRelayersPerLane>
	LaneRelayersSet<AccountId, BlockNumber, MaxRelayersPerLane>
where
	AccountId: Clone + PartialOrd,
	BlockNumber: Copy + Zero,
	MaxRelayersPerLane: Get<u32>,
{
	/// Creates new empty relayers set, where next sets enacts at given block.
	pub fn empty(next_set_may_enact_at: BlockNumber) -> Self {
		LaneRelayersSet {
			enacted_at: Zero::zero(),
			next_set_may_enact_at,
			active_set: BoundedVec::new(),
			next_set: BoundedVec::new(),
		}
	}

	/// Returns block, starting from which the [`Self::next_set`] may be enacted.
	pub fn next_set_may_enact_at(&self) -> BlockNumber {
		self.next_set_may_enact_at
	}

	/// Returns relayers in the active set.
	pub fn active_relayers(&self) -> &[RelayerAndReward<AccountId>] {
		self.active_set.as_slice()
	}

	/// Returns relayers in the next set.
	pub fn next_relayers(&self) -> &[RelayerAndReward<AccountId>] {
		self.next_set.as_slice()
	}

	/// Try insert relayer to the next set.
	///
	/// Returns `true` if relayer has been added to the set and false otherwise.
	pub fn next_set_try_push(&mut self, relayer: AccountId, reward: RewardAtSource) -> bool {
		// first, remove existing entry for the same relayer from the set
		self.next_set_try_remove(&relayer);
		// now try to insert new entry into the queue
		self.next_set
			.force_insert_keep_left(
				self.select_position_in_next_set(reward),
				RelayerAndReward { relayer, reward },
			)
			.is_ok()
	}

	/// Try remove relayer from the next set.
	///
	/// Returns `true` if relayer has been removed from the set.
	pub fn next_set_try_remove(&mut self, relayer: &AccountId) -> bool {
		let len_before = self.next_set.len();
		self.next_set.retain(|entry| entry.relayer != *relayer);
		self.next_set.len() != len_before
	}

	/// Activate next set of relayers.
	///
	/// The [`Self::active_set`] is replaced with the [`Self::next_set`].
	pub fn activate_next_set(&mut self, new_next_set_may_enact_at: BlockNumber) {
		sp_std::mem::swap(&mut self.active_set, &mut self.next_set);
		// we clear next set here. Relayers from the active set will be readded here if
		// they deliver at least one message in epoch and their reward will be concurrent.
		// Or else, they'll need to reregister manually.
		self.next_set.clear();
		self.next_set_may_enact_at = new_next_set_may_enact_at;
	}

	fn select_position_in_next_set(&self, reward: RewardAtSource) -> usize {
		// we need to insert new entry **after** the last entry with the same `reward`. Otherwise it
		// may be used to push relayers our of the queue
		let mut initial_position = self
			.next_set
			.binary_search_by_key(&reward, |entry| entry.reward)
			.unwrap_or_else(|position| position);
		while self
			.next_set
			.get(initial_position)
			.map(|entry| entry.reward == reward)
			.unwrap_or(false)
		{
			initial_position += 1;
		}
		initial_position
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use sp_runtime::traits::ConstU32;

	const MAX_LANE_RELAYERS: u32 = 4;
	type TestLaneRelayersSet = LaneRelayersSet<u32, u32, u32, ConstU32<MAX_LANE_RELAYERS>>;

	#[test]
	fn next_set_try_push_works() {
		let mut relayers: TestLaneRelayersSet = LaneRelayersSet {
			enacted_at: 0,
			next_set_may_enact_at: 100,
			active_set: vec![].try_into().unwrap(),
			next_set: vec![].try_into().unwrap(),
		};

		// first `MAX_LANE_RELAYERS` are simply filling the set
		for i in 0..MAX_LANE_RELAYERS {
			assert!(relayers.next_set_try_push(i, (MAX_LANE_RELAYERS - i) * 10));
		}
		assert_eq!(
			relayers.next_set.as_slice(),
			&[
				RelayerAndReward { relayer: 3, reward: 10 },
				RelayerAndReward { relayer: 2, reward: 20 },
				RelayerAndReward { relayer: 1, reward: 30 },
				RelayerAndReward { relayer: 0, reward: 40 },
			],
		);

		// try to insert relayer who wants reward, that is larger than anyone in the set
		// => the set is not changed
		assert!(!relayers.next_set_try_push(4, 50));
		assert_eq!(
			relayers.next_set.as_slice(),
			&[
				RelayerAndReward { relayer: 3, reward: 10 },
				RelayerAndReward { relayer: 2, reward: 20 },
				RelayerAndReward { relayer: 1, reward: 30 },
				RelayerAndReward { relayer: 0, reward: 40 },
			],
		);

		// replace worst relayer in the set
		assert!(relayers.next_set_try_push(5, 35));
		assert_eq!(
			relayers.next_set.as_slice(),
			&[
				RelayerAndReward { relayer: 3, reward: 10 },
				RelayerAndReward { relayer: 2, reward: 20 },
				RelayerAndReward { relayer: 1, reward: 30 },
				RelayerAndReward { relayer: 5, reward: 35 },
			],
		);

		// insert best relayer to the set, pushing worst relayer out of set
		assert!(relayers.next_set_try_push(6, 5));
		assert_eq!(
			relayers.next_set.as_slice(),
			&[
				RelayerAndReward { relayer: 6, reward: 5 },
				RelayerAndReward { relayer: 3, reward: 10 },
				RelayerAndReward { relayer: 2, reward: 20 },
				RelayerAndReward { relayer: 1, reward: 30 },
			],
		);

		// insert best relayer to the set, pushing worst relayer out of set
		assert!(relayers.next_set_try_push(6, 5));
		assert_eq!(
			relayers.next_set.as_slice(),
			&[
				RelayerAndReward { relayer: 6, reward: 5 },
				RelayerAndReward { relayer: 3, reward: 10 },
				RelayerAndReward { relayer: 2, reward: 20 },
				RelayerAndReward { relayer: 1, reward: 30 },
			],
		);

		// insert relayer to the middle of the set, pushing worst relayer out of set
		assert!(relayers.next_set_try_push(7, 15));
		assert_eq!(
			relayers.next_set.as_slice(),
			&[
				RelayerAndReward { relayer: 6, reward: 5 },
				RelayerAndReward { relayer: 3, reward: 10 },
				RelayerAndReward { relayer: 7, reward: 15 },
				RelayerAndReward { relayer: 2, reward: 20 },
			],
		);

		// insert couple of relayer that want the same reward as some relayer in the middle of the
		// queue => they are inserted **after** existing relayers
		assert!(relayers.next_set_try_push(8, 10));
		assert!(relayers.next_set_try_push(9, 10));
		assert_eq!(
			relayers.next_set.as_slice(),
			&[
				RelayerAndReward { relayer: 6, reward: 5 },
				RelayerAndReward { relayer: 3, reward: 10 },
				RelayerAndReward { relayer: 8, reward: 10 },
				RelayerAndReward { relayer: 9, reward: 10 },
			],
		);

		// insert next relayer, similar to previous => it isn't inserted
		assert!(!relayers.next_set_try_push(10, 10));
		assert_eq!(
			relayers.next_set.as_slice(),
			&[
				RelayerAndReward { relayer: 6, reward: 5 },
				RelayerAndReward { relayer: 3, reward: 10 },
				RelayerAndReward { relayer: 8, reward: 10 },
				RelayerAndReward { relayer: 9, reward: 10 },
			],
		);

		// update expected reward of existing relayer => the set order is changed
		assert!(relayers.next_set_try_push(8, 2));
		assert_eq!(
			relayers.next_set.as_slice(),
			&[
				RelayerAndReward { relayer: 8, reward: 2 },
				RelayerAndReward { relayer: 6, reward: 5 },
				RelayerAndReward { relayer: 3, reward: 10 },
				RelayerAndReward { relayer: 9, reward: 10 },
			],
		);
	}
}
