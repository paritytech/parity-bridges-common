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

//! Runtime module that allows sending and receiving messages using lane concept:
//!
//! 1) the message is sent using `send_message()` call;
//! 2) every outbound message is assigned nonce;
//! 3) the messages are stored in the storage;
//! 4) external component (relay) delivers messages to bridged chain;
//! 5) messages are processed in order (ordered by assigned nonce);
//! 6) relay may send proof-of-receiving and proof-of-processing back to this chain.
//!
//! Once message is sent, its progress can be tracked by looking at module events.
//! The assigned nonce is reported using `MessageAccepted` event. When message is
//! accepted by the bridged chain, `MessagesDelivered` is fired. When message is
//! processedby the bridged chain, `MessagesProcessed` by the bridged chain.

#![cfg_attr(not(feature = "std"), no_std)]

use crate::inbound_lane::{InboundLane, InboundLaneStorage};
use crate::outbound_lane::{OutboundLane, OutboundLaneStorage};

use bp_message_lane::{
	BridgedHeaderChain, InboundLaneData, LaneId, LaneMessageVerifier, MessageKey, MessageNonce, OnMessageReceived,
	OutboundLaneData, ProcessQueuedMessages,
};
use frame_support::{decl_event, decl_module, decl_storage, traits::Get, Parameter, StorageMap, weights::Weight};
use frame_system::ensure_signed;
use sp_std::{marker::PhantomData, prelude::*};

pub mod by_weight_dispatcher;

mod inbound_lane;
mod outbound_lane;

#[cfg(test)]
mod mock;

/// The module configuration trait
pub trait Trait<I = DefaultInstance>: frame_system::Trait {
	/// They overarching event type.
	type Event: From<Event> + Into<<Self as frame_system::Trait>::Event>;
	/// Message payload.
	type Payload: Parameter;
	/// Maximal number of messages that may be pruned during maintenance. Maintenance occurs
	/// whenever outbound lane is updated - i.e. when new message is sent, or receival is
	/// confirmed. The reason is that if you want to use lane, you should be ready to pay
	/// for it.
	type MaxMessagesToPruneAtOnce: Get<MessageNonce>;
	/// Bridged header chain.
	type BridgedHeaderChain: BridgedHeaderChain<Self::Payload>;
	/// Message paylaod verifier.
	type LaneMessageVerifier: LaneMessageVerifier<Self::AccountId, Self::Payload>;

	/// Called when message has been received.
	type OnMessageReceived: Default + OnMessageReceived<Self::Payload>;

	/// Queued messages processor. It is called during block initialization and may
	/// choose to process queued inbound messages, or just do nothing. It should
	/// return weight that has been 'spent' on processing queued messages.
	type ProcessQueuedMessages: Default + ProcessQueuedMessages<Self::Payload>;
}

decl_storage! {
	trait Store for Module<T: Trait<I>, I: Instance = DefaultInstance> as MessageLane {
		/// Map of lane id => inbound lane data.
		InboundLanes: map hasher(blake2_128_concat) LaneId => InboundLaneData;
		/// All stored (unprocessed) inbound messages.
		InboundMessages: map hasher(blake2_128_concat) MessageKey => Option<T::Payload>;
		/// Map of lane id => outbound lane data.
		OutboundLanes: map hasher(blake2_128_concat) LaneId => OutboundLaneData;
		/// All queued outbound messages.
		OutboundMessages: map hasher(blake2_128_concat) MessageKey => Option<T::Payload>;

		/// Set of unprocessed inbound lanes (i.e. inbound lanes that have unprocessed
		/// messages). It is used only by `ByWeightDispatcher`. So if you are not using
		/// this implementation of `OnMessageReceived`, this will always be empty.
		UnprocessedInboundLanes: Vec<LaneId>;
	}
}

decl_event!(
	pub enum Event {
		/// Message has been accepted and is waiting to be delivered.
		MessageAccepted(LaneId, MessageNonce),
		/// Messages in the inclusive range have been delivered to the bridged chain.
		MessagesDelivered(LaneId, MessageNonce, MessageNonce),
		/// Messages in the inclusive range have been processed by the bridged chain.
		MessagesProcessed(LaneId, MessageNonce, MessageNonce),
	}
);

