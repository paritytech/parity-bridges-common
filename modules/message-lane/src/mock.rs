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

use crate::by_weight_dispatch::{ByWeightMessageDispatch, ByWeightMessageDispatchRuntimeStorage};
use crate::{DefaultInstance, Trait};

use bp_message_dispatch::MessageDispatch as WrappedMessageDispatch;
use bp_message_lane::{
	source_chain::{LaneMessageVerifier, MessageDeliveryAndDispatchPayment, TargetHeaderChain},
	target_chain::{MessageDispatch, MessageDispatchPayment, QueuedMessageDispatch, SourceHeaderChain},
	LaneId, Message, MessageData, MessageId, MessageNonce, MessageResult,
};
use bp_runtime::InstanceId;
use frame_support::{impl_outer_event, impl_outer_origin, parameter_types, weights::Weight};
use sp_core::H256;
use sp_runtime::{
	testing::Header as SubstrateHeader,
	traits::{BlakeTwo256, IdentityLookup},
	Perbill,
};

pub type AccountId = u64;
pub type TestPayload = (u64, Weight);
pub type TestMessageFee = u64;

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct TestRuntime;

mod message_lane {
	pub use crate::Event;
}

impl_outer_event! {
	pub enum TestEvent for TestRuntime {
		frame_system<T>,
		message_lane,
	}
}

impl_outer_origin! {
	pub enum Origin for TestRuntime where system = frame_system {}
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: Weight = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::one();
}

impl frame_system::Trait for TestRuntime {
	type Origin = Origin;
	type Index = u64;
	type Call = ();
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = SubstrateHeader;
	type Event = TestEvent;
	type BlockHashCount = BlockHashCount;
	type MaximumBlockWeight = MaximumBlockWeight;
	type DbWeight = ();
	type BlockExecutionWeight = ();
	type ExtrinsicBaseWeight = ();
	type MaximumExtrinsicWeight = MaximumBlockWeight;
	type AvailableBlockRatio = AvailableBlockRatio;
	type MaximumBlockLength = MaximumBlockLength;
	type Version = ();
	type ModuleToIndex = ();
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type BaseCallFilter = ();
	type SystemWeightInfo = ();
}

parameter_types! {
	pub const MaxMessagesToPruneAtOnce: u64 = 10;
}

impl Trait for TestRuntime {
	type Event = TestEvent;
	type Payload = TestPayload;
	type MaxMessagesToPruneAtOnce = MaxMessagesToPruneAtOnce;

	type MessageFee = TestMessageFee;
	type TargetHeaderChain = TestTargetHeaderChain;
	type LaneMessageVerifier = TestLaneMessageVerifier;
	type MessageDeliveryAndDispatchPayment = TestMessageDeliveryAndDispatchPayment;

	type SourceHeaderChain = TestSourceHeaderChain;
	type MessageDispatch = TestMessageDispatch;
	type QueuedMessageDispatch = TestQueuedMessageDispatch;
	type MessageDispatchPayment = TestMessageDispatchPayment;
}

/// Max(total weight of all messages across all tests).
pub const MAX_ALLOWED_WEIGHT: Weight = 0xFFFFFFFFFFFFFFFF;

/// Error that is returned by all test implementations.
pub const TEST_ERROR: &str = "Test error";

/// Instance id we're using in tests.
pub const TEST_INSTANCE_ID: InstanceId = *b"test";

/// Lane that we're using in tests.
pub const TEST_LANE_ID: LaneId = [0, 0, 0, 1];

/// Regular message payload that is not PAYLOAD_TO_QUEUE.
pub const REGULAR_PAYLOAD: TestPayload = (0, 50);

/// Payload that is rejected by `TestTargetHeaderChain`.
pub const PAYLOAD_REJECTED_BY_TARGET_CHAIN: TestPayload = (1, 50);

/// All messages with this payload are queued by TestMessageProcessor.
pub const PAYLOAD_TO_QUEUE: TestPayload = (42, 50);

