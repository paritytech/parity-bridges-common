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

//! Primitives of messages module, that are used on the target chain.

use crate::{LaneId, Message, MessageKey, MessagePayload, OutboundLaneData};

use bp_runtime::{messages::MessageDispatchResult, Size};
use codec::{Decode, Encode, Error as CodecError};
use frame_support::{weights::Weight, Parameter, RuntimeDebug};
use scale_info::TypeInfo;
use sp_std::{collections::btree_map::BTreeMap, fmt::Debug, prelude::*};

/// Proved messages from the source chain.
pub type ProvedMessages<Message> = BTreeMap<LaneId, ProvedLaneMessages<Message>>;

/// Proved messages from single lane of the source chain.
#[derive(RuntimeDebug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo)]
pub struct ProvedLaneMessages<Message> {
	/// Optional outbound lane state.
	pub lane_state: Option<OutboundLaneData>,
	/// Messages sent through this lane.
	pub messages: Vec<Message>,
}

/// Message data with decoded dispatch payload.
#[derive(RuntimeDebug)]
pub struct DispatchMessageData<DispatchPayload> {
	/// Result of dispatch payload decoding.
	pub payload: Result<DispatchPayload, CodecError>,
}

/// Message with decoded dispatch payload.
#[derive(RuntimeDebug)]
pub struct DispatchMessage<DispatchPayload> {
	/// Message key.
	pub key: MessageKey,
	/// Message data with decoded dispatch payload.
	pub data: DispatchMessageData<DispatchPayload>,
}

/// Source chain API. Used by target chain, to verify source chain proofs.
///
/// All implementations of this trait should only work with finalized data that
/// can't change. Wrong implementation may lead to invalid lane states (i.e. lane
/// that's stuck) and/or processing messages without paying fees.
pub trait SourceHeaderChain {
	/// Error type.
	type Error: Debug + Into<&'static str>;

	/// Proof that messages are sent from source chain. This may also include proof
	/// of corresponding outbound lane states.
	type MessagesProof: Parameter + Size;

	/// Verify messages proof and return proved messages.
	///
	/// Returns error if either proof is incorrect, or the number of messages in the proof
	/// is not matching the `messages_count`.
	///
	/// Messages vector is required to be sorted by nonce within each lane. Out-of-order
	/// messages will be rejected.
	///
	/// The `messages_count` argument verification (sane limits) is supposed to be made
	/// outside this function. This function only verifies that the proof declares exactly
	/// `messages_count` messages.
	fn verify_messages_proof(
		proof: Self::MessagesProof,
		messages_count: u32,
	) -> Result<ProvedMessages<Message>, Self::Error>;
}

/// Called when inbound message is received.
pub trait MessageDispatch<AccountId> {
	/// Decoded message payload type. Valid message may contain invalid payload. In this case
	/// message is delivered, but dispatch fails. Therefore, two separate types of payload
	/// (opaque `MessagePayload` used in delivery and this `DispatchPayload` used in dispatch).
	type DispatchPayload: Decode;

	/// Fine-grained result of single message dispatch (for better diagnostic purposes)
	type DispatchLevelResult: Clone + sp_std::fmt::Debug + Eq;

	/// Estimate dispatch weight.
	///
	/// This function must return correct upper bound of dispatch weight. The return value
	/// of this function is expected to match return value of the corresponding
	/// `From<Chain>InboundLaneApi::message_details().dispatch_weight` call.
	fn dispatch_weight(message: &mut DispatchMessage<Self::DispatchPayload>) -> Weight;

	/// Called when inbound message is received.
	///
	/// It is up to the implementers of this trait to determine whether the message
	/// is invalid (i.e. improperly encoded, has too large weight, ...) or not.
	///
	/// If your configuration allows paying dispatch fee at the target chain, then
	/// it must be paid inside this method to the `relayer_account`.
	fn dispatch(
		relayer_account: &AccountId,
		message: DispatchMessage<Self::DispatchPayload>,
	) -> MessageDispatchResult<Self::DispatchLevelResult>;
}

impl<Message> Default for ProvedLaneMessages<Message> {
	fn default() -> Self {
		ProvedLaneMessages { lane_state: None, messages: Vec::new() }
	}
}

impl<DispatchPayload: Decode> From<Message> for DispatchMessage<DispatchPayload> {
	fn from(message: Message) -> Self {
		DispatchMessage { key: message.key, data: message.payload.into() }
	}
}

impl<DispatchPayload: Decode> From<MessagePayload> for DispatchMessageData<DispatchPayload> {
	fn from(payload: MessagePayload) -> Self {
		DispatchMessageData { payload: DispatchPayload::decode(&mut &payload[..]) }
	}
}

/// Structure that may be used in place of `SourceHeaderChain` and `MessageDispatch` on chains,
/// where inbound messages are forbidden.
pub struct ForbidInboundMessages;

/// Error message that is used in `ForbidOutboundMessages` implementation.
const ALL_INBOUND_MESSAGES_REJECTED: &str =
	"This chain is configured to reject all inbound messages";

impl SourceHeaderChain for ForbidInboundMessages {
	type Error = &'static str;
	type MessagesProof = ();

	fn verify_messages_proof(
		_proof: Self::MessagesProof,
		_messages_count: u32,
	) -> Result<ProvedMessages<Message>, Self::Error> {
		Err(ALL_INBOUND_MESSAGES_REJECTED)
	}
}

impl<AccountId> MessageDispatch<AccountId> for ForbidInboundMessages {
	type DispatchPayload = ();
	type DispatchLevelResult = ();

	fn dispatch_weight(_message: &mut DispatchMessage<Self::DispatchPayload>) -> Weight {
		Weight::MAX
	}

	fn dispatch(
		_: &AccountId,
		_: DispatchMessage<Self::DispatchPayload>,
	) -> MessageDispatchResult<Self::DispatchLevelResult> {
		MessageDispatchResult { unspent_weight: Weight::zero(), dispatch_level_result: () }
	}
}
