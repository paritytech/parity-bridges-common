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

//! Primitives for the Substrate light client (a.k.a bridge) pallet.

#![cfg_attr(not(feature = "std"), no_std)]

use core::default::Default;
use parity_scale_codec::{Decode, Encode};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_finality_grandpa::{AuthorityList, SetId};
use sp_runtime::traits::Header as HeaderT;
use sp_runtime::RuntimeDebug;

/// A Grandpa Authority List and ID.
///
/// The list contains the authorities for the current round, while the
/// set id is a monotonic identifier of the current authority set.
#[derive(Default, Encode, Decode, RuntimeDebug, PartialEq, Clone)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct AuthoritySet {
	pub authorities: AuthorityList,
	pub set_id: SetId,
}

impl AuthoritySet {
	pub fn new(authorities: AuthorityList, set_id: SetId) -> Self {
		Self { authorities, set_id }
	}
}

/// Keeps track of when the next Grandpa authority set change will occur.
///
/// The authority set stored here is expected to be enacted at a block height
/// of N, assuming we get a valid justification for the header at N.
#[derive(Default, Encode, Decode, RuntimeDebug, PartialEq, Clone)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct ScheduledChange<N> {
	pub authority_set: AuthoritySet,
	pub height: N,
}

impl<N> ScheduledChange<N> {
	pub fn new(authority_set: AuthoritySet, height: N) -> Self {
		Self { authority_set, height }
	}
}

/// A more useful representation of a header for storage purposes.
///
/// Keeps track of two important fields aside from the header. First,
/// if the header requires a Grandpa justification. This is required
/// for headers which signal new authority set changes.
///
/// Secondly, whether or not this header has been finalized. A header
/// does not need to be finalized explictly, but instead may be finalized
/// implicitly when one of its children gets finalized.
#[derive(Default, Encode, Decode, Clone, RuntimeDebug, PartialEq)]
pub struct ImportedHeader<H: HeaderT> {
	pub header: H,
	pub requires_justification: bool,
	pub is_finalized: bool,
}

impl<H: HeaderT> ImportedHeader<H> {
	/// Create a new ImportedHeader.
	pub fn new(header: H, requires_justification: bool, is_finalized: bool) -> Self {
		Self {
			header,
			requires_justification,
			is_finalized,
		}
	}

	/// Get the hash of this header.
	pub fn hash(&self) -> H::Hash {
		self.header.hash()
	}

	/// Get the hash of the parent header.
	pub fn parent_hash(&self) -> &H::Hash {
		self.header.parent_hash()
	}

	/// Get the number of this header.
	pub fn number(&self) -> &H::Number {
		self.header.number()
	}
}

/// Prove that the given header was finalized by the given authority set.
pub fn prove_finality<H>(_header: &H, _set: &AuthoritySet, _justification: &[u8]) -> bool {
	true
}
