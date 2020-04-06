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

use bridge_node_runtime::{BlockNumber, Hash, Header as RuntimeHeader};
use finality_grandpa::VoterSet;
use sc_finality_grandpa::GrandpaJustification;
use sp_blockchain::Error as ClientError;
use sp_finality_grandpa::AuthorityList;

/// Builtin errors.
pub enum Error {
	/// Failed to decode Substrate header.
	HeaderDecode(codec::Error),
	/// Failed to decode best voters set.
	BestVotersDecode(codec::Error),
	/// Failed to decode finality proof.
	FinalityProofDecode(codec::Error),
	/// Failed to verify justification.
	JustificationVerify(ClientError),
	///
	NoNewHeaders,
}

/// Substrate header.
pub struct Header {
	/// Header number.
	pub number: BlockNumber,
}

/// All types of finality proofs.
pub enum FinalityProof {
	/// GRANDPA justification.
	Justification(Vec<u8>),
}

/// Parse Substrate header.
pub fn parse_substrate_header(raw_header: &[u8]) -> Result<Header, Error> {
	RuntimeHeader::decode(&mut &ras_header[..])
		.map(|header| Header {
			number: header.number,
		})
		.map_err(Error::HeaderDecode)
}

/// Verify GRANDPA finality proof.
pub fn verify_substrate_finality_proof(
	best_set_id: u64,
	raw_best_voters: &[u8],
	raw_best_header: &[u8],
	raw_headers: &[&[u8]],
	raw_finality_proof: &[u8],
) -> Result<(usize, usize), Error> {
	let best_voters = AuthorityList::decode(&mut &raw_best_voters[..])
		.map_err(Error::BestVotersDecode)?;
	let finality_proof = FinalityProof::decode(&mut &raw_finality_proof[..])
		.map_err(Error::FinalityProofDecode)?;

	let best_header = RuntimeHeader::decode(&mut &raw_best_header[..])
		.map_err(Error::HeaderDecode)?;
	let headers = raw_headers
		.iter()
		.map(|raw_header| RuntimeHeader::decode(&mut &raw_header[..])
			.map_err(Error::HeaderDecode)
		)
		.collect::<Result<Vec<_>, _>>()?;

	let finality_target = match headers.last() {
		Some(header) => (header.hash(), header.number()),
		None => return Err(Error::NoNewHeaders),
	};

	match finality_proof {
		FinalityProof::Justification(raw_justification) => {
			GrandpaJustification::decode_and_verify_finalizes(
				&raw_justification,
				finality_target,
				best_set_id,
				&VoterSet::new(best_voters)
			).map_err(Error::JustificationVerify)?;
		},
	}

	Ok((0, headers.len()))
}