/// Target header chain that is used in tests.
#[derive(Debug, Default)]
pub struct TestTargetHeaderChain;

impl TargetHeaderChain<TestPayload> for TestTargetHeaderChain {
	type Error = &'static str;

	type MessagesReceivingProof = Result<(LaneId, MessageNonce), ()>;
	type MessagesProcessingProof = Result<(LaneId, MessageNonce), ()>;

	fn verify_message(payload: &TestPayload) -> Result<(), Self::Error> {
		if *payload == PAYLOAD_REJECTED_BY_TARGET_CHAIN {
			Err(TEST_ERROR)
		} else {
			Ok(())
		}
	}

	fn verify_messages_receiving_proof(
		proof: Self::MessagesReceivingProof,
	) -> Result<(LaneId, MessageNonce), Self::Error> {
		proof.map_err(|_| TEST_ERROR)
	}

	fn verify_messages_processing_proof(
		proof: Self::MessagesProcessingProof,
	) -> Result<(LaneId, MessageNonce), Self::Error> {
		proof.map_err(|_| TEST_ERROR)
	}
}

/// Lane message verifier that is used in tests.
#[derive(Debug, Default)]
pub struct TestLaneMessageVerifier;

impl LaneMessageVerifier<AccountId, TestPayload, TestMessageFee> for TestLaneMessageVerifier {
	type Error = &'static str;

	fn verify_message(
		_submitter: &AccountId,
		delivery_and_dispatch_fee: &TestMessageFee,
		_lane: &LaneId,
		_payload: &TestPayload,
	) -> Result<(), Self::Error> {
		if *delivery_and_dispatch_fee != 0 {
			Ok(())
		} else {
			Err(TEST_ERROR)
		}
	}
}

/// Message fee payment system that is used in tests.
#[derive(Debug, Default)]
pub struct TestMessageDeliveryAndDispatchPayment;

impl TestMessageDeliveryAndDispatchPayment {
	/// Reject all payments.
	pub fn reject_payments() {
		frame_support::storage::unhashed::put(b":reject-message-fee:", &true);
	}

	/// Returns true if given fee has been paid by given relayer.
	pub fn is_fee_paid(submitter: AccountId, fee: TestMessageFee) -> bool {
		frame_support::storage::unhashed::get(b":message-fee:") == Some((submitter, fee))
	}
}

impl MessageDeliveryAndDispatchPayment<AccountId, TestMessageFee> for TestMessageDeliveryAndDispatchPayment {
	type Error = &'static str;

	fn pay_delivery_and_dispatch_fee(submitter: &AccountId, fee: &TestMessageFee) -> Result<(), Self::Error> {
		if frame_support::storage::unhashed::get(b":reject-message-fee:") == Some(true) {
			return Err(TEST_ERROR);
		}

		frame_support::storage::unhashed::put(b":message-fee:", &(submitter, fee));
		Ok(())
	}
}

/// Source header chain that is used in tests.
#[derive(Debug)]
pub struct TestSourceHeaderChain;

impl SourceHeaderChain<TestPayload, TestMessageFee> for TestSourceHeaderChain {
	type Error = &'static str;

	type MessagesProof = Result<Vec<Message<TestPayload, TestMessageFee>>, ()>;

	fn verify_messages_proof(
		proof: Self::MessagesProof,
	) -> Result<Vec<Message<TestPayload, TestMessageFee>>, Self::Error> {
		proof.map_err(|_| TEST_ERROR)
	}
}

/// Message dispatch that is passing all messages to ByWeightMessageDispatch. The only exceptions are
/// messages with PAYLOAD_TO_QUEUE payload, that are queued instead. And messages
#[derive(Debug)]
pub struct TestMessageDispatch {
	by_weight: ByWeightMessageDispatch<
		TestRuntime,
		DefaultInstance,
		Self,
		ByWeightMessageDispatchRuntimeStorage<TestRuntime, DefaultInstance>,
	>,
}

impl WrappedMessageDispatch<MessageId> for TestMessageDispatch {
	type Message = TestPayload;

