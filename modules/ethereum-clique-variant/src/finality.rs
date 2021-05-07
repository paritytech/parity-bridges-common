// Copyright 2019-2021 Parity Technologies (UK) Ltd.
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

use crate::error::Error;
use crate::Storage;
use bp_eth_clique::{public_to_address, Address, CliqueHeader, HeaderId, SealedEmptyStep, H256};
use codec::{Decode, Encode};
use sp_io::crypto::secp256k1_ecdsa_recover;
use sp_runtime::RuntimeDebug;
use sp_std::collections::{
	btree_map::{BTreeMap, Entry},
	btree_set::BTreeSet,
	vec_deque::VecDeque,
};
use sp_std::prelude::*;

/// Finality effects.
#[derive(RuntimeDebug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct FinalityEffects<Submitter> {
	/// Finalized headers.
	pub finalized_headers: Vec<(HeaderId, Option<Submitter>)>,
}

/// Information about block ancestor that is used in computations.
#[derive(RuntimeDebug, Decode, Encode)]
#[cfg_attr(test, derive(Clone, Default, PartialEq))]
pub struct FinalityAncestor<Submitter> {
	/// Block id.
	pub id: HeaderId,
	/// Block submitter.
	pub submitter: Option<Submitter>,
	/// Validators that have signed this block and empty steps on top
	/// of this block.
	pub signers: BTreeSet<Address>,
}

/// Tries to finalize blocks when given block is imported.
///
/// Returns numbers and hashes of finalized blocks in ascending order.
pub fn finalize_blocks<S: Storage>(
	storage: &S,
	best_finalized: HeaderId,
	header_validators: (HeaderId, &[Address]),
	id: HeaderId,
	submitter: Option<&S::Submitter>,
	header: &CliqueHeader,
	two_thirds_majority_transition: u64,
) -> Result<FinalityEffects<S::Submitter>, Error> {
	// compute count of voters for every unfinalized block in ancestry
	let validators = header_validators.1.iter().collect();

	// now let's iterate in reverse order && find just finalized blocks
	let mut finalized_headers = Vec::new();
	for ancestor in &votes.ancestry {
		if !is_finalized(
			&validators,
			&current_votes,
			ancestor.id.number >= two_thirds_majority_transition,
		) {
			break;
		}

		remove_signers_votes(&ancestor.signers, &mut current_votes);
		finalized_headers.push((ancestor.id, ancestor.submitter.clone()));
	}

	Ok(FinalityEffects {
		finalized_headers,
		votes,
	})
}

/// Returns true if there are enough votes to treat this header as finalized.
fn is_finalized(
	validators: &BTreeSet<&Address>,
	votes: &BTreeMap<Address, u64>,
	requires_two_thirds_majority: bool,
) -> bool {
	(!requires_two_thirds_majority && votes.len() * 2 > validators.len())
		|| (requires_two_thirds_majority && votes.len() * 3 > validators.len() * 2)
}
