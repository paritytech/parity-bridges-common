// Copyright 2020 Parity Technologies (UK) Ltd.
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

#![cfg_attr(not(feature = "std"), no_std)]

use crate::BridgeStorage;
use sp_finality_grandpa::{AuthorityList, SetId};
use sp_runtime::traits::Header as HeaderT;

pub type FinalityProof = (Vec<u8>, AuthorityList, SetId);

/// A trait for verifying whether a header is valid for a particular blockchain.
pub trait ChainVerifier<S, He> {
	/// Import a header to the pallet.
	// TODO: This should return a result
	fn import_header(storage: &mut S, header: &He, finality_proof: Option<FinalityProof>) -> bool;

	//	/// Check that a standalone header is well-formed. This does not need to provide any sort
	//	/// of ancestry related verification.
	//	// TODO: This should return a result
	//	fn validate_header<S: BridgeStorage>(storage: &mut S, header: &Self::Header) -> bool;
	//
	//	/// Verify that the given header has been finalized and is part of the canonical chain.
	//	// TODO: This should return a result
	//	fn verify_finality<S: BridgeStorage>(storage: &mut S, header: &Self::Header, proof: &Self::Proof) -> bool;
}

pub struct Verifier;

impl<S, He> ChainVerifier<S, He> for Verifier
where
	S: BridgeStorage<Header = He>,
	He: HeaderT,
{
	fn import_header(storage: &mut S, header: &He, finality_proof: Option<FinalityProof>) -> bool {
		let foo = header.hash();
		let boop = storage.write_header(header);
		todo!()
	}
}
