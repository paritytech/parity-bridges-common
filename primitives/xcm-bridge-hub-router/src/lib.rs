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

//! Primitives of the `pallet-xcm-bridge-hub-router` pallet.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{EqNoBound, PartialEqNoBound};
use scale_info::TypeInfo;
use sp_runtime::{traits::Get, BoundedVec, FixedU128, RuntimeDebug};
use sp_std::ops::RangeInclusive;

pub use bp_xcm_bridge_hub::{
	bridge_id_from_locations, bridge_locations, BridgeId, LocalXcmChannelManager,
};

/// All required bridge details, known to the chain that uses XCM bridge hub for
/// sending messages.
#[derive(Clone, Decode, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub struct Bridge<BlockNumber> {
	/// The bridge-specific number to multiply the base delivery fee by.
	///
	/// This is a second component of the total fee factor. The first component is
	/// shared by all bridges and depends on the physical HRMP congestion.
	pub bridge_fee_factor: FixedU128,
	/// A latest block, at which the bridge has been resumed. If bridge is currently
	/// suspended, it is `None`.
	pub bridge_resumed_at: Option<BlockNumber>,
	/// Indices (inclusive range) of all currently suspended bridge messages.
	pub suspended_messages: Option<(u64, u64)>,
}

impl<BlockNumber> Bridge<BlockNumber> {
	/// Returns true if bridge is currently suspended.
	pub fn is_suspended(&self) -> bool {
		self.bridge_resumed_at.is_none()
	}

	/// Selects and returns index for next suspended message.
	pub fn select_next_suspended_message_index(&mut self) -> u64 {
		match self.suspended_messages {
			Some((start, end)) => {
				self.suspended_messages = Some((start, end + 1));
				end + 1
			},
			None => {
				self.suspended_messages = Some((1, 1));
				1
			},
		}
	}

	/// Returns range of all currently suspended messages.
	pub fn suspended_messages(&self) -> RangeInclusive<u64> {
		self.suspended_messages.map(|(start, end)| start..=end).unwrap_or(1..=0)
	}
}

/// Relieving bridges service queue.
///
/// Relieving bridge is the bridge that has been suspended some time ago, but now it is
/// resumed. Some messages have been queued (and stuck in the router pallet - in other words
/// "suspended") while it has been suspended. So the router tries to deliver all such
/// messages for relieving bridges. Once there's no more suspended messages, the bridge is
/// back to normal mode - we don't call it relieved anymore.
#[derive(
	Clone, Decode, Encode, EqNoBound, PartialEqNoBound, TypeInfo, MaxEncodedLen, RuntimeDebug,
)]
#[scale_info(skip_type_params(MaxBridges))]
pub struct RelievingBridgesQueue<MaxBridges: Get<u32>> {
	/// An index within the `self.bridges` of next relieving bridge that needs some service.
	pub current: u32,
	/// All relieving bridges.
	pub bridges: BoundedVec<BridgeId, MaxBridges>,
}

impl<MaxBridges: Get<u32>> RelievingBridgesQueue<MaxBridges> {
	/// Creates a queue with single item.
	pub fn with(bridge_id: BridgeId) -> Self {
		RelievingBridgesQueue {
			current: 0,
			bridges: {
				let mut bridges = BoundedVec::new();
				bridges.force_push(bridge_id);
				bridges
			},
		}
	}

	/// Push another bridges onto the queue.
	pub fn try_push(&mut self, bridge_id: BridgeId) -> Result<(), BridgeId> {
		self.bridges.try_push(bridge_id)
	}

	/// Returns true if the queue is empty.
	pub fn is_empty(&self) -> bool {
		self.bridges.is_empty()
	}

	/// Reset current position.
	pub fn reset_current(&mut self) {
		self.current = 0;
	}

	/// Returns current bridge identifier.
	pub fn current(&self) -> Option<BridgeId> {
		self.bridges.get(self.current as usize).cloned()
	}

	/// Remove current bridge from the queue.
	pub fn remove_current(&mut self) {
		self.bridges.remove(self.current as usize);
		if self.current as usize >= self.bridges.len() {
			self.current = 0;
		}
	}

	/// Advance current bridge position.
	pub fn advance(&mut self) {
		self.current += 1;
		if self.current as usize >= self.bridges.len() {
			self.current = 0;
		}
	}

	/// Remove relieving bridge from the set.
	pub fn remove(&mut self, bridge: BridgeId) {
		self.bridges.retain(|b| *b != bridge);
	}
}
