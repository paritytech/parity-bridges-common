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

//! A common interface for all Bridge Message Dispatch modules.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

use bp_runtime::InstanceId;
use codec::{Decode, Encode};
use frame_support::RuntimeDebug;

/// Message dispatch weight.
pub type Weight = u64;

/// Message dispatch result.
#[derive(Encode, Decode, RuntimeDebug, Clone, PartialEq, Eq)]
pub struct MessageDispatchResult {
	/// Unspent dispatch weight. This weight that will be deducted from total delivery transaction
	/// weight, thus reducing the transaction cost. This shall not be zero in (at least) two cases:
	///
	/// 1) if message has been dispatched successfully, but post-dispatch weight is less than
	///    the weight, declared by the message sender;
	/// 2) if message has not been dispatched at all.
	pub unspent_weight: Weight,
	/// Whether the message dispatch fee has been paid during dispatch. This will be true if your
	/// configuration supports pay-dispatch-fee-at-target-chain option and message sender has enabled
	/// this option.
	pub dispatch_fee_paid_during_dispatch: bool,
}

/// A generic trait to dispatch arbitrary messages delivered over the bridge.
pub trait MessageDispatch<AccountId, MessageId> {
	/// A type of the message to be dispatched.
	type Message: codec::Decode;

	/// Estimate dispatch weight.
	///
	/// This function must: (1) be instant and (2) return correct upper bound
	/// of dispatch weight.
	fn dispatch_weight(message: &Self::Message) -> Weight;

	/// Dispatches the message internally.
	///
	/// `bridge` indicates instance of deployed bridge where the message came from.
	///
	/// `id` is a short unique identifier of the message.
	///
	/// If message is `Ok`, then it should be dispatched. If it is `Err`, then it's just
	/// a sign that some other component has rejected the message even before it has
	/// reached `dispatch` method (right now this may only be caused if we fail to decode
	/// the whole message).
	///
	/// Returns unspent dispatch weight.
	fn dispatch<P: FnOnce(&AccountId, Weight) -> Result<(), ()>>(
		bridge: InstanceId,
		id: MessageId,
		message: Result<Self::Message, ()>,
		pay_dispatch_fee: P,
	) -> MessageDispatchResult;
}
