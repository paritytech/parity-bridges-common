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

impl HeaderVerifier for Millau {
	type Header = Header;
	type Extra = Vec<u8>;

	fn validate_header(header: Self::Header, extra_data: Option<Self::Extra>) -> bool {
		todo!()
	}
}

impl FinalityVerifier for Millau {
	type Header = Header;
	type Proof = Vec<u8>;

	fn verify_finality(header: Self::Header, proof: Self::Proof) -> bool {
		todo!()
	}
}
