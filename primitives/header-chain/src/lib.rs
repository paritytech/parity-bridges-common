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

//! Defines traits which represent a common interface for Substrate pallets which want to
//! incorporate bridge functionality.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Codec, Decode, Encode, EncodeLike};
use core::clone::Clone;
use core::cmp::Eq;
use core::default::Default;
use core::fmt::Debug;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_finality_grandpa::{AuthorityList, SetId};
use sp_runtime::RuntimeDebug;
use sp_std::prelude::Vec;

/// A type that can be used as a parameter in a dispatchable function.
///
/// When using `decl_module` all arguments for call functions must implement this trait.
pub trait Parameter: Codec + EncodeLike + Clone + Eq + Debug {}
impl<T> Parameter for T where T: Codec + EncodeLike + Clone + Eq + Debug {}

/// A GRANDPA Authority List and ID.
#[derive(Default, Encode, Decode, RuntimeDebug, PartialEq, Clone)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct AuthoritySet {
	/// List of GRANDPA authorities for the current round.
	pub authorities: AuthorityList,
	/// Monotonic identifier of the current GRANDPA authority set.
	pub set_id: SetId,
}

impl AuthoritySet {
	/// Create a new GRANDPA Authority Set.
	pub fn new(authorities: AuthorityList, set_id: SetId) -> Self {
		Self { authorities, set_id }
	}
}

// Used by `currency-exchange`, maybe rename or remove?
/// A base trait for pallets which want to keep track of a full set of headers from a bridged chain.
pub trait BaseHeaderChain {
	/// Transaction type.
	type Transaction: Parameter;
	/// Transaction inclusion proof type.
	type TransactionInclusionProof: Parameter;

	/// Verify that transaction is a part of given block.
	///
	/// Returns Some(transaction) if proof is valid and None otherwise.
	fn verify_transaction_inclusion_proof(proof: &Self::TransactionInclusionProof) -> Option<Self::Transaction>;
}

/// A base trait for pallets which want to keep track of a full set of headers from a bridged chain.
pub trait HeaderChain<H> {
	/// Get the best finalized header known to the header chain.
	fn best_finalized() -> H;

	/// Get the best authority set known to the header chain.
	fn authority_set() -> AuthoritySet;

	fn import_header(header: H) -> Result<(), ()>;
	fn import_finality_proof(header: H, finality_proof: Vec<u8>) -> Result<(), ()>;
}

impl HeaderChain<()> for () {
	fn best_finalized() -> () {
		()
	}

	fn authority_set() -> AuthoritySet {
		unimplemented!()
	}

	fn import_header(_header: ()) -> Result<(), ()> {
		unimplemented!()
	}
	fn import_finality_proof(_header: (), _finality_proof: Vec<u8>) -> Result<(), ()> {
		unimplemented!()
	}
}

pub trait AncestryChecker<H, P> {
	fn are_ancestors(ancestor: &H, child: &H, proof: &P) -> bool;
}

impl AncestryChecker<(), ()> for () {
	fn are_ancestors(_ancestor: &(), _child: &(), _proof: &()) -> bool {
		true
	}
}

// #[cfg(tests)]
#[cfg(feature = "std")]
pub mod test_helpers {
	use finality_grandpa::voter_set::VoterSet;
	use sp_finality_grandpa::{AuthorityId, AuthorityList};
	use sp_keyring::Ed25519Keyring;

	pub fn extract_keyring(id: &AuthorityId) -> Ed25519Keyring {
		let mut raw_public = [0; 32];
		raw_public.copy_from_slice(id.as_ref());
		Ed25519Keyring::from_raw_public(raw_public).unwrap()
	}

	pub fn voter_set() -> VoterSet<AuthorityId> {
		VoterSet::new(authority_list()).unwrap()
	}

	pub fn authority_list() -> AuthorityList {
		vec![(alice(), 1), (bob(), 1), (charlie(), 1)]
	}

	pub fn alice() -> AuthorityId {
		Ed25519Keyring::Alice.public().into()
	}

	pub fn bob() -> AuthorityId {
		Ed25519Keyring::Bob.public().into()
	}

	pub fn charlie() -> AuthorityId {
		Ed25519Keyring::Charlie.public().into()
	}
}
