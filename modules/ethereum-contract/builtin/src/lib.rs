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

use codec::{Encode, Decode};
use bridge_node_runtime::{BlockNumber, Hash, Header as RuntimeHeader};
use sp_blockchain::Error as ClientError;
use sp_finality_grandpa::AuthorityList;

/// Builtin errors.
#[derive(Debug)]
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

/// All types of finality proofs.
#[derive(Decode, Encode)]
pub enum FinalityProof {
	/// GRANDPA justification.
	Justification(Vec<u8>),
}

/// Parse Substrate header.
pub fn parse_substrate_header(raw_header: &[u8]) -> Result<Header, Error> {
	RuntimeHeader::decode(&mut &raw_header[..])
		.map(|header| Header {
			hash: header.hash(),
			parent_hash: header.parent_hash,
			number: header.number,
			signal: None, // TODO
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

	let _best_header = RuntimeHeader::decode(&mut &raw_best_header[..])
		.map_err(Error::HeaderDecode)?;
	let headers = raw_headers
		.iter()
		.map(|raw_header| RuntimeHeader::decode(&mut &raw_header[..])
			.map_err(Error::HeaderDecode)
		)
		.collect::<Result<Vec<_>, _>>()?;

	let finality_target = match headers.last() {
		Some(header) => (header.hash(), header.number),
		None => return Err(Error::NoNewHeaders),
	};

	match finality_proof {
		FinalityProof::Justification(raw_justification) => {
			substrate::GrandpaJustification::decode_and_verify_finalizes(
				&raw_justification,
				finality_target,
				best_set_id,
				&best_voters.into_iter().collect(),
			).map_err(Error::JustificationVerify)?;
		},
	}

	Ok((0, headers.len()))
}

// ===================================================================================================
// ===================================================================================================
// ===================================================================================================


mod substrate {
	use std::collections::HashMap;
	use codec::{Encode, Decode};
	use bridge_node_runtime::{BlockNumber, Hash, Header as RuntimeHeader};
	use finality_grandpa::{voter_set::VoterSet};
	use sp_blockchain::Error as ClientError;
	use sp_finality_grandpa::{AuthorityId, AuthoritySignature};

	/// A commit message for this chain's block type.
	pub type Commit = finality_grandpa::Commit<Hash, BlockNumber, AuthoritySignature, AuthorityId>;

	/// GRANDPA justification.
	#[derive(Encode, Decode)]
	pub struct GrandpaJustification {
		pub round: u64,
		pub commit: Commit,
		pub votes_ancestries: Vec<RuntimeHeader>,
	}

	impl GrandpaJustification {
		/// Decode a GRANDPA justification and validate the commit and the votes'
		/// ancestry proofs finalize the given block.
		pub(crate) fn decode_and_verify_finalizes(
			encoded: &[u8],
			finalized_target: (Hash, BlockNumber),
			set_id: u64,
			voters: &VoterSet<AuthorityId>,
		) -> Result<GrandpaJustification, ClientError> {

			let justification = GrandpaJustification::decode(&mut &*encoded)
				.map_err(|_| ClientError::JustificationDecode)?;

			if (justification.commit.target_hash, justification.commit.target_number) != finalized_target {
				let msg = "invalid commit target in grandpa justification".to_string();
				Err(ClientError::BadJustification(msg))
			} else {
				justification.verify(set_id, voters).map(|_| justification)
			}
		}

		/// Validate the commit and the votes' ancestry proofs.
		pub(crate) fn verify(&self, _set_id: u64, voters: &VoterSet<AuthorityId>) -> Result<(), ClientError> {
//			use finality_grandpa::Chain;

			let ancestry_chain = AncestryChain::new(&self.votes_ancestries);

			match finality_grandpa::validate_commit(
				&self.commit,
				voters,
				&ancestry_chain,
			) {
				Ok(ref result) if result.ghost().is_some() => {},
				_ => {
					let msg = "invalid commit in grandpa justification".to_string();
					return Err(ClientError::BadJustification(msg));
				}
			}
/* TODO
			let mut buf = Vec::new();
			let mut visited_hashes = HashSet::new();
			for signed in self.commit.precommits.iter() {
				if let Err(_) = communication::check_message_sig_with_buffer::<Block>(
					&finality_grandpa::Message::Precommit(signed.precommit.clone()),
					&signed.id,
					&signed.signature,
					self.round,
					set_id,
					&mut buf,
				) {
					return Err(ClientError::BadJustification(
						"invalid signature for precommit in grandpa justification".to_string()).into());
				}

				if self.commit.target_hash == signed.precommit.target_hash {
					continue;
				}

				match ancestry_chain.ancestry(self.commit.target_hash, signed.precommit.target_hash) {
					Ok(route) => {
						// ancestry starts from parent hash but the precommit target hash has been visited
						visited_hashes.insert(signed.precommit.target_hash);
						for hash in route {
							visited_hashes.insert(hash);
						}
					},
					_ => {
						return Err(ClientError::BadJustification(
							"invalid precommit ancestry proof in grandpa justification".to_string()).into());
					},
				}
			}

			let ancestry_hashes = self.votes_ancestries
				.iter()
				.map(|h: &Block::Header| h.hash())
				.collect();

			if visited_hashes != ancestry_hashes {
				return Err(ClientError::BadJustification(
					"invalid precommit ancestries in grandpa justification with unused headers".to_string()).into());
			}
*/
			Ok(())
		}
	}

	/// A utility trait implementing `finality_grandpa::Chain` using a given set of headers.
	/// This is useful when validating commits, using the given set of headers to
	/// verify a valid ancestry route to the target commit block.
	struct AncestryChain {
		ancestry: HashMap<Hash, RuntimeHeader>,
	}

	impl AncestryChain {
		fn new(ancestry: &[RuntimeHeader]) -> AncestryChain {
			let ancestry: HashMap<_, _> = ancestry
				.iter()
				.cloned()
				.map(|h: RuntimeHeader| (h.hash(), h))
				.collect();

			AncestryChain { ancestry }
		}
	}

	impl finality_grandpa::Chain<Hash, BlockNumber> for AncestryChain {
		fn ancestry(&self, base: Hash, block: Hash) -> Result<Vec<Hash>, finality_grandpa::Error> {
			let mut route = Vec::new();
			let mut current_hash = block;
			loop {
				if current_hash == base { break; }
				match self.ancestry.get(&current_hash) {
					Some(current_header) => {
						current_hash = current_header.parent_hash;
						route.push(current_hash);
					},
					_ => return Err(finality_grandpa::Error::NotDescendent),
				}
			}
			route.pop(); // remove the base

			Ok(route)
		}

		fn best_chain_containing(&self, _block: Hash) -> Option<(Hash, BlockNumber)> {
			None
		}
	}
}
