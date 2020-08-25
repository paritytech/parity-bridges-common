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

use crate::BridgeStorage;
use parity_scale_codec::{Decode, Encode};
use sp_finality_grandpa::{AuthorityList, ConsensusLog, SetId, GRANDPA_ENGINE_ID};
use sp_runtime::traits::Header as HeaderT;
use sp_runtime::DigestItem;

pub type FinalityProof = (Vec<u8>, AuthorityList, SetId);

pub enum ImportError {
	OldHeader,
	HeaderAlreadyExists,
	MissingParent,
}

/// A trait for verifying whether a header is valid for a particular blockchain.
pub trait ChainVerifier<S, H> {
	/// Import a header to the pallet.
	fn import_header(storage: &mut S, header: &H, finality_proof: Option<FinalityProof>) -> Result<(), ImportError>;

	/// Verify that the given header has been finalized and is part of the canonical chain.
	fn verify_finality(storage: &mut S, header: &H, proof: &FinalityProof) -> Result<(), ImportError>;
}

pub struct Verifier;

impl<S, H> ChainVerifier<S, H> for Verifier
where
	S: BridgeStorage<Header = H>,
	H: HeaderT,
{
	fn import_header(storage: &mut S, header: &H, finality_proof: Option<FinalityProof>) -> Result<(), ImportError> {
		let highest_finalized = storage.best_finalized_header().expect("TODO");
		if header.number() < highest_finalized.number() {
			return Err(ImportError::OldHeader);
		}

		if storage.header_exists(header.hash()) {
			return Err(ImportError::HeaderAlreadyExists);
		}

		let parent_header = storage.get_header_by_hash(*header.parent_hash());
		if parent_header.is_none() {
			return Err(ImportError::MissingParent);
		}

		// A block at this height should come with a justification and signal a new
		// authority set. We'll want to make sure it is valid
		let scheduled_change_height = storage.scheduled_set_change().height;
		if *header.number() == scheduled_change_height {
			// Maybe pass the scheduled_change in here so we don't have to query storage later
			Self::verify_finality(storage, header, &finality_proof.expect("TOOO"))?;
		}

		Ok(())
	}

	fn verify_finality(storage: &mut S, header: &H, proof: &FinalityProof) -> Result<(), ImportError> {
		let digest = header.digest().logs().last().expect("TODO");
		if let DigestItem::Consensus(id, item) = digest {
			if *id == GRANDPA_ENGINE_ID {
				let current_authority_set = storage.current_authority_set();
				let current_set_id = current_authority_set.set_id;
				let justification = &proof.0;
				// prove_finality(header, current_authority_set, current_set_id, justification)?

				// We'll need to mark ancestors as finalized

				// Since we've checked and the header is finalized we can start updating the
				// authority set info

				// We need to update the `next_validator_set` storage item if it's appropriate
				let log: ConsensusLog<u32> = ConsensusLog::decode(&mut &item[..]).expect("TODO");
				let authority_change = match log {
					ConsensusLog::ScheduledChange(scheduled_change) => todo!(),
					ConsensusLog::ForcedChange(n, forced_change) => todo!(),
					_ => todo!("idk what to do here"),
				};

				// storage.update_current_authority_set(storage.scheduled_set_change());
				// storage.schedule_next_change(authority_change);
			}
		} else {
			// This block doesn't have a justification
			todo!()
		}

		Ok(())
	}
}
