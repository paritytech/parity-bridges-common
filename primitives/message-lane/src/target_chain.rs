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

//! Primitives of message lane module, that are used on the target chain.

use crate::{Message, MessageNonce, MessageResult};

use frame_support::{weights::Weight, Parameter};
use sp_std::{fmt::Debug, prelude::*};

/// Source chain API. Used by target chain, to verify source chain proofs.
///
/// All implementations of this trait should only work with finalized data that
/// can't change. Wrong implementation may lead to invalid lane states (i.e. lane
/// that's stuck) and/or processing messages without paying fees.
pub trait SourceHeaderChain<Payload, Fee> {
	/// Error type.
	type Error: Debug + Into<&'static str>;

	/// Proof that messages are sent from source chain.
	type MessagesProof: Parameter;

	/// Verify messages proof and return proved messages.
	///
	/// Messages vector is required to be sorted by nonce within each lane. Out-of-order
	/// messages will be rejected.
	fn verify_messages_proof(proof: Self::MessagesProof) -> Result<Vec<Message<Payload, Fee>>, Self::Error>;
}

/// Instant message dispatch API.
///
/// Used to process incoming messages in the message-delivery transaction.
pub trait MessageDispatch<Payload, Fee> {
	/// Creates instance of message dispatch, which can spent at most `weight` on
	/// messages dispatch. Spending more than weight is not allowed.
	fn with_allowed_weight(weight: Weight) -> Self;
	/// Return weight that is left at this moment. Initially it is `weight` from
	/// `with_allowed_weight` call, but it should be decreased after every successful
	/// `dispatch` call.
	fn weight_left(&self) -> Weight;
	/// Estimate dispatch weight.
	///
	/// This function must: (1) be instant and (2) return correct, but upper bound
	/// of dispatch weight.
	fn dispatch_weight(&self, message: &Message<Payload, Fee>) -> Weight;
	/// Called to process inbound message.
	///
	/// It is up to the implementers of this trait to determine whether the message
	/// is invalid (i.e. improperly encoded, has too large weight, ...) or not. And,
	/// if message is invalid, then it should be dropped immediately (by returning
	/// `MessageResult::Processed`), or it'll block the lane forever.
	///
	/// If `MessageDispatch` ever returns `MessageResult::NotProcessed`, then it should
	/// remember that it has queued this message and provide `QueuedMessageDispatch`
	/// implementation that uses this information later.
	///
	/// Decision whether to return `MessageResult::NotProcessed` from this function
	/// **MUST** be made instantly. We do not account weight, spent to make this
	/// decision.
	fn dispatch(&mut self, message: Message<Payload, Fee>) -> MessageResult<Payload, Fee>;
}

/// Queued message dispatch API.
pub trait QueuedMessageDispatch<Payload> {
	/// Creates instance of message dispatch, which can spent at most `weight` on
	/// messages dispatch. Spending more than weight is not allowed.
	fn with_allowed_weight(weight: Weight) -> Self;
	/// Creates instance of message dispatch, which could spent any weight on
	/// dispatching messages. "Any" here means that it **MUST** fit the block
	/// weight AND that it should not always take 100% of block weight (or it'll
	/// block all other transactions from including into block).
	fn with_any_weight() -> Self;
	/// Called to process queued messages.
	///
	/// Returns number of processed queued messages and weight 'spent' on that.
	fn dispatch(&mut self) -> (MessageNonce, Weight);
}

/// Message dispatch payment. It is called as a part of message-delivery transaction. Delivery
/// transaction submitter (message relay account) pays for the messages dispatch (probably
/// in advance).
pub trait MessageDispatchPayment<AccountId> {
	/// Error type.
	type Error: Debug + Into<&'static str>;

	/// Write-off fee for dispatching messages of given cumulative weight.
	fn pay_dispatch_fee(payer: &AccountId, weight: Weight) -> Result<(), Self::Error>;
}