decl_module! {
	pub struct Module<T: Trait<I>, I: Instance = DefaultInstance> for enum Call where origin: T::Origin {
		/// Deposit one of this module's events by using the default implementation.
		fn deposit_event() = default;

		/// Block initialization.
		fn on_initialize(now: T::BlockNumber) -> Weight {
			// TODO: seems like block current weight is increased somewhere else => we can't update
			// frame_system::BlockWeight directly!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!

			T::ProcessQueuedMessages::default().process_queued_messages()
		}

		/// Send message over lane.
		#[weight = 0] // TODO: update me (https://github.com/paritytech/parity-bridges-common/issues/78)
		pub fn send_message(
			origin,
			lane_id: LaneId,
			payload: T::Payload,
		) {
			let submitter = ensure_signed(origin)?;
			T::LaneMessageVerifier::verify_message(&submitter, &lane_id, &payload).map_err(|err| {
				frame_support::debug::trace!(
					target: "runtime",
					"Rejected message to lane {:?}: {:?}",
					lane_id,
					err,
				);

				err.into()
			})?;

			let mut lane = outbound_lane::<T, I>(lane_id);
			let nonce = lane.send_message(payload);
			lane.prune_messages(T::MaxMessagesToPruneAtOnce::get());

			frame_support::debug::trace!(
				target: "runtime",
				"Accepted message {} to lane {:?}",
				nonce,
				lane_id,
			);

			Self::deposit_event(Event::MessageAccepted(lane_id, nonce));
		}

		/// Receive messages proof from bridged chain.
		#[weight = 0] // TODO: update me (https://github.com/paritytech/parity-bridges-common/issues/78)
		pub fn receive_messages_proof(
			origin,
			proof: <<T as Trait<I>>::BridgedHeaderChain as BridgedHeaderChain<T::Payload>>::MessagesProof,
		) {
			let _ = ensure_signed(origin)?;
			let messages = T::BridgedHeaderChain::verify_messages_proof(proof).map_err(Into::into)?;
			let mut correct_messages = 0;
			let mut processor = T::OnMessageReceived::default();
			for message in messages {
				let mut lane = inbound_lane::<T, I>(message.key.lane_id);
				if lane.receive_message(message.key.nonce, message.payload, &mut processor) {
					correct_messages += 1;
				}
			}

			frame_support::debug::trace!(
				target: "runtime",
				"Received {} messages",
				correct_messages,
			);
		}

		/// Receive messages receiving proof from bridged chain.
		#[weight = 0] // TODO: update me (https://github.com/paritytech/parity-bridges-common/issues/78)
		pub fn receive_message_receiving_proof(
			origin,
			proof: <<T as Trait<I>>::BridgedHeaderChain as BridgedHeaderChain<T::Payload>>::MessagesReceivingProof,
		) {
			let _ = ensure_signed(origin)?;
			let (lane_id, nonce) = T::BridgedHeaderChain::verify_messages_receiving_proof(proof).map_err(Into::into)?;

			let mut lane = outbound_lane::<T, I>(lane_id);
			let received_range = lane.confirm_receival(nonce);
			if let Some(received_range) = received_range {
				Self::deposit_event(Event::MessagesDelivered(lane_id, received_range.0, received_range.1));
			}

			frame_support::debug::trace!(
				target: "runtime",
				"Received proof of receiving messages up to (and including) {} at lane {:?}",
				nonce,
				lane_id,
			);
		}

		/// Receive messages processing proof from bridged chain.
		#[weight = 0] // TODO: update me (https://github.com/paritytech/parity-bridges-common/issues/78)
		pub fn receive_message_processing_proof(
			origin,
			proof: <<T as Trait<I>>::BridgedHeaderChain as BridgedHeaderChain<T::Payload>>::MessagesProcessingProof,
		) {
			let _ = ensure_signed(origin)?;
			let (lane_id, nonce) = T::BridgedHeaderChain::verify_messages_processing_proof(proof).map_err(Into::into)?;

			let mut lane = outbound_lane::<T, I>(lane_id);
			let processed_range = lane.confirm_processing(nonce);
			if let Some(processed_range) = processed_range {
				Self::deposit_event(Event::MessagesProcessed(lane_id, processed_range.0, processed_range.1));
			}

			frame_support::debug::trace!(
				target: "runtime",
				"Received proof of processing messages up to (and including) {} at lane {:?}",
				nonce,
				lane_id,
			);
		}
	}
}

