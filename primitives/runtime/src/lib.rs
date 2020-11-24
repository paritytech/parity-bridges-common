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

//! Primitives that may be used at (bridges) runtime level.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::Encode;
use sp_core::hash::H256;
use sp_io::hashing::blake2_256;

pub use chain::{BlockNumberOf, Chain, HashOf, HasherOf, HeaderOf};

mod chain;

/// Use this when something must be shared among all instances.
pub const NO_INSTANCE_ID: InstanceId = [0, 0, 0, 0];

/// Bridge-with-Rialto instance id.
pub const RIALTO_BRIDGE_INSTANCE: InstanceId = *b"rlto";

/// Bridge-with-Millau instance id.
pub const MILLAU_BRIDGE_INSTANCE: InstanceId = *b"mlau";

/// Call-dispatch module prefix.
pub const CALL_DISPATCH_MODULE_PREFIX: &[u8] = b"pallet-bridge/call-dispatch";

/// Message-lane module prefix.
pub const MESSAGE_LANE_MODULE_PREFIX: &[u8] = b"pallet-bridge/message-lane";

/// Id of deployed module instance. We have a bunch of pallets that may be used in
/// different bridges. E.g. message-lane pallet may be deployed twice in the same
/// runtime to bridge ThisChain with Chain1 and Chain2. Sometimes we need to be able
/// to identify deployed instance dynamically. This type is used for that.
pub type InstanceId = [u8; 4];

/// Type of accounts on the source chain.
pub enum SourceAccount<T> {
	/// An account that belongs to Root (priviledged origin).
	Root,
	/// A non-priviledged account.
	///
	/// The embedded account ID may or may not have a private key depending on the "owner" of the
	/// account (private key, pallet, proxy, etc.).
	Account(T),
}

/// Derive an account ID from a foreign account ID.
///
/// This function returns an encoded Blake2 hash. It is the responsibility of the caller to ensure
/// this can be succesfully decoded into an AccountId.
///
/// The `bridge_id` is used to provide extra entropy when producing account IDs. This helps prevent
/// AccountId collisions between different bridges on a single target chain.
pub fn derive_account_id<AccountId>(bridge_id: InstanceId, id: SourceAccount<AccountId>) -> H256
where
	AccountId: Encode,
{
	match id {
		SourceAccount::Root => ("root", bridge_id).using_encoded(blake2_256),
		SourceAccount::Account(id) => ("account", bridge_id, id).using_encoded(blake2_256),
	}
	.into()
}
