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

//! Message dispatcher that only cares about weights.

use crate::{Module, QueuedInboundLanes, Trait};

use bp_message_dispatch::MessageDispatch as WrappedMessageDispatch;
use bp_message_lane::{target_chain::MessageDispatch, LaneId, Message, MessageId, MessageNonce, MessageResult};
use bp_runtime::InstanceId;
use frame_support::{RuntimeDebug, storage::StorageValue, traits::{Get, Instance}, weights::Weight};
use sp_std::marker::PhantomData;

/// By-weight message dispatch storage.
pub trait ByWeightMessageDispatchStorage {
	/// Returns absolute maximum weight of single message. All messages that have weight above this
	/// limit are immediately dropped (during dispatch).
	fn max_dispatch_weight(&self) -> Weight;
	/// Append message lane to the end of the queue.
	fn enqueue_lane(&mut self, lane: LaneId);
	/// Return next queued message lane, removing it from the queue.
	fn take_next_queued_lane(&mut self) -> Option<LaneId>;
}

/// Message dispatcher that only cares about weights. If message fits within
/// allowed weight, it is dispatched immediately. Otherwise, message is queued
/// using message-lane module storage and will be processed later.
#[derive(RuntimeDebug)]
pub struct ByWeightMessageDispatch<T, I, WrappedDispatch, Storage> {
	storage: Storage,
	instance: InstanceId,
	weight: Weight,
	_phantom: PhantomData<(T, I, WrappedDispatch)>,
}

impl<T, I, WrappedDispatch, Storage> ByWeightMessageDispatch<T, I, WrappedDispatch, Storage>
where
	T: Trait<I>,
	I: Instance,
	WrappedDispatch: WrappedMessageDispatch<MessageId, Message = T::Payload>,
	Storage: ByWeightMessageDispatchStorage,
{
	/// Create new by-weight dispatch given bridge instance id and allowed weight.
	pub fn new(storage: Storage, instance: InstanceId, weight: Weight) -> Self {
		ByWeightMessageDispatch {
			storage,
			instance,
			weight,
			_phantom: Default::default(),
		}
	}

	/// Returns weight that is left.
	pub fn weight_left(&self) -> Weight {
		self.weight
	}

	/// Returns weight required to dispatch given message.
	pub fn dispatch_weight(&self, message: &Message<T::Payload, T::MessageFee>) -> Weight {
		WrappedDispatch::dispatch_weight(&message.data.payload)
	}

	/// If there's enough weight left, dispatch the message. Otherwise, enqueue it
	/// for later processal.
	pub fn dispatch(
		&mut self,
		message: Message<T::Payload, T::MessageFee>,
	) -> MessageResult<T::Payload, T::MessageFee> {
		let dispatch_weight = self.dispatch_weight(&message);
		if dispatch_weight > self.storage.max_dispatch_weight() {
			return MessageResult::Processed(0);
		}

		if dispatch_weight > self.weight {
			self.storage.enqueue_lane(message.key.lane_id);
			return MessageResult::NotProcessed(message);
		}

		MessageResult::Processed(self.dispatch_unchecked(message))
	}

	/// Dispatch queued messages until there's enough weight left. If `force_first_message` is
	/// true, then the first message is processed even if it doesn't fit available weight.
	pub fn dispatch_queued(&mut self, force_first_message: bool) -> (MessageNonce, Weight) {
		let mut dispatcher = ByWeightQueuedMessageDispatch {
			outer: self,
			force_next_message: force_first_message,
			messages_processed: 0,
			weight_spent: 0,
		};

		while let Some(lane) = dispatcher.outer.storage.take_next_queued_lane() {
			let is_empty_lane = Module::<T, I>::process_lane_messages(&lane, &mut dispatcher);

			if !is_empty_lane {
				// lane is enqueued internally, during dispatch() call
				break;
			}
		}

		(dispatcher.messages_processed, dispatcher.weight_spent)
	}

	/// Dispatch message without any checks.
	fn dispatch_unchecked(&mut self, message: Message<T::Payload, T::MessageFee>) -> Weight {
		let weight_spent = WrappedDispatch::dispatch(
			self.instance,
			(message.key.lane_id, message.key.nonce),
			message.data.payload,
		);
		self.weight = self.weight.saturating_sub(weight_spent);
		weight_spent
	}
}

/// Queued message dispatcher that is processing messages while they fit within given weight limit.
struct ByWeightQueuedMessageDispatch<'a, T, I, WrappedDispatch, Storage> {
	outer: &'a mut ByWeightMessageDispatch<T, I, WrappedDispatch, Storage>,
	force_next_message: bool,
	messages_processed: MessageNonce,
	weight_spent: Weight,
}

