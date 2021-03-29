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

//! Module for checking GRANDPA Finality Proofs.
//!
//! Adapted copy of substrate/client/finality-grandpa/src/justification.rs. If origin
//! will ever be moved to the sp_finality_grandpa, we should reuse that implementation.

use codec::{Decode, Encode};
use finality_grandpa::{voter_set::VoterSet, Chain, Error as GrandpaError};
use frame_support::RuntimeDebug;
use sp_finality_grandpa::{AuthorityId, AuthoritySignature, SetId};
use sp_runtime::traits::Header as HeaderT;
use sp_std::collections::{btree_map::BTreeMap, btree_set::BTreeSet};
use sp_std::prelude::Vec;

/// Justification verification error.
#[derive(RuntimeDebug, PartialEq)]
pub enum Error {
	/// Failed to decode justification.
	JustificationDecode,
	/// Justification is finalizing unexpected header.
	InvalidJustificationTarget,
	/// Invalid commit in justification.
	InvalidJustificationCommit,
	/// Justification has invalid authority singature.
	InvalidAuthoritySignature,
	/// The justification has precommit for the header that has no route from the target header.
	InvalidPrecommitAncestryProof,
	/// The justification has 'unused' headers in its precommit ancestries.
	InvalidPrecommitAncestries,
}

/// Decode justification target.
pub fn decode_justification_target<Header: HeaderT>(
	raw_justification: &[u8],
) -> Result<(Header::Hash, Header::Number), Error> {
	GrandpaJustification::<Header>::decode(&mut &*raw_justification)
		.map(|justification| (justification.commit.target_hash, justification.commit.target_number))
		.map_err(|_| Error::JustificationDecode)
}

/// Verify that justification, that is generated by given authority set, finalizes given header.
pub fn verify_justification<Header: HeaderT>(
	finalized_target: (Header::Hash, Header::Number),
	authorities_set_id: SetId,
	authorities_set: &VoterSet<AuthorityId>,
	raw_justification: &[u8],
) -> Result<GrandpaJustification<Header>, Error>
where
	Header::Number: finality_grandpa::BlockNumberOps,
{
	// Decode justification first
	let justification =
		GrandpaJustification::<Header>::decode(&mut &*raw_justification).map_err(|_| Error::JustificationDecode)?;

	// Ensure that it is justification for the expected header
	if (justification.commit.target_hash, justification.commit.target_number) != finalized_target {
		return Err(Error::InvalidJustificationTarget);
	}

	// Validate commit of the justification. Note that `validate_commit()` assumes that all
	// signatures are valid. We'll check the validity of the signatures later since they're more
	// resource intensive to verify.
	let ancestry_chain = AncestryChain::new(&justification.votes_ancestries);
	match finality_grandpa::validate_commit(&justification.commit, authorities_set, &ancestry_chain) {
		Ok(ref result) if result.ghost().is_some() => {}
		_ => return Err(Error::InvalidJustificationCommit),
	}

	// Now that we know that the commit is correct, check authorities signatures
	let mut buf = Vec::new();
	let mut visited_hashes = BTreeSet::new();
	for signed in &justification.commit.precommits {
		if !sp_finality_grandpa::check_message_signature_with_buffer(
			&finality_grandpa::Message::Precommit(signed.precommit.clone()),
			&signed.id,
			&signed.signature,
			justification.round,
			authorities_set_id,
			&mut buf,
		) {
			return Err(Error::InvalidAuthoritySignature);
		}

		if justification.commit.target_hash == signed.precommit.target_hash {
			continue;
		}

		match ancestry_chain.ancestry(justification.commit.target_hash, signed.precommit.target_hash) {
			Ok(route) => {
				// ancestry starts from parent hash but the precommit target hash has been visited
				visited_hashes.insert(signed.precommit.target_hash);
				visited_hashes.extend(route);
			}
			_ => {
				// could this happen in practice? I don't think so, but original code has this check
				return Err(Error::InvalidPrecommitAncestryProof);
			}
		}
	}

	let ancestry_hashes = justification
		.votes_ancestries
		.iter()
		.map(|h: &Header| h.hash())
		.collect();
	if visited_hashes != ancestry_hashes {
		return Err(Error::InvalidPrecommitAncestries);
	}

	// Note: The original GRANDPA code doesn't return the decoded justification. However, from the
	// runtime's perspective it is useful for getting infromation about how much work was actual
	// spent validating the justification.
	Ok(justification)
}

/// A GRANDPA Justification is a proof that a given header was finalized
/// at a certain height and with a certain set of authorities.
///
/// This particular proof is used to prove that headers on a bridged chain
/// (so not our chain) have been finalized correctly.
#[derive(Encode, Decode, RuntimeDebug)]
pub struct GrandpaJustification<Header: HeaderT> {
	/// The round (voting period) this justification is valid for.
	pub round: u64,
	/// The set of votes for the chain which is to be finalized.
	pub commit: finality_grandpa::Commit<Header::Hash, Header::Number, AuthoritySignature, AuthorityId>,
	/// A proof that the chain of blocks in the commit are related to each other.
	pub votes_ancestries: Vec<Header>,
}

/// A utility trait implementing `finality_grandpa::Chain` using a given set of headers.
#[derive(RuntimeDebug)]
struct AncestryChain<Header: HeaderT> {
	ancestry: BTreeMap<Header::Hash, Header::Hash>,
}

impl<Header: HeaderT> AncestryChain<Header> {
	fn new(ancestry: &[Header]) -> AncestryChain<Header> {
		AncestryChain {
			ancestry: ancestry
				.iter()
				.map(|header| (header.hash(), *header.parent_hash()))
				.collect(),
		}
	}
}

impl<Header: HeaderT> finality_grandpa::Chain<Header::Hash, Header::Number> for AncestryChain<Header>
where
	Header::Number: finality_grandpa::BlockNumberOps,
{
	fn ancestry(&self, base: Header::Hash, block: Header::Hash) -> Result<Vec<Header::Hash>, GrandpaError> {
		let mut route = Vec::new();
		let mut current_hash = block;
		loop {
			if current_hash == base {
				break;
			}
			match self.ancestry.get(&current_hash).cloned() {
				Some(parent_hash) => {
					current_hash = parent_hash;
					route.push(current_hash);
				}
				_ => return Err(GrandpaError::NotDescendent),
			}
		}
		route.pop(); // remove the base

		Ok(route)
	}
}
