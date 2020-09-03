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

//! Message scheduler definitions and utilities.

// TODO: if weight is larger than MaxBlockWeight, drop it immediately!!!

use crate::{Trait, UnprocessedInboundLanes};

use bp_message_lane::{LaneId, Message, MessageResult, OnMessageReceived};
use frame_support::{
	storage::StorageValue,
	traits::{Get, Instance},
	weights::Weight,
};
use sp_std::marker::PhantomData;

/// Something that can estimate message dispatch weight.
pub trait OnWeightedMessageReceived<Payload>: OnMessageReceived<Payload> {
	/// Return upper-bound of message dispatch weight.
	fn dispatch_weight(&self, payload: &Message<Payload>) -> Weight;
}

/// By-weight dispatcher storage.
pub trait ByWeightDispatcherStorage {
	/// Appends given lane to the end of unprocessed lanes queue.
	fn append_unprocessed_lane(&mut self, lane: LaneId);
	/// Take next unprocessed lane from the queue.
	fn take_next_unprocessed_lane(&mut self) -> Option<LaneId>;
}

/// Weight limits traits.
pub trait WeightLimits {
	/// Returns current weight.
	fn current(&self) -> Weight;
	/// Returns maximal weight that we may try to fit in.
	fn max(&self) -> Weight;
	/// Returns maximal weight of message that may ever be processed.
	fn absolute_max(&self) -> Weight;
}

/// Message dispatcher that tries to dispatch message immediately if it fits current block.
/// If not, message is queued.
pub struct ByWeightDispatcher<Payload, Storage, Dispatcher, Limits> {
	storage: Storage,
	dispatcher: Dispatcher,
	limits: Limits,
	force_next_message_dispatch: bool,
	_phantom: PhantomData<Payload>,
}

impl<Payload, Storage, Dispatcher, Limits> ByWeightDispatcher<Payload, Storage, Dispatcher, Limits> {
	/// Creates new dispatcher.
	pub fn new(storage: Storage, dispatcher: Dispatcher, limits: Limits) -> Self {
		ByWeightDispatcher {
			storage,
			dispatcher,
			limits,
			force_next_message_dispatch: false,
			_phantom: Default::default(),
		}
	}

	/// Force dispatcher to dispatch next message immediately.
	pub fn force_next_message_dispatch(mut self) -> Self {
		self.force_next_message_dispatch = true;
		self
	}
}

impl<Payload, Storage, Dispatcher, Limits> OnMessageReceived<Payload>
	for ByWeightDispatcher<Payload, Storage, Dispatcher, Limits>
where
	Storage: ByWeightDispatcherStorage,
	Dispatcher: OnWeightedMessageReceived<Payload>,
	Limits: WeightLimits,
{
	fn on_message_received(&mut self, message: Message<Payload>) -> MessageResult<Payload> {
		let weight = self.dispatcher.dispatch_weight(&message);
		if weight > self.limits.absolute_max() {
			return MessageResult::Processed;
		}

		if !self.force_next_message_dispatch {
			let weight_left = self.limits.max().saturating_sub(self.limits.current());
			if weight > weight_left {
				self.storage.append_unprocessed_lane(message.key.lane_id);
				return MessageResult::NotProcessed(message);
			}
		}

		self.force_next_message_dispatch = false;
		if let MessageResult::NotProcessed(message) = self.dispatcher.on_message_received(message) {
			self.storage.append_unprocessed_lane(message.key.lane_id);
			return MessageResult::NotProcessed(message);
		}

		MessageResult::Processed
	}
}

/// Runtime-based storage implementation.
#[derive(Debug)]
pub struct ByWeightDispatcherRuntimeStorage<T, I> {
	_phantom: PhantomData<(T, I)>,
}

impl<T, I> Default for ByWeightDispatcherRuntimeStorage<T, I> {
	fn default() -> Self {
		ByWeightDispatcherRuntimeStorage {
			_phantom: Default::default(),
		}
	}
}

impl<T: Trait<I>, I: Instance> ByWeightDispatcherStorage for ByWeightDispatcherRuntimeStorage<T, I> {
	fn append_unprocessed_lane(&mut self, lane_id: LaneId) {
		UnprocessedInboundLanes::<I>::append(lane_id)
	}