impl<'a, T, I, WrappedDispatch, Storage> MessageDispatch<T::Payload, T::MessageFee>
	for ByWeightQueuedMessageDispatch<'a, T, I, WrappedDispatch, Storage>
where
	T: Trait<I>,
	I: Instance,
	WrappedDispatch: WrappedMessageDispatch<MessageId, Message = T::Payload>,
	Storage: ByWeightMessageDispatchStorage,
{
	fn with_allowed_weight(_weight: Weight) -> Self {
		unreachable!("only created manually; qed")
	}

	fn weight_left(&self) -> Weight {
		self.outer.weight
	}

	fn dispatch_weight(&self, message: &Message<T::Payload, T::MessageFee>) -> Weight {
		self.outer.dispatch_weight(message)
	}

	fn dispatch(&mut self, message: Message<T::Payload, T::MessageFee>) -> MessageResult<T::Payload, T::MessageFee> {
		let message_result = if !self.force_next_message {
			self.outer.dispatch(message)
		} else {
			self.force_next_message = false;
			MessageResult::Processed(self.outer.dispatch_unchecked(message))
		};
		self.weight_spent = self.weight_spent.saturating_add(message_result.weight_spent());
		message_result
	}
}

/// By-weight message dispatch runtime-backed storage.
#[derive(RuntimeDebug)]
pub struct ByWeightMessageDispatchRuntimeStorage<T, I> {
	_phantom: PhantomData<(T, I)>,
}

impl<T: Trait<I>, I: Instance> Default for ByWeightMessageDispatchRuntimeStorage<T, I> {
	fn default() -> Self {
		ByWeightMessageDispatchRuntimeStorage {
			_phantom: Default::default(),
		}
	}
}

impl<T: Trait<I>, I: Instance> ByWeightMessageDispatchStorage for ByWeightMessageDispatchRuntimeStorage<T, I> {
	fn max_dispatch_weight(&self) -> Weight {
		<T as frame_system::Trait>::MaximumExtrinsicWeight::get()
	}

	fn enqueue_lane(&mut self, lane: LaneId) {
		QueuedInboundLanes::<I>::append(lane);
	}

