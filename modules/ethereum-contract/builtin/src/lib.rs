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

use bridge_node_runtime::{BlockNumber, Hash};
use sc_finality_grandpa::GrandpaJustification;

/// Builtin errors.
pub enum Error {
	/// Failed to decode Substrate header.
	HeaderDecode(codec::Error),
}

/// Substrate header.
pub struct Header {
	/// Header number.
	pub number: BlockNumber,
}

/// All types of finality proofs.
pub enum FinalityProof {
	/// GRANDPA justification.
	Justification(GrandpaJustification),
}

/// Parse Substrate header.
pub fn parse_substrate_header(raw_header: &[u8]) -> Result<Header, Error> {
	unimplemented!()
}

/// 
pub fn verify_finalization_data(
	best_header_hash: &Hash,
	headers: &[Header],
	raw_finalization_data: &[u8]
) -> Result<(usize, usize), Error> {
	unimplemented!()
}
