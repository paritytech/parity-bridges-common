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

use crate::inbound_lane::{InboundLane, InboundLaneStorage, ReceiveMessageResult};
use crate::outbound_lane::{OutboundLane, OutboundLaneStorage};

use bp_message_lane::{
	InboundLaneData, LaneId, MessageData, MessageKey, MessageNonce, OutboundLaneData,
	source_chain::{LaneMessageVerifier, MessageDeliveryAndDispatchPayment, TargetHeaderChain},
	target_chain::{MessageDispatch, MessageDispatchPayment, QueuedMessageDispatch, SourceHeaderChain},
};
use frame_support::{decl_event, decl_module, decl_storage, traits::Get, Parameter, StorageMap, weights::Weight};
use frame_system::ensure_signed;
use sp_std::{marker::PhantomData, prelude::*};

pub mod by_weight_dispatch;

mod inbound_lane;
mod outbound_lane;

#[cfg(test)]
mod mock;

// TODO: update me (https://github.com/paritytech/parity-bridges-common/issues/78)
/// Upper bound of delivery transaction weight.
const DELIVERY_BASE_WEIGHT: Weight = 0;

/// The module configuration trait
pub trait Trait<I = DefaultInstance>: frame_system::Trait {
	// General types
	
	/// They overarching event type.
	type Event: From<Event> + Into<<Self as frame_system::Trait>::Event>;
	/// Message payload.
	type Payload: Parameter;
	/// Maximal number of messages that may be pruned during maintenance. Maintenance occurs
	/// whenever outbound lane is updated - i.e. when new message is sent, or receival is
	/// confirmed. The reason is that if you want to use lane, you should be ready to pay
	/// for it.
	type MaxMessagesToPruneAtOnce: Get<MessageNonce>;

	// Types that are used by outbound_lane (on source chain).

	/// Type of delivery_and_dispatch_fee on source chain.
	type MessageFee: Parameter;
	/// Target header chain.
	type TargetHeaderChain: TargetHeaderChain<Self::Payload>;
	/// Message payload verifier.
	type LaneMessageVerifier: LaneMessageVerifier<
		Self::AccountId,
		Self::Payload,
		Self::MessageFee,
	>;
	/// Message delivery payment.
	type MessageDeliveryAndDispatchPayment: MessageDeliveryAndDispatchPayment<
		Self::AccountId,
		Self::MessageFee,
	>;

	// Types that are used by inbound_lane (on target chain).

	/// Source header chain, as it is represented on target chain.
	type SourceHeaderChain: SourceHeaderChain<Self::Payload, Self::MessageFee>;
	/// Message dispatch.
	type MessageDispatch: MessageDispatch<Self::Payload, Self::MessageFee>;
	/// Queued message dispatch.
	type QueuedMessageDispatch: QueuedMessageDispatch<Self::Payload>;
	/// Message dispatch payment.
	type MessageDispatchPayment: MessageDispatchPayment<Self::AccountId>;
}

/// Shortcut to messages proof type for Trait.
type MessagesProofOf<T, I> = <<T as Trait<I>>::SourceHeaderChain as SourceHeaderChain<<T as Trait<I>>::Payload, <T as Trait<I>>::MessageFee>>::MessagesProof;
/// Shortcut to messages receiving proof type for Trait.
type MessagesReceivingProofOf<T, I> = <<T as Trait<I>>::TargetHeaderChain as TargetHeaderChain<<T as Trait<I>>::Payload>>::MessagesReceivingProof;
/// Shortcut to messages processing proof type for Trait.
type MessagesProcessingProofOf<T, I> = <<T as Trait<I>>::TargetHeaderChain as TargetHeaderChain<<T as Trait<I>>::Payload>>::MessagesProcessingProof;