	fn dispatch_weight(message: &Self::Message) -> Weight {
		message.1
	}

	fn dispatch(_bridge: InstanceId, _id: MessageId, message: Self::Message) -> Weight {
		message.1
	}
}

impl MessageDispatch<TestPayload, TestMessageFee> for TestMessageDispatch {
	fn with_allowed_weight(weight: Weight) -> Self {
		TestMessageDispatch {
			by_weight: ByWeightMessageDispatch::new(
				ByWeightMessageDispatchRuntimeStorage::<TestRuntime, DefaultInstance>::default(),
				TEST_INSTANCE_ID,
				weight,
			),
		}
	}

	fn weight_left(&self) -> Weight {
		self.by_weight.weight_left()
	}

	fn dispatch_weight(&self, message: &Message<TestPayload, TestMessageFee>) -> Weight {
		self.by_weight.dispatch_weight(message)
	}

	fn dispatch(
		&mut self,
		message: Message<TestPayload, TestMessageFee>,
	) -> MessageResult<TestPayload, TestMessageFee> {
		if message.data.payload == PAYLOAD_TO_QUEUE {
			MessageResult::NotProcessed(message)
		} else {
			self.by_weight.dispatch(message)
		}
	}
}

/// Queued message dispatch that is used in tests.
#[derive(Debug)]
pub struct TestQueuedMessageDispatch {
	force_first_message: bool,
	by_weight: ByWeightMessageDispatch<
		TestRuntime,
		DefaultInstance,
		TestMessageDispatch,
		ByWeightMessageDispatchRuntimeStorage<TestRuntime, DefaultInstance>,
	>,
}

impl QueuedMessageDispatch<TestPayload> for TestQueuedMessageDispatch {
	fn with_allowed_weight(weight: Weight) -> Self {
		Self {
			force_first_message: false,
			by_weight: ByWeightMessageDispatch::new(
				ByWeightMessageDispatchRuntimeStorage::<TestRuntime, DefaultInstance>::default(),
				TEST_INSTANCE_ID,
				weight,
			),
		}
	}

	fn with_any_weight() -> Self {
		Self {
			force_first_message: true,
			by_weight: ByWeightMessageDispatch::new(
				ByWeightMessageDispatchRuntimeStorage::<TestRuntime, DefaultInstance>::default(),
				TEST_INSTANCE_ID,
				MAX_ALLOWED_WEIGHT,
			),
		}
	}

	fn dispatch(&mut self) -> (MessageNonce, Weight) {
		self.by_weight.dispatch_queued(self.force_first_message)
	}
}

/// Message verifier that is used in tests.
#[derive(Debug)]
pub struct TestMessageDispatchPayment;

impl TestMessageDispatchPayment {
	/// Reject all payments.
	pub fn reject_payments() {
		frame_support::storage::unhashed::put(b":reject-dispatch-fee:", &true);
	}

	/// Returns true if given fee has been paid by given relayer.
	pub fn is_fee_paid(payer: AccountId, weight: Weight) -> bool {
		frame_support::storage::unhashed::get(b":dispatch-fee:") == Some((payer, weight))
	}
}

impl MessageDispatchPayment<AccountId> for TestMessageDispatchPayment {
	type Error = &'static str;

	fn pay_dispatch_fee(payer: &AccountId, weight: Weight) -> Result<(), Self::Error> {
		if frame_support::storage::unhashed::get(b":reject-dispatch-fee:") == Some(true) {
			return Err(TEST_ERROR);
		}

		frame_support::storage::unhashed::put(b":dispatch-fee:", &(payer, weight));
		Ok(())
	}
}

/// Return message data with valid fee for given payload.
pub fn message_data(payload: TestPayload) -> MessageData<TestPayload, TestMessageFee> {
	MessageData { payload, fee: 1 }
}

/// Run message lane test.
pub fn run_test<T>(test: impl FnOnce() -> T) -> T {
	let t = frame_system::GenesisConfig::default()
		.build_storage::<TestRuntime>()
		.unwrap();
	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(test)
}
