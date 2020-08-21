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

/// Origin of the message.
///
/// 3 bytes to uniquely identify a bridge which sent the message within current runtime.
/// This should be added by the delivery protocol - i.e. we should not rely
/// on this being part of the bridge message itself.
pub type BridgeOrigin = [u8; 3];

/// A generic trait to dispatch arbitrary messages delivered over the bridge.
pub trait MessageDispatch {
	/// A type of the message to be dispatched.
	type Message: codec::Decode;

	/// Dispatches the message internally.
	///
	/// `origin` is a short indication of the source of the message.
	///
	/// Returns `true` if the dispatch was succesful, `false` otherwise.
	fn dispatch(origin: BridgeOrigin, message: Self::Message) -> bool;
}
