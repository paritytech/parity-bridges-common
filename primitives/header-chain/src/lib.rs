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

use core::clone::Clone;
use core::cmp::Eq;
use core::fmt::Debug;
use parity_scale_codec::{Codec, EncodeLike};

/// A type that can be used as a parameter in a dispatchable function.
///
/// When using `decl_module` all arguments for call functions must implement this trait.
pub trait Parameter: Codec + EncodeLike + Clone + Eq + Debug {}
impl<T> Parameter for T where T: Codec + EncodeLike + Clone + Eq + Debug {}

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

// pub trait BridgeStorage {
// 	type Header;
// 	type Hash;
//
// 	fn best_finalized_header(&self) -> Option<Self::Header>;
// 	fn write_header(&mut self, header: Self::Header) -> bool;
// 	fn header_exists(&self, hash: Self::Hash) -> bool;
// 	// Maybe this one doesn't belong here...
// 	fn authority_set_id(&self) -> u64;
// }

// /// A trait for verifying whether a header is valid for a particular blockchain.
// pub trait ChainVerifier {
// 	type Header: Parameter;
// 	type Extra: Parameter;
// 	type Proof: Parameter;
//
// 	/// Import a header to the pallet.
// 	// TODO: This should return a result
// 	fn import_header<S: BridgeStorage>(
// 		storage: &mut S,
// 		header: &Self::Header,
// 		extra_data: Option<Self::Extra>,
// 		finality_proof: Option<Self::Proof>,
// 	) -> bool;
//
// 	/// Check that a standalone header is well-formed. This does not need to provide any sort
// 	/// of ancestry related verification.
// 	// TODO: This should return a result
// 	fn validate_header<S: BridgeStorage>(storage: &mut S, header: &Self::Header) -> bool;
//
// 	/// Verify that the given header has been finalized and is part of the canonical chain.
// 	// TODO: This should return a result
// 	fn verify_finality<S: BridgeStorage>(storage: &mut S, header: &Self::Header, proof: &Self::Proof) -> bool;
// }
