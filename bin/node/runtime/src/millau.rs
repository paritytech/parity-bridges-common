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

use crate::Hash as RuntimeHash;
use crate::Header as RuntimeHeader;
use bp_header_chain::{BridgeStorage, ChainVerifier};
use sp_finality_grandpa::GRANDPA_ENGINE_ID;
use sp_std::vec::Vec;

pub struct Millau;

// This will contain things like Authority set changes
type AuthorityInfo = Vec<u8>;

type FinalityProof = (Vec<u8>, Vec<u8>);

impl ChainVerifier for Millau {
	type Header = RuntimeHeader;
	type Extra = AuthorityInfo;
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

		if is_finalized { /* Walk through previous headers and mark them as final */ }

		/* save block, but mark as unfinalized */

		true
	}

	fn validate_header<S: BridgeStorage>(storage: &mut S, header: &Self::Header) -> bool {
		let best_finalized_number = storage.best_finalized_header().expect("TODO").number;
		if header.number < best_finalized_number {
			return false;
		}

		if storage.header_exists(header.hash()) {
			return false;
		}

		true
	}

	fn verify_finality<S: BridgeStorage>(storage: &mut S, header: &Self::Header, proof: &Self::Proof) -> bool {
		todo!()
	}
}
