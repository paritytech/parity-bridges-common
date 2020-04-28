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

use bridge_node_runtime::{Block, BlockNumber, Hash, Header as RuntimeHeader};
use codec::{Decode, Encode};
use ethereum_types::U256;
use sp_blockchain::Error as ClientError;
use sp_finality_grandpa::{AuthorityList, ConsensusLog, GRANDPA_ENGINE_ID};

/// Builtin errors.
#[derive(Debug)]
pub enum Error {
	/// Failed to decode block number.
	BlockNumberDecode,
	/// Failed to decode Substrate header.
	HeaderDecode(codec::Error),
	/// Failed to decode best voters set.
	BestSetDecode(codec::Error),
	/// Failed to decode finality proof.
	FinalityProofDecode(codec::Error),
	/// Failed to verify justification.
	JustificationVerify(ClientError),
}

/// Substrate header.
#[derive(Debug)]
pub struct Header {
	/// Header hash.
	pub hash: Hash,
	/// Parent header hash.
	pub parent_hash: Hash,
	/// Header number.
	pub number: BlockNumber,
	/// GRANDPA validators change signal.
	pub signal: Option<ValidatorsSetSignal>,
}

/// GRANDPA validators set change signal.
#[derive(Debug)]
pub struct ValidatorsSetSignal {
	/// Signal delay.
	pub delay: BlockNumber,
	/// New validators set.
	pub validators: Vec<u8>,
}

/// Convert from U256 to BlockNumber.
pub fn to_substrate_block_number(number: U256) -> Result<BlockNumber, Error> {
	match number == number.low_u32().into() {
		true => Ok(number.low_u32()),
		false => Err(Error::BlockNumberDecode),
	}
}

/// Convert from BlockNumber to U256.
pub fn from_substrate_block_number(number: BlockNumber) -> Result<U256, Error> {
	Ok(U256::from(number as u64))
}

/// Parse Substrate header.
pub fn parse_substrate_header(raw_header: &[u8]) -> Result<Header, Error> {
	RuntimeHeader::decode(&mut &raw_header[..])
		.map(|header| Header {
			hash: header.hash(),
			parent_hash: header.parent_hash,
			number: header.number,
			signal: sp_runtime::traits::Header::digest(&header)
				.log(|log| {
					log.as_consensus().and_then(|(engine_id, log)| {
						if engine_id == GRANDPA_ENGINE_ID {
							Some(log)
						} else {
							None
						}
					})
				})
				.and_then(|log| ConsensusLog::decode(&mut &log[..]).ok())
				.and_then(|log| match log {
					ConsensusLog::ScheduledChange(scheduled_change) => Some(ValidatorsSetSignal {
						delay: scheduled_change.delay,
						validators: scheduled_change.next_authorities.encode(),
					}),
					_ => None,
				}),
		})
		.map_err(Error::HeaderDecode)
}

/// Verify GRANDPA finality proof.
pub fn verify_substrate_finality_proof(
	finality_target_number: BlockNumber,
	finality_target_hash: Hash,
	best_set_id: u64,
	raw_best_set: &[u8],
	raw_finality_proof: &[u8],
) -> Result<(), Error> {
	let best_set = AuthorityList::decode(&mut &raw_best_set[..]).map_err(Error::BestSetDecode)?;
	sc_finality_grandpa::GrandpaJustification::<Block>::decode_and_verify_finalizes(
		&raw_finality_proof,
		(finality_target_hash, finality_target_number),
		best_set_id,
		&best_set.into_iter().collect(),
	)
	.map_err(Error::JustificationVerify)
	.map(|_| ())
}