impl<T: Trait<I>, I: Instance> Module<T, I> {
	// =========================================================================================
	// === Exposed mutables ====================================================================
	// =========================================================================================

	/// Process stored lane messages.
	///
	/// Stops processing either when all messages are processed, or when processor returns
	/// MessageResult::NotProcessed.
	///
	/// Returns empty-lane flag and weight of all processed messages.
	pub fn process_lane_messages(lane_id: &LaneId, processor: &mut impl OnMessageReceived<T::Payload>) -> (bool, Weight) {
		inbound_lane::<T, I>(*lane_id).process_messages(processor)
	}
}

/// Creates new inbound lane object, backed by runtime storage.
fn inbound_lane<T: Trait<I>, I: Instance>(lane_id: LaneId) -> InboundLane<RuntimeInboundLaneStorage<T, I>> {
	InboundLane::new(RuntimeInboundLaneStorage {
		lane_id,
		_phantom: Default::default(),
	})
}

/// Creates new outbound lane object, backed by runtime storage.
fn outbound_lane<T: Trait<I>, I: Instance>(lane_id: LaneId) -> OutboundLane<RuntimeOutboundLaneStorage<T, I>> {
	OutboundLane::new(RuntimeOutboundLaneStorage {
		lane_id,
		_phantom: Default::default(),
	})
}

/// Runtime inbound lane storage.
struct RuntimeInboundLaneStorage<T, I = DefaultInstance> {
	lane_id: LaneId,
	_phantom: PhantomData<(T, I)>,
}

impl<T: Trait<I>, I: Instance> InboundLaneStorage for RuntimeInboundLaneStorage<T, I> {
	type Payload = T::Payload;

	fn id(&self) -> LaneId {
		self.lane_id
	}

	fn data(&self) -> InboundLaneData {
		InboundLanes::<I>::get(&self.lane_id)
	}

	fn set_data(&mut self, data: InboundLaneData) {
		InboundLanes::<I>::insert(&self.lane_id, data)
	}

	fn message(&self, nonce: &MessageNonce) -> Option<Self::Payload> {
		InboundMessages::<T, I>::get(MessageKey {
			lane_id: self.lane_id,
			nonce: *nonce,
		})
	}

	fn save_message(&mut self, nonce: MessageNonce, payload: T::Payload) {
		InboundMessages::<T, I>::insert(
			MessageKey {
				lane_id: self.lane_id,
				nonce,
			},
			payload,
		);
	}

	fn remove_message(&mut self, nonce: &MessageNonce) {
		InboundMessages::<T, I>::remove(MessageKey {
			lane_id: self.lane_id,
			nonce: *nonce,
		});
	}
}

/// Runtime outbound lane storage.
struct RuntimeOutboundLaneStorage<T, I = DefaultInstance> {
	lane_id: LaneId,
	_phantom: PhantomData<(T, I)>,
}

impl<T: Trait<I>, I: Instance> OutboundLaneStorage for RuntimeOutboundLaneStorage<T, I> {
	type Payload = T::Payload;

	fn id(&self) -> LaneId {
		self.lane_id
	}

	fn data(&self) -> OutboundLaneData {
		OutboundLanes::<I>::get(&self.lane_id)
	}

	fn set_data(&mut self, data: OutboundLaneData) {
		OutboundLanes::<I>::insert(&self.lane_id, data)
	}

	#[cfg(test)]
	fn message(&self, nonce: &MessageNonce) -> Option<Self::Payload> {
		OutboundMessages::<T, I>::get(MessageKey {
			lane_id: self.lane_id,
			nonce: *nonce,
		})
	}

	fn save_message(&mut self, nonce: MessageNonce, payload: T::Payload) {
		OutboundMessages::<T, I>::insert(
			MessageKey {
				lane_id: self.lane_id,
				nonce,
			},
			payload,
		);
	}