	fn take_next_unprocessed_lane(&mut self) -> Option<LaneId> {
		UnprocessedInboundLanes::<I>::mutate(|lanes| if lanes.is_empty() { None } else { Some(lanes.remove(0)) })
	}
}

/// Storage that saves and returns nothing
#[derive(Debug)]
pub struct ByWeightDispatcherDumpStorage;

impl ByWeightDispatcherStorage for ByWeightDispatcherDumpStorage {
	fn append_unprocessed_lane(&mut self, _lane: LaneId) {}

	fn take_next_unprocessed_lane(&mut self) -> Option<LaneId> {
		None
	}
}

/// Runtime weight limits.
#[derive(Debug)]
pub struct ByWeightDispatcherRuntimeLimits<T> {
	_phantom: PhantomData<T>,
}

impl<T> Default for ByWeightDispatcherRuntimeLimits<T> {
	fn default() -> Self {
		ByWeightDispatcherRuntimeLimits {
			_phantom: Default::default(),
		}
	}
}

impl<T: Trait> WeightLimits for ByWeightDispatcherRuntimeLimits<T> {
	fn current(&self) -> Weight {
		frame_system::Module::<T>::block_weight().total()
	}

	fn max(&self) -> Weight {
		T::MaximumBlockWeight::get()
	}

	fn absolute_max(&self) -> Weight {
		self.max()
	}
}

/// Weight limits with custom max value.
#[derive(Debug)]
pub struct ByWeightDispatcherCustomLimits<T> {
	max: Weight,
	_phantom: PhantomData<T>,
}

impl<T> ByWeightDispatcherCustomLimits<T> {
	/// Create weight limits with custom max.
	pub fn new(max: Weight) -> Self {
		ByWeightDispatcherCustomLimits {
			max,
			_phantom: Default::default(),
		}
	}
}

impl<T: Trait> WeightLimits for ByWeightDispatcherCustomLimits<T> {
	fn current(&self) -> Weight {
		frame_system::Module::<T>::block_weight().total()
	}

	fn max(&self) -> Weight {
		self.max
	}

	fn absolute_max(&self) -> Weight {
		T::MaximumBlockWeight::get()
	}
}

