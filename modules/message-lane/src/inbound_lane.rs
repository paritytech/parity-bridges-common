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

//! Everything about incoming messages receival.

use bp_message_lane::{LaneId, Message, MessageAction, MessageKey, MessageNonce, InboundLaneData, OnMessageReceived};

/// Inbound lane storage.
pub trait InboundLaneStorage {
	/// Message payload.
	type Payload;

	/// Lane id.
	fn id(&self) -> LaneId;
	/// Get lane data from the storage.
	fn data(&self) -> InboundLaneData;
	/// Update lane data in the storage.
	fn set_data(&mut self, data: InboundLaneData);
	/// Returns saved inbound message payload.
	fn message(&self, nonce: &MessageNonce) -> Option<Self::Payload>;
	/// Save inbound message in the storage.
	fn save_message(&mut self, nonce: MessageNonce, payload: Self::Payload);
	/// Remove inbound message from the storage.
	fn remove_message(&mut self, nonce: &MessageNonce);
}

/// Inbound messages lane.
pub struct InboundLane<Storage> {
	storage: Storage,
}

impl<Storage: InboundLaneStorage> InboundLane<Storage> {
	/// Create new inbound lane backed by given storage.
	pub fn new(storage: Storage) -> Self {
		InboundLane { storage }
	}

	/// Receive new message.
	pub fn receive_message(
		&mut self,
		nonce: MessageNonce,
		payload: Storage::Payload,
		processor: &mut impl OnMessageReceived<Storage::Payload>,
	) -> bool {
		let mut data = self.storage.data();
		let is_correct_message = nonce == data.latest_received_nonce + 1;
		if !is_correct_message {
			return false;
		}

		let is_process_required = is_correct_message && data.oldest_unprocessed_nonce == data.latest_received_nonce;
		data.latest_received_nonce = nonce;
		self.storage.set_data(data);

		let payload_to_save = match is_process_required {
			true => {
				let message = Message {
					key: MessageKey {
						lane_id: self.storage.id(),
						nonce,
					},
					payload,
				};
				match processor.on_message_received(message) {
					MessageAction::Drop => None,
					MessageAction::Queue(message) => Some(message.payload),
				}
			},
			false => Some(payload),
		};

		if let Some(payload_to_save) = payload_to_save {
			self.storage.save_message(nonce, payload_to_save);
		}

		true
	}

	/// Process stored lane messages.
	///
	/// Stops processing either when all messages are processed, or when processor returns
	/// MessageAction::Queue.
	pub fn process_messages(
		&mut self,
		processor: &mut impl OnMessageReceived<Storage::Payload>,
	) {
		let mut anything_processed = false;
		let mut data = self.storage.data();
		while data.oldest_unprocessed_nonce != data.latest_received_nonce {
			let nonce = data.oldest_unprocessed_nonce;
			let payload = self.storage.message(&nonce).expect("message is referenced by lane; referenced message is not pruned; qed");
			let message = Message {
				key: MessageKey {
					lane_id: self.storage.id(),
					nonce,
				},
				payload,
			};

			let process_result = processor.on_message_received(message);
			if let MessageAction::Queue(_) = process_result {
				break;
			}

			self.storage.remove_message(&nonce);

			anything_processed = true;
			data.oldest_unprocessed_nonce += 1;
		}

		if anything_processed {
			self.storage.set_data(data);
		}
	}
}