	fn remove_message(&mut self, nonce: &MessageNonce) {
		OutboundMessages::<T, I>::remove(MessageKey {
			lane_id: self.lane_id,
			nonce: *nonce,
		});
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{
		run_test, Origin, TestEvent, TestRuntime, PAYLOAD_TO_QUEUE, PAYLOAD_TO_REJECT, REGULAR_PAYLOAD, TEST_LANE_ID,
	};
	use bp_message_lane::Message;
	use frame_support::{assert_noop, assert_ok};
	use frame_system::{EventRecord, Module as System, Phase};

	fn send_regular_message() {
		System::<TestRuntime>::set_block_number(1);
		System::<TestRuntime>::reset_events();

		assert_ok!(Module::<TestRuntime>::send_message(
			Origin::signed(1),
			TEST_LANE_ID,
			REGULAR_PAYLOAD,
		));

		assert_eq!(
			System::<TestRuntime>::events(),
			vec![EventRecord {
				phase: Phase::Initialization,
				event: TestEvent::message_lane(Event::MessageAccepted(TEST_LANE_ID, 1)),
				topics: vec![],
			}],
		);
	}

	fn receive_message_receiving_proof() {
		System::<TestRuntime>::set_block_number(1);
		System::<TestRuntime>::reset_events();

		assert_ok!(Module::<TestRuntime>::receive_message_receiving_proof(
			Origin::signed(1),
			Ok((TEST_LANE_ID, 1)),
		));

		assert_eq!(
			System::<TestRuntime>::events(),
			vec![EventRecord {
				phase: Phase::Initialization,
				event: TestEvent::message_lane(Event::MessagesDelivered(TEST_LANE_ID, 1, 1)),
				topics: vec![],
			}],
		);
	}

	fn receive_message_processing_proof() {
		System::<TestRuntime>::set_block_number(1);
		System::<TestRuntime>::reset_events();

		assert_ok!(Module::<TestRuntime>::receive_message_processing_proof(
			Origin::signed(1),
			Ok((TEST_LANE_ID, 1)),
		));

		assert_eq!(
			System::<TestRuntime>::events(),
			vec![EventRecord {
				phase: Phase::Initialization,
				event: TestEvent::message_lane(Event::MessagesProcessed(TEST_LANE_ID, 1, 1)),
				topics: vec![],
			}],
		);
	}

	#[test]
	fn send_event_works() {
		run_test(|| {
			send_regular_message();
		});
	}

	#[test]
	fn send_event_rejects_invalid_message() {
		run_test(|| {
			assert_noop!(
				Module::<TestRuntime>::send_message(Origin::signed(1), TEST_LANE_ID, PAYLOAD_TO_REJECT,),
				"Rejected by TestMessageVerifier"
			);
		});
	}

	#[test]
	fn receive_messages_proof_works() {
		run_test(|| {
			let key = MessageKey {
				lane_id: TEST_LANE_ID,
				nonce: 1,
			};

			assert_ok!(Module::<TestRuntime>::receive_messages_proof(
				Origin::signed(1),
				Ok(vec![Message {
					key: key.clone(),
					payload: PAYLOAD_TO_QUEUE,
				}]),
			));

			assert!(InboundMessages::<TestRuntime>::contains_key(&key));
		});
	}

	#[test]
	fn receive_messages_proof_rejects_invalid_proof() {
		run_test(|| {
			assert_noop!(
				Module::<TestRuntime>::receive_messages_proof(Origin::signed(1), Err(()),),
				"Rejected by TestHeaderChain"
			);
		});
	}

	#[test]
	fn receive_messages_receiving_proof_works() {
		run_test(|| {
			send_regular_message();
			receive_message_receiving_proof();

			assert_eq!(
				OutboundLanes::<DefaultInstance>::get(&TEST_LANE_ID).latest_received_nonce,
				1,
			);
		});
	}

	#[test]
	fn receive_messages_receiving_proof_rejects_invalid_proof() {
		run_test(|| {
			assert_noop!(
				Module::<TestRuntime>::receive_message_receiving_proof(Origin::signed(1), Err(()),),
				"Rejected by TestHeaderChain"
			);
		});
	}

	#[test]
	fn receive_messages_processing_proof_works() {
		run_test(|| {
			send_regular_message();
			receive_message_receiving_proof();
			receive_message_processing_proof();

			assert_eq!(
				OutboundLanes::<DefaultInstance>::get(&TEST_LANE_ID).latest_processed_nonce,
				1,
			);
		});
	}

	#[test]
	fn receive_messages_processing_proof_rejects_invalid_proof() {
		run_test(|| {
			assert_noop!(
				Module::<TestRuntime>::receive_message_processing_proof(Origin::signed(1), Err(()),),
				"Rejected by TestHeaderChain"
			);
		});
	}
}
