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

//! Everything about outgoing messages sending.

use bp_message_lane::{LaneId, MessageNonce, OutboundLaneData};

/// Outbound lane storage.
pub trait OutboundLaneStorage {
	/// Message payload.
	type Payload;

	/// Lane id.
	fn id(&self) -> LaneId;
	/// Get lane data from the storage.
	fn data(&self) -> OutboundLaneData;
	/// Update lane data in the storage.
	fn set_data(&mut self, data: OutboundLaneData);
	/// Save outbound message in the storage.
	fn save_message(&mut self, nonce: MessageNonce, payload: Self::Payload);
	/// Remove outbound message from the storage.
	fn remove_message(&mut self, nonce: &MessageNonce);
}

/// Outbound messages lane.
pub struct OutboundLane<Storage> {
	storage: Storage,
}

impl<Storage: OutboundLaneStorage> OutboundLane<Storage> {
	/// Create new inbound lane backed by given storage.
	pub fn new(storage: Storage) -> Self {
		OutboundLane { storage }
	}

	/// Send message over lane.
	pub fn send_message(&mut self, payload: Storage::Payload) {
		let mut data = self.storage.data();
		// TODO: do we need to protect against nonce overflow?
		let nonce = data.latest_generated_nonce + 1;
		data.latest_generated_nonce = nonce;

		self.storage.save_message(nonce, payload);
		self.storage.set_data(data);
	}

	/// Confirm message receival.
	pub fn confirm_receival(&mut self, latest_received_nonce: MessageNonce) -> bool {
		let mut data = self.storage.data();
		if latest_received_nonce < data.latest_received_nonce || latest_received_nonce > data.latest_generated_nonce {
			return false;
		}

		data.latest_received_nonce = latest_received_nonce;
		self.storage.set_data(data);

		true
	}

	/// Prune at most `max_messages_to_prune` already received messages.
	pub fn _prune_messages(&mut self, max_messages_to_prune: MessageNonce) -> MessageNonce {
		// TODO: do we need to call it during `on_finalize` - then we need to iterate lanes map,
		// prbably without any effect (=> slowdown if there are many lanes)
		// track unpruned messages in other map?
		// prune on confirmation?

		let mut pruned_messages = 0;
		let mut anything_changed = false;
		let mut data = self.storage.data();
		while pruned_messages < max_messages_to_prune && data.oldest_unpruned_nonce <= data.latest_received_nonce {
			self.storage.remove_message(&data.oldest_unpruned_nonce);

			anything_changed = true;
			pruned_messages += 1;
			data.oldest_unpruned_nonce += 1;
		}

		if anything_changed {
			self.storage.set_data(data);
		}

		pruned_messages
	}
}
