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

//! A common interface for all Bridge Message Dispatch modules.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

/// Message dispatch weight.
pub type Weight = u64;

/// A generic trait to dispatch arbitrary messages delivered over the bridge.
pub trait MessageDispatch<MessageOrigin, MessageId> {
	/// A type of the message to be dispatched.
	type Message: codec::Decode;

	/// Dispatches the message internally.
	///
	/// `origin` is a short indication of the source of the message.
	///
	/// `id` is a short unique if of the message.
	///
	/// Returns post-dispatch (actual) message weight.
	fn dispatch(origin: MessageOrigin, id: MessageId, message: Self::Message) -> Weight;
}