/// Process as much scheduled messages, as possible given scheduler parameters and
/// current block state. This function will always process at least one message if
/// there are any queued messages in the storage.
///
/// The function tries to processes **all** messages of first queued lane. Then it
/// proceeds to the next lane. If it fails to process all lane messages in single
/// call, lane is moved to the end of scheduled lanes queue.
///
/// **CAUTION**: previous paragraph implies that if underlying dispatcher isn't
/// processing message for any reason, then all other non-empty lanes would be blocked at
/// least during this call. So combining this function with implementations of
/// `OnMessageReceived` that have its own logic of message processing, could lead
/// to significant delays in message dispatch.
pub fn process_scheduled_messages<Payload, Limits, Dispatcher, ProcessLaneMessages>(
	mut storage: impl ByWeightDispatcherStorage,
	limits: Limits,
	dispatcher: Dispatcher,
	process_lane_messages: ProcessLaneMessages,
) where
	Limits: WeightLimits,
	Dispatcher: OnWeightedMessageReceived<Payload>,
	ProcessLaneMessages:
		Fn(&LaneId, &mut ByWeightDispatcher<Payload, ByWeightDispatcherDumpStorage, Dispatcher, Limits>) -> bool,
{
	let mut dispatcher =
		ByWeightDispatcher::new(ByWeightDispatcherDumpStorage, dispatcher, limits).force_next_message_dispatch();
	while let Some(lane) = storage.take_next_unprocessed_lane() {
		let is_empty_lane = process_lane_messages(&lane, &mut dispatcher);
		if !is_empty_lane {
			storage.append_unprocessed_lane(lane);
			break;
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::inbound_lane;
	use crate::inbound_lane::InboundLaneStorage;
	use crate::mock::{run_test, TestMessageProcessor, TestPayload, TestRuntime, PAYLOAD_TO_QUEUE, REGULAR_PAYLOAD, TEST_LANE_ID};

	use frame_support::weights::DispatchClass;

	const CURRENT_WEIGHT: Weight = 1000;
	const HEAVY_PAYLOAD: TestPayload = (0, 800);

	fn dispatcher() -> ByWeightDispatcher<
		TestPayload,
		ByWeightDispatcherRuntimeStorage<TestRuntime, crate::DefaultInstance>,
		TestMessageProcessor,
		ByWeightDispatcherRuntimeLimits<TestRuntime>,
	> {
		ByWeightDispatcher::new(
			ByWeightDispatcherRuntimeStorage::default(),
			TestMessageProcessor,
			ByWeightDispatcherRuntimeLimits::default(),
		)
	}

	#[test]
	fn by_weight_dispatcher_process_message_immediately_if_it_fits() {
		run_test(|| {
			let mut lane = inbound_lane::<TestRuntime, _>(TEST_LANE_ID);
			let mut dispatcher = dispatcher();
			assert!(lane.receive_message(1, REGULAR_PAYLOAD, &mut dispatcher));
			assert_eq!(lane.storage().data().latest_received_nonce, 1);
			assert_eq!(lane.storage().data().oldest_unprocessed_nonce, 2);
			assert_eq!(dispatcher.storage.take_next_unprocessed_lane(), None);
		});
	}

	#[test]
	fn by_weight_dispatcher_enqueues_message_if_it_doesnt_fit() {
		run_test(|| {
			frame_system::Module::<TestRuntime>::register_extra_weight_unchecked(CURRENT_WEIGHT, DispatchClass::Normal);

			let mut lane = inbound_lane::<TestRuntime, _>(TEST_LANE_ID);
			let mut dispatcher = dispatcher();
			assert!(lane.receive_message(1, HEAVY_PAYLOAD, &mut dispatcher));
			assert_eq!(lane.storage().data().latest_received_nonce, 1);
			assert_eq!(lane.storage().data().oldest_unprocessed_nonce, 1);
			assert_eq!(dispatcher.storage.take_next_unprocessed_lane(), Some(TEST_LANE_ID));
		});
	}

	#[test]
	fn by_weight_dispatcher_enqueues_message_if_it_is_not_processed_by_backend() {
		run_test(|| {
			let mut lane = inbound_lane::<TestRuntime, _>(TEST_LANE_ID);
			let mut dispatcher = dispatcher();
			assert!(lane.receive_message(1, PAYLOAD_TO_QUEUE, &mut dispatcher));
			assert_eq!(lane.storage().data().latest_received_nonce, 1);
			assert_eq!(lane.storage().data().oldest_unprocessed_nonce, 1);
			assert_eq!(dispatcher.storage.take_next_unprocessed_lane(), Some(TEST_LANE_ID));
		});
	}

	#[test]
	fn process_scheduled_messages_always_processing_at_least_one_message() {
		run_test(|| {
			frame_system::Module::<TestRuntime>::register_extra_weight_unchecked(1000, DispatchClass::Normal);

			let mut lane = inbound_lane::<TestRuntime, _>(TEST_LANE_ID);
			let mut dispatcher = dispatcher();
			assert!(lane.receive_message(1, HEAVY_PAYLOAD, &mut dispatcher));
			assert!(lane.receive_message(2, HEAVY_PAYLOAD, &mut dispatcher));
			assert_eq!(lane.storage().data().latest_received_nonce, 2);
			assert_eq!(lane.storage().data().oldest_unprocessed_nonce, 1);

			process_scheduled_messages(
				ByWeightDispatcherRuntimeStorage::<TestRuntime, crate::DefaultInstance>::default(),
				ByWeightDispatcherRuntimeLimits::<TestRuntime>::default(),
				TestMessageProcessor,
				crate::Module::<TestRuntime, crate::DefaultInstance>::process_lane_messages,
			);

			assert_eq!(lane.storage().data().latest_received_nonce, 2);
			assert_eq!(lane.storage().data().oldest_unprocessed_nonce, 2);
		});
	}

	#[test]
	fn process_scheduled_messages_works_with_custom_limits() {
		run_test(|| {
			frame_system::Module::<TestRuntime>::register_extra_weight_unchecked(CURRENT_WEIGHT, DispatchClass::Normal);

			let mut lane = inbound_lane::<TestRuntime, _>(TEST_LANE_ID);
			let mut dispatcher = dispatcher();
			assert!(lane.receive_message(1, HEAVY_PAYLOAD, &mut dispatcher));
			assert!(lane.receive_message(2, HEAVY_PAYLOAD, &mut dispatcher));
			assert_eq!(lane.storage().data().latest_received_nonce, 2);
			assert_eq!(lane.storage().data().oldest_unprocessed_nonce, 1);

			process_scheduled_messages(
				ByWeightDispatcherRuntimeStorage::<TestRuntime, crate::DefaultInstance>::default(),
				ByWeightDispatcherCustomLimits::<TestRuntime>::new(CURRENT_WEIGHT + HEAVY_PAYLOAD.1 * 2),
				TestMessageProcessor,
				crate::Module::<TestRuntime, crate::DefaultInstance>::process_lane_messages,
			);

			assert_eq!(lane.storage().data().latest_received_nonce, 2);
			assert_eq!(lane.storage().data().oldest_unprocessed_nonce, 3);
		});
	}

	#[test]
	fn process_scheduled_messages_is_able_to_serve_several_lanes() {
		run_test(|| {
			frame_system::Module::<TestRuntime>::register_extra_weight_unchecked(CURRENT_WEIGHT, DispatchClass::Normal);

			const TEST_LANE_ID_2: LaneId = [0, 0, 0, 2];
			const TEST_LANE_ID_3: LaneId = [0, 0, 0, 3];

			let mut dispatcher = dispatcher();

			let mut lane1 = inbound_lane::<TestRuntime, _>(TEST_LANE_ID);
			assert!(lane1.receive_message(1, HEAVY_PAYLOAD, &mut dispatcher));
			assert!(lane1.receive_message(2, HEAVY_PAYLOAD, &mut dispatcher));
			assert_eq!(lane1.storage().data().latest_received_nonce, 2);
			assert_eq!(lane1.storage().data().oldest_unprocessed_nonce, 1);

			let mut lane2 = inbound_lane::<TestRuntime, _>(TEST_LANE_ID_2);
			assert!(lane2.receive_message(1, HEAVY_PAYLOAD, &mut dispatcher));
			assert!(lane2.receive_message(2, HEAVY_PAYLOAD, &mut dispatcher));
			assert!(lane2.receive_message(3, HEAVY_PAYLOAD, &mut dispatcher));
			assert_eq!(lane2.storage().data().latest_received_nonce, 3);
			assert_eq!(lane2.storage().data().oldest_unprocessed_nonce, 1);

			let mut lane3 = inbound_lane::<TestRuntime, _>(TEST_LANE_ID_3);
			assert!(lane3.receive_message(1, HEAVY_PAYLOAD, &mut dispatcher));
			assert!(lane3.receive_message(2, HEAVY_PAYLOAD, &mut dispatcher));
			assert_eq!(lane3.storage().data().latest_received_nonce, 2);
			assert_eq!(lane3.storage().data().oldest_unprocessed_nonce, 1);

			process_scheduled_messages(
				ByWeightDispatcherRuntimeStorage::<TestRuntime, crate::DefaultInstance>::default(),
				ByWeightDispatcherCustomLimits::<TestRuntime>::new(CURRENT_WEIGHT + HEAVY_PAYLOAD.1 * 6),
				TestMessageProcessor,
				crate::Module::<TestRuntime, crate::DefaultInstance>::process_lane_messages,
			);

			assert_eq!(lane1.storage().data().latest_received_nonce, 2);
			assert_eq!(lane1.storage().data().oldest_unprocessed_nonce, 3);

			assert_eq!(lane2.storage().data().latest_received_nonce, 3);
			assert_eq!(lane2.storage().data().oldest_unprocessed_nonce, 4);

			assert_eq!(lane3.storage().data().latest_received_nonce, 2);
			assert_eq!(lane3.storage().data().oldest_unprocessed_nonce, 2);

			assert_eq!(dispatcher.storage.take_next_unprocessed_lane(), Some(TEST_LANE_ID_3));
			assert_eq!(dispatcher.storage.take_next_unprocessed_lane(), None);
		});
	}
}