	fn take_next_queued_lane(&mut self) -> Option<LaneId> {
		QueuedInboundLanes::<I>::mutate(|lanes| if lanes.is_empty() { None } else { Some(lanes.remove(0)) })
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::inbound_lane::ReceiveMessageResult;
	use crate::mock::{
		run_test, TestMessageDispatch, TestMessageFee, TestPayload, TestRuntime, MAX_ALLOWED_WEIGHT, REGULAR_PAYLOAD,
		TEST_INSTANCE_ID, TEST_LANE_ID,
	};
	use crate::{inbound_lane, DefaultInstance, InboundMessages};

	use bp_message_lane::{MessageData, MessageKey};
	use frame_support::StorageMap;

	fn message() -> Message<TestPayload, TestMessageFee> {
		Message {
			key: MessageKey {
				lane_id: TEST_LANE_ID,
				nonce: 1,
			},
			data: MessageData {
				payload: REGULAR_PAYLOAD,
				fee: 0,
			},
		}
	}

	#[test]
	fn available_weight_is_decreased_with_every_dispatch() {
		run_test(|| {
			let mut dispatch = TestMessageDispatch::with_allowed_weight(REGULAR_PAYLOAD.1 * 10 + 42);
			assert_eq!(
				dispatch.dispatch(message()),
				MessageResult::Processed(REGULAR_PAYLOAD.1)
			);
			assert_eq!(dispatch.weight_left(), REGULAR_PAYLOAD.1 * 9 + 42);
			assert_eq!(
				dispatch.dispatch(message()),
				MessageResult::Processed(REGULAR_PAYLOAD.1)
			);
			assert_eq!(dispatch.weight_left(), REGULAR_PAYLOAD.1 * 8 + 42);
		});
	}

	#[test]
	fn dispatch_weight_returns_correct_weight() {
		run_test(|| {
			assert_eq!(
				TestMessageDispatch::with_allowed_weight(0).dispatch_weight(&message()),
				REGULAR_PAYLOAD.1,
			);
		});
	}

	#[test]
	fn by_weight_is_not_processing_message_if_not_enough_weight_left() {
		run_test(|| {
			let mut dispatch = TestMessageDispatch::with_allowed_weight(REGULAR_PAYLOAD.1 - 1);
			assert_eq!(dispatch.dispatch(message()), MessageResult::NotProcessed(message()));
			assert_eq!(dispatch.weight_left(), REGULAR_PAYLOAD.1 - 1);
		});
	}

	#[test]
	fn queued_by_weight_is_processing_at_least_one_message() {
		run_test(|| {
			let mut dispatch = TestMessageDispatch::with_allowed_weight(REGULAR_PAYLOAD.1 - 1);
			let mut inbound_lane = inbound_lane::<TestRuntime, DefaultInstance>(TEST_LANE_ID);
			assert_eq!(
				inbound_lane.receive_message(1, message().data, &mut dispatch),
				ReceiveMessageResult::Queued,
			);
			assert_eq!(QueuedInboundLanes::<DefaultInstance>::get(), vec![TEST_LANE_ID]);

			let mut dispatch = ByWeightMessageDispatch::<TestRuntime, DefaultInstance, TestMessageDispatch, _>::new(
				ByWeightMessageDispatchRuntimeStorage::<TestRuntime, DefaultInstance>::default(),
				TEST_INSTANCE_ID,
				REGULAR_PAYLOAD.1 - 1,
			);
			dispatch.dispatch_queued(true);

			assert_eq!(QueuedInboundLanes::<DefaultInstance>::get(), Vec::<LaneId>::new());
		});
	}

	#[test]
	fn queued_by_weight_is_processing_as_much_messages_as_possible() {
		run_test(|| {
			let mut dispatch = TestMessageDispatch::with_allowed_weight(0);
			let mut inbound_lane = inbound_lane::<TestRuntime, DefaultInstance>(TEST_LANE_ID);
			assert_eq!(
				inbound_lane.receive_message(1, message().data, &mut dispatch),
				ReceiveMessageResult::Queued
			);
			assert_eq!(
				inbound_lane.receive_message(2, message().data, &mut dispatch),
				ReceiveMessageResult::Queued
			);
			assert_eq!(
				inbound_lane.receive_message(3, message().data, &mut dispatch),
				ReceiveMessageResult::Queued
			);
			assert_eq!(
				inbound_lane.receive_message(4, message().data, &mut dispatch),
				ReceiveMessageResult::Queued
			);

			assert!(InboundMessages::<TestRuntime>::contains_key(&MessageKey {
				lane_id: TEST_LANE_ID,
				nonce: 1
			}));
			assert!(InboundMessages::<TestRuntime>::contains_key(&MessageKey {
				lane_id: TEST_LANE_ID,
				nonce: 2
			}));
			assert!(InboundMessages::<TestRuntime>::contains_key(&MessageKey {
				lane_id: TEST_LANE_ID,
				nonce: 3
			}));
			assert!(InboundMessages::<TestRuntime>::contains_key(&MessageKey {
				lane_id: TEST_LANE_ID,
				nonce: 4
			}));
			assert_eq!(QueuedInboundLanes::<DefaultInstance>::get(), vec![TEST_LANE_ID]);

			let mut dispatch = ByWeightMessageDispatch::<TestRuntime, DefaultInstance, TestMessageDispatch, _>::new(
				ByWeightMessageDispatchRuntimeStorage::<TestRuntime, DefaultInstance>::default(),
				TEST_INSTANCE_ID,
				REGULAR_PAYLOAD.1 * 4 - 1,
			);
			dispatch.dispatch_queued(true);

			assert!(!InboundMessages::<TestRuntime>::contains_key(&MessageKey {
				lane_id: TEST_LANE_ID,
				nonce: 1
			}));
			assert!(!InboundMessages::<TestRuntime>::contains_key(&MessageKey {
				lane_id: TEST_LANE_ID,
				nonce: 2
			}));
			assert!(!InboundMessages::<TestRuntime>::contains_key(&MessageKey {
				lane_id: TEST_LANE_ID,
				nonce: 3
			}));
			assert!(InboundMessages::<TestRuntime>::contains_key(&MessageKey {
				lane_id: TEST_LANE_ID,
				nonce: 4
			}));
			assert_eq!(QueuedInboundLanes::<DefaultInstance>::get(), vec![TEST_LANE_ID]);
		});
	}

	#[test]
	fn too_heavy_message_is_rejected() {
		run_test(|| {
			let mut dispatch = TestMessageDispatch::with_allowed_weight(REGULAR_PAYLOAD.1);
			let mut message = message();
			message.data.payload.1 = MAX_ALLOWED_WEIGHT;
			assert_eq!(dispatch.dispatch(message), MessageResult::Processed(0));
			assert_eq!(dispatch.weight_left(), REGULAR_PAYLOAD.1);
		});
	}
}
