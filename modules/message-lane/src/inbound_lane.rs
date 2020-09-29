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

use bp_message_lane::{
	target_chain::{DispatchMessage, DispatchMessageData, MessageDispatch},
	InboundLaneData, LaneId, MessageKey, MessageNonce, OutboundLaneData,
};

/// Inbound lane storage.
pub trait InboundLaneStorage {
	/// Delivery and dispatch fee type on source chain.
	type MessageFee;
	/// Id of relayer on source chain.
	type Relayer;

	/// Lane id.
	fn id(&self) -> LaneId;
	/// Return maximal number of unconfirmed messages in inbound lane.
	fn max_unconfirmed_messages(&self) -> MessageNonce;
	/// Get lane data from the storage.
	fn data(&mut self) -> InboundLaneData<Self::Relayer>;
	/// Update lane data in the storage.
	fn set_data(&mut self, data: InboundLaneData<Self::Relayer>);
}

/// Inbound messages lane.
pub struct InboundLane<S> {
	storage: S,
}

impl<S: InboundLaneStorage> InboundLane<S> {
	/// Create new inbound lane backed by given storage.
	pub fn new(storage: S) -> Self {
		InboundLane { storage }
	}

	/// Receive state of the corresponding outbound lane.
	pub fn receive_state_update(&mut self, outbound_lane_data: OutboundLaneData) -> Option<MessageNonce> {
		let mut data = self.storage.data();
		if outbound_lane_data.latest_received_nonce > data.latest_received_nonce {
			// this is something that should never happen if proofs are correct
			return None;
		}
		if outbound_lane_data.latest_received_nonce <= data.latest_confirmed_nonce {
			return None;
		}

		data.latest_confirmed_nonce = outbound_lane_data.latest_received_nonce;
		while data
			.relayers
			.front()
			.map(|(nonce, _)| *nonce <= data.latest_confirmed_nonce)
			.unwrap_or(false)
		{
			data.relayers.pop_front();
		}

		self.storage.set_data(data);
		Some(outbound_lane_data.latest_received_nonce)
	}

	/// Receive new message.
	pub fn receive_message<P: MessageDispatch<S::MessageFee>>(
		&mut self,
		relayer: S::Relayer,
		nonce: MessageNonce,
		message_data: DispatchMessageData<P::DispatchPayload, S::MessageFee>,
	) -> bool {
		let mut data = self.storage.data();
		let is_correct_message = nonce == data.latest_received_nonce + 1;
		if !is_correct_message {
			return false;
		}

		if self.storage.max_unconfirmed_messages() == data.relayers.len() as MessageNonce {
			return false;
		}

		data.latest_received_nonce = nonce;
		data.relayers.push_back((nonce, relayer));
		self.storage.set_data(data);

		P::dispatch(DispatchMessage {
			key: MessageKey {
				lane_id: self.storage.id(),
				nonce,
			},
			data: message_data,
		});

		true
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{
		inbound_lane,
		mock::{message_data, run_test, TestMessageDispatch, TestRuntime, REGULAR_PAYLOAD, TEST_LANE_ID, TEST_RELAYER},
		DefaultInstance, RuntimeInboundLaneStorage,
	};

	fn receive_regular_message(
		lane: &mut InboundLane<RuntimeInboundLaneStorage<TestRuntime, DefaultInstance>>,
		nonce: MessageNonce,
	) {
		assert!(lane.receive_message::<TestMessageDispatch>(TEST_RELAYER, nonce, message_data(REGULAR_PAYLOAD).into()));
	}

	#[test]
	fn receive_status_update_ignores_status_from_the_future() {
		run_test(|| {
			let mut lane = inbound_lane::<TestRuntime, _>(TEST_LANE_ID);
			receive_regular_message(&mut lane, 1);
			assert_eq!(
				lane.receive_state_update(OutboundLaneData {
					latest_received_nonce: 10,
					..Default::default()
				}),
				None,
			);

			assert_eq!(lane.storage.data().latest_confirmed_nonce, 0);
		});
	}

	#[test]
	fn receive_status_update_ignores_obsolete_status() {
		run_test(|| {
			let mut lane = inbound_lane::<TestRuntime, _>(TEST_LANE_ID);
			receive_regular_message(&mut lane, 1);
			receive_regular_message(&mut lane, 2);
			receive_regular_message(&mut lane, 3);
			assert_eq!(
				lane.receive_state_update(OutboundLaneData {
					latest_received_nonce: 3,
					..Default::default()
				}),
				Some(3),
			);
			assert_eq!(lane.storage.data().latest_confirmed_nonce, 3);

			assert_eq!(
				lane.receive_state_update(OutboundLaneData {
					latest_received_nonce: 3,
					..Default::default()
				}),
				None,
			);
			assert_eq!(lane.storage.data().latest_confirmed_nonce, 3);
		});
	}

	#[test]
	fn receive_status_update_works() {
		run_test(|| {
			let mut lane = inbound_lane::<TestRuntime, _>(TEST_LANE_ID);
			receive_regular_message(&mut lane, 1);
			receive_regular_message(&mut lane, 2);
			receive_regular_message(&mut lane, 3);
			assert_eq!(lane.storage.data().latest_confirmed_nonce, 0);
			assert_eq!(
				lane.storage.data().relayers,
				vec![(1, TEST_RELAYER), (2, TEST_RELAYER), (3, TEST_RELAYER)]
			);

			assert_eq!(
				lane.receive_state_update(OutboundLaneData {
					latest_received_nonce: 2,
					..Default::default()
				}),
				Some(2),
			);
			assert_eq!(lane.storage.data().latest_confirmed_nonce, 2);
			assert_eq!(lane.storage.data().relayers, vec![(3, TEST_RELAYER)]);

			assert_eq!(
				lane.receive_state_update(OutboundLaneData {
					latest_received_nonce: 3,
					..Default::default()
				}),
				Some(3),
			);
			assert_eq!(lane.storage.data().latest_confirmed_nonce, 3);
			assert_eq!(lane.storage.data().relayers, vec![]);
		});
	}

	#[test]
	fn fails_to_receive_message_with_incorrect_nonce() {
		run_test(|| {
			let mut lane = inbound_lane::<TestRuntime, _>(TEST_LANE_ID);
			assert!(!lane.receive_message::<TestMessageDispatch>(
				TEST_RELAYER,
				10,
				message_data(REGULAR_PAYLOAD).into()
			));
			assert_eq!(lane.storage.data().latest_received_nonce, 0);
		});
	}

	#[test]
	fn correct_message_is_processed_instantly() {
		run_test(|| {
			let mut lane = inbound_lane::<TestRuntime, _>(TEST_LANE_ID);
			receive_regular_message(&mut lane, 1);
			assert_eq!(lane.storage.data().latest_received_nonce, 1);
		});
	}
}
