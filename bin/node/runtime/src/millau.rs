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

use crate::Header;
use bp_header_chain::{FinalityVerifier, HeaderVerifier};
use sp_finality_grandpa::GRANDPA_ENGINE_ID;
use sp_std::vec::Vec;

pub struct Millau;

// This will contain things like Authority set changes
type AuthorityInfo = Vec<u8>;

type FinalityProof = (Vec<u8>, Vec<u8>);

impl ChainVerifier for Millau {
	type Header = ();
	type Extra = AuthorityInfo;
	type Proof = FinalityProof;

	fn import_header<S: BridgeStorage>(
		storage: &mut S,
		header: Self::Header,
		extra_data: Option<Self::Extra>,
		finality_proof: Option<Self::Proof>,
	) -> bool {
		let is_valid = Self::validate_header(&mut storage, header);
		if !is_valid {
			return false;
		}

		let is_finalized = Self::verify_finality();

		if is_finalized { /* Walk through previous headers and mark them as final */ }

		/* save block, but mark as unfinalized */

		true
	}

	fn validate_header<S: BridgeStorage>(
		storage: &mut S,
		header: &Self::Header,
		extra_data: &Option<Self::Extra>,
	) -> bool {
		let last_finalized_number = storage.last_finalized_header().number;
		if header.number < last_finalized_number {
			return false;
		}

		/* we've previously seen this header */
		// Don't want to import a header we've already seen
		if true {
			return false;
		}
	}

	fn verify_finality<S: Storage>(storage: &mut S, header: &Self::Header, proof: &Self::Proof) -> bool {
		todo!()
	}
}