decl_storage! {
	trait Store for Module<T: Trait<I>, I: Instance = DefaultInstance> as MessageLane {
		/// Map of lane id => inbound lane data.
		InboundLanes: map hasher(blake2_128_concat) LaneId => InboundLaneData;
		/// All stored (unprocessed) inbound messages.
		InboundMessages: map hasher(blake2_128_concat) MessageKey => Option<MessageData<T::Payload, T::MessageFee>>;
		/// Map of lane id => outbound lane data.
		OutboundLanes: map hasher(blake2_128_concat) LaneId => OutboundLaneData;
		/// All queued outbound messages.
		OutboundMessages: map hasher(blake2_128_concat) MessageKey => Option<MessageData<T::Payload, T::MessageFee>>;

		/// Set of unprocessed inbound lanes (i.e. inbound lanes that have unprocessed
		/// messages). It is used only by `ByWeightMessageDispatch`. So if you are not using
		/// this implementation, this will always be empty.
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
			let mut dispatcher = T::QueuedMessageDispatch::with_any_weight();
			let (queued_messages_dispatched, weight_spent_on_queued_dispatch) = dispatcher.dispatch();
			if queued_messages_dispatched != 0 {
				frame_support::debug::trace!(
					target: "runtime",
					"Spent {} weight on {} queued messages from on_initialize()",
					weight_spent_on_queued_dispatch,
					queued_messages_dispatched,
				);
			}

			weight_spent_on_queued_dispatch
		}

		/// Send message over lane.
		#[weight = 0] // TODO: update me (https://github.com/paritytech/parity-bridges-common/issues/78)
		pub fn send_message(
			origin,
			lane_id: LaneId,
			payload: T::Payload,
			delivery_and_dispatch_fee: T::MessageFee,
		) {
			let submitter = ensure_signed(origin)?;

			// let's first check if message can be delivered to target chain
			T::TargetHeaderChain::verify_message(&payload).map_err(|err| {
				frame_support::debug::trace!(
					target: "runtime",
					"Message to lane {:?} is rejected by target chain: {:?}",
					lane_id,
					err,
				);

				err.into()
			})?;

			// now let's enforce any additional lane rules
			T::LaneMessageVerifier::verify_message(
				&submitter,
				&delivery_and_dispatch_fee,
				&lane_id,
				&payload,
			).map_err(|err| {
				frame_support::debug::trace!(
					target: "runtime",
					"Message to lane {:?} is rejected by lane verifier: {:?}",
					lane_id,
					err,
				);

				err.into()
			})?;

			// let's withdraw delivery and dispatch fee from submitter
			T::MessageDeliveryAndDispatchPayment::pay_delivery_and_dispatch_fee(
				&submitter,
				&delivery_and_dispatch_fee,
			).map_err(|err| {
				frame_support::debug::trace!(
					target: "runtime",
					"Message to lane {:?} is rejected because submitter {} is unable to pay fee {:?}: {:?}",
					lane_id,
					submitter,
					delivery_and_dispatch_fee,
					err,
				);

				err.into()
			})?;

			// finally, save message in outbound storage and emit event
			let mut lane = outbound_lane::<T, I>(lane_id);
			let nonce = lane.send_message(MessageData {
				payload,
				fee: delivery_and_dispatch_fee,
			});
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
		#[weight = DELIVERY_BASE_WEIGHT + max_dispatch_weight]
		pub fn receive_messages_proof(origin, proof: MessagesProofOf<T, I>, max_dispatch_weight: Weight) {
			let relayer = ensure_signed(origin)?;

			// verify messages proof && convert proof into messages
			let messages = T::SourceHeaderChain::verify_messages_proof(proof).map_err(Into::into)?;

			// estimate cumulative weight of messages dispatch
			let mut dispatcher = T::MessageDispatch::with_allowed_weight(max_dispatch_weight);
			let messages_dispatch_weight: Weight = messages.iter().map(|m| dispatcher.dispatch_weight(m)).sum();

			// the submitter pays for dispatch in advance, so the whole `messages_dispatch_weight` should
			// be covered. But it has already paid regular fee for at least `max_dispatch_weight`, so we
			// only need to withdraw `messages_dispatch_weight - max_dispatch_weight`.
			let messages_dispatch_extra_weight = messages_dispatch_weight.saturating_sub(max_dispatch_weight);
			T::MessageDispatchPayment::pay_dispatch_fee(&relayer, messages_dispatch_extra_weight);

			// now let's process as many messages, as possible
			let total_messages = messages.len();
			let mut invalid_messages = 0;
			let mut processed_messages = 0;
			let mut queued_messages = 0;
			for message in messages {
				let mut lane = inbound_lane::<T, I>(message.key.lane_id);
				match lane.receive_message(message.key.nonce, message.data, &mut dispatcher) {
					ReceiveMessageResult::Invalid => invalid_messages += 1,
					ReceiveMessageResult::Processed => processed_messages += 1,
					ReceiveMessageResult::Queued => queued_messages += 1,
				}
			}

			let weight_left = dispatcher.weight_left();
			frame_support::debug::trace!(
				target: "runtime",
				"Received messages: total={}, invalid={}, processed={}, queued={}. Weight spent: {}",
				total_messages,
				invalid_messages,
				processed_messages,
				queued_messages,
				max_dispatch_weight.saturating_sub(weight_left),
			);

			// ok - we have processed or queued all bundled messages. But what if relayer acts aggressivly
			// (i.e. `max_dispatch_weight` is larger than `messages_dispatch_weight`), or if some messages
			// have been queued? Then we may have some capacity left from `max_dispatch_weight`.
			// => let's not waste it and try to process queued messages
			let mut queued_dispatcher = T::QueuedMessageDispatch::with_allowed_weight(weight_left);
			let (queued_messages_dispatched, weight_spent_on_queued_dispatch) = queued_dispatcher.dispatch();
			if queued_messages_dispatched != 0 {
				frame_support::debug::trace!(
					target: "runtime",
					"Spent {} weight on {} queued messages from receive-messages",
					weight_spent_on_queued_dispatch,
					queued_messages_dispatched,
				);
			}
		}

		/// Receive messages receiving proof from bridged chain.
		#[weight = 0] // TODO: update me (https://github.com/paritytech/parity-bridges-common/issues/78)
		pub fn receive_message_receiving_proof(origin, proof: MessagesReceivingProofOf<T, I>) {
			let _ = ensure_signed(origin)?;
			let (lane_id, nonce) = T::TargetHeaderChain::verify_messages_receiving_proof(proof).map_err(Into::into)?;

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
		pub fn receive_message_processing_proof(origin, proof: MessagesProcessingProofOf<T, I>) {
			let _ = ensure_signed(origin)?;
			let (lane_id, nonce) = T::TargetHeaderChain::verify_messages_processing_proof(proof).map_err(Into::into)?;

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
	pub fn process_lane_messages(lane_id: &LaneId, processor: &mut impl MessageDispatch<T::Payload, T::MessageFee>) -> (bool, Weight) {
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
	type MessageFee = T::MessageFee;

	fn id(&self) -> LaneId {
		self.lane_id
	}

	fn data(&self) -> InboundLaneData {
		InboundLanes::<I>::get(&self.lane_id)
	}

	fn set_data(&mut self, data: InboundLaneData) {
		InboundLanes::<I>::insert(&self.lane_id, data)
	}

	fn message(&self, nonce: &MessageNonce) -> Option<MessageData<T::Payload, T::MessageFee>> {
		InboundMessages::<T, I>::get(MessageKey {
			lane_id: self.lane_id,
			nonce: *nonce,
		})
	}

	fn save_message(&mut self, nonce: MessageNonce, message_data: MessageData<T::Payload, T::MessageFee>) {
		InboundMessages::<T, I>::insert(
			MessageKey {
				lane_id: self.lane_id,
				nonce,
			},
			message_data,
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
	type MessageFee = T::MessageFee;

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

	fn save_message(&mut self, nonce: MessageNonce, mesage_data: MessageData<T::Payload, T::MessageFee>) {
		OutboundMessages::<T, I>::insert(
			MessageKey {
				lane_id: self.lane_id,
				nonce,
			},
			mesage_data,
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
	use crate::by_weight_dispatcher::{ByWeightDispatcherStorage, ByWeightDispatcherRuntimeStorage};
	use crate::mock::{
		run_test, Origin, TestEvent, TestRuntime, PAYLOAD_TO_QUEUE, PAYLOAD_TO_QUEUE_AT_0, PAYLOAD_TO_REJECT, REGULAR_PAYLOAD, TEST_LANE_ID,
	};
	use bp_message_lane::Message;
	use frame_support::{assert_noop, assert_ok, traits::OnInitialize};
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
				<TestRuntime as frame_system::Trait>::MaximumBlockWeight::get(),
			));

			assert!(InboundMessages::<TestRuntime>::contains_key(&key));
		});
	}

	#[test]
	fn receive_messages_proof_rejects_invalid_proof() {
		run_test(|| {
			assert_noop!(
				Module::<TestRuntime>::receive_messages_proof(Origin::signed(1), Err(()), <TestRuntime as frame_system::Trait>::MaximumBlockWeight::get()),
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

	#[test]
	fn queued_messages_are_processed_from_on_initialize() {
		run_test(|| {
			let key = MessageKey { lane_id: TEST_LANE_ID, nonce: 1 };
			Module::<TestRuntime>::receive_messages_proof(
				Origin::signed(1),
				Ok(vec![Message {
					key: key.clone(),
					payload: PAYLOAD_TO_QUEUE_AT_0,
				}]),
				<TestRuntime as frame_system::Trait>::MaximumBlockWeight::get(),
			).unwrap();
			assert!(InboundMessages::<TestRuntime>::contains_key(&key));

			let mut storage = ByWeightDispatcherRuntimeStorage::<TestRuntime, DefaultInstance>::default();
			storage.append_unprocessed_lane(TEST_LANE_ID);

			System::<TestRuntime>::set_block_number(1);
			Module::<TestRuntime>::on_initialize(1);
			assert!(!InboundMessages::<TestRuntime>::contains_key(&key));
			assert!(storage.take_next_unprocessed_lane().is_none());
		});
	}
}
