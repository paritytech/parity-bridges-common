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

use crate::DigestItem as RuntimeDigestItem;
use crate::Hash as RuntimeHash;
use crate::Header as RuntimeHeader;
use bp_header_chain::{BridgeStorage, ChainVerifier};
use sp_finality_grandpa::{AuthorityList, SetId, GRANDPA_ENGINE_ID};
use sp_std::vec::Vec;

pub struct Millau;

// The finality proof will consist of three things:
//	- Encoded Grandpa Justification
//	- Authority Set
//	- Authority Set ID
type FinalityProof = (Vec<u8>, AuthorityList, SetId);

impl ChainVerifier for Millau {
	type Header = RuntimeHeader;
	type Extra = Vec<u8>;
	type Proof = FinalityProof;

	fn import_header<S: BridgeStorage>(
		storage: &mut S,
		header: &Self::Header,
		extra_data: Option<Self::Extra>,
		finality_proof: Option<Self::Proof>,
	) -> bool {
		let is_valid = Self::validate_header(storage, &header);
		if !is_valid {
			return false;
		}

		let is_finalized = if let Some(proof) = finality_proof {
			Self::verify_finality(storage, &header, &proof)
		} else {
			false
		};

		if is_finalized { /* Walk through parent headers and mark them as final */ }

		/* save block, but mark as unfinalized */

		true
	}

	// Verify that the header we got sent is indeed a Substrate header. Not entirely sure
	// what I need to check to ensure it's valid though.
	fn validate_header<S: BridgeStorage>(storage: &mut S, header: &Self::Header) -> bool {
		let best_finalized_number = storage.best_finalized_header().expect("TODO").number;
		if header.number < best_finalized_number {
			return false;
		}

		if storage.header_exists(header.hash()) {
			return false;
		}

		// Does every header have a digest with the Consensus engine ID? If not, then this
		// should go in the `verify_finality` method.
		let digest = header.digest().logs().last();
		if let DigestItem::Consensus(id, item) = digest {
			if id == GRANDPA_ENGINE_ID {
				// Get current validator set
				// Check if this header triggers any validator set changes
			}
		} else {
			return false;
		}
	}

	fn verify_finality<S: BridgeStorage>(storage: &mut S, header: &Self::Header, proof: &Self::Proof) -> bool {
		// Should look into using the code from the Ethereum built-in contract
		// Will also need to check ancestry here
		todo!()
	}
}
