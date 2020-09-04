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

use bp_message_lane::{
	BridgedHeaderChain, LaneId, LaneMessageVerifier, Message, MessageNonce, MessageResult, MessageDispatch,
};
use frame_support::{impl_outer_event, impl_outer_origin, parameter_types, weights::{DispatchClass, Weight}};
use sp_core::H256;
use sp_runtime::{
	testing::Header as SubstrateHeader,
	traits::{BlakeTwo256, IdentityLookup},
	Perbill,
};

use crate::by_weight_dispatcher::{OnWeightedMessageReceived, ByWeightQueuedMessagesProcessor};
use crate::Trait;

pub type AccountId = u64;
pub type TestPayload = (u64, Weight);

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
	type MaximumExtrinsicWeight = ();
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
	type BridgedHeaderChain = TestHeaderChain;
	type LaneMessageVerifier = TestMessageVerifier;
	type MessageDispatch = TestMessageProcessor;
	type ProcessQueuedMessages = ByWeightQueuedMessagesProcessor<TestRuntime, crate::DefaultInstance>;
}

/// Lane that we're using in tests.
pub const TEST_LANE_ID: LaneId = [0, 0, 0, 1];

/// Regular message payload that is not PAYLOAD_TO_QUEUE.
pub const REGULAR_PAYLOAD: TestPayload = (0, 0);

/// All messages with this payload are queued by TestMessageProcessor.
pub const PAYLOAD_TO_QUEUE: TestPayload = (42, 0);

/// All messages with this payload are queued by TestMessageProcessor at block#0, but
/// processed at all other blocks.
pub const PAYLOAD_TO_QUEUE_AT_0: TestPayload = (43, 0);

/// All messags with this payload are rejected by TestMessageVerifier.
pub const PAYLOAD_TO_REJECT: TestPayload = (44, 0);

/// Message processor that immediately handles all messages except messages with PAYLOAD_TO_QUEUE payload.
#[derive(Debug, Default)]
pub struct TestMessageProcessor;

impl MessageDispatch<TestPayload> for TestMessageProcessor {
	fn on_message_received(&mut self, message: Message<TestPayload>) -> MessageResult<TestPayload> {
		if message.payload == PAYLOAD_TO_QUEUE_AT_0 && frame_system::Module::<TestRuntime>::block_number() == 0 {
			MessageResult::NotProcessed(message)
		}
		else if message.payload == PAYLOAD_TO_QUEUE {
			MessageResult::NotProcessed(message)
		} else {
			frame_system::Module::<TestRuntime>::register_extra_weight_unchecked(
				message.payload.1,
				DispatchClass::Normal,
			);
			MessageResult::Processed(message.payload.1)
		}
	}
}

impl OnWeightedMessageReceived<TestPayload> for TestMessageProcessor {
	fn dispatch_weight(&self, message: &Message<TestPayload>) -> Weight {
		message.payload.1
	}
}

/// Header chain that is used in tests.
#[derive(Debug)]
pub struct TestHeaderChain;

impl BridgedHeaderChain<TestPayload> for TestHeaderChain {
	type Error = &'static str;

	type MessagesProof = Result<Vec<Message<TestPayload>>, ()>;
	type MessagesReceivingProof = Result<(LaneId, MessageNonce), ()>;
	type MessagesProcessingProof = Result<(LaneId, MessageNonce), ()>;

	fn verify_messages_proof(proof: Self::MessagesProof) -> Result<Vec<Message<TestPayload>>, Self::Error> {
		proof.map_err(|_| "Rejected by TestHeaderChain")
	}

	fn verify_messages_receiving_proof(
		proof: Self::MessagesReceivingProof,
	) -> Result<(LaneId, MessageNonce), Self::Error> {
		proof.map_err(|_| "Rejected by TestHeaderChain")
	}

	fn verify_messages_processing_proof(
		proof: Self::MessagesProcessingProof,
	) -> Result<(LaneId, MessageNonce), Self::Error> {
		proof.map_err(|_| "Rejected by TestHeaderChain")
	}
}

/// Message verifier that is used in tests.
#[derive(Debug)]
pub struct TestMessageVerifier;

impl LaneMessageVerifier<AccountId, TestPayload> for TestMessageVerifier {
	type Error = &'static str;

	fn verify_message(_submitter: &AccountId, _lane: &LaneId, payload: &TestPayload) -> Result<(), Self::Error> {
		if *payload == PAYLOAD_TO_REJECT {
			Err("Rejected by TestMessageVerifier")
		} else {
			Ok(())
		}
	}
}

/// Run message lane test.
pub fn run_test<T>(test: impl FnOnce() -> T) -> T {
	let t = frame_system::GenesisConfig::default()
		.build_storage::<TestRuntime>()
		.unwrap();
	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(test)
}
