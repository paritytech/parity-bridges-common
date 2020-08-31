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
use bp_substrate::{prove_finality, AuthoritySet, ImportedHeader, ScheduledChange};
use sp_finality_grandpa::{ConsensusLog, SetId, GRANDPA_ENGINE_ID};
use sp_runtime::generic::OpaqueDigestItemId;
use sp_runtime::traits::Header as HeaderT;
use sp_std::{prelude::Vec, vec};

pub type FinalityProof = Vec<u8>;

#[derive(Debug, PartialEq)]
pub enum ImportError {
	OldHeader,
	HeaderAlreadyExists,
	MissingParent,
	UnfinalizedHeader,
	AncestryCheckFailed,
	MissingConsensusDigest,
}

/// A trait for verifying whether a header is valid for a particular blockchain.
pub trait ChainVerifier<S, H> {
	/// Import a header to the pallet.
	fn import_header(storage: &mut S, header: &H, finality_proof: Option<FinalityProof>) -> Result<(), ImportError>;

	/// Verify that the given header has been finalized and is part of the canonical chain.
	///
	/// Returns a list of headers which got finalized by the given header.
	fn verify_finality(storage: &mut S, header: &H, proof: &FinalityProof) -> Result<Vec<H>, ImportError>;
}

#[derive(Debug)]
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
		//
		// This defaults to 0, should it maybe be an Option?
		let scheduled_change_height = storage.scheduled_set_change().height;
		if *header.number() == scheduled_change_height && finality_proof.is_some() {
			// Maybe pass the scheduled_change in here so we don't have to query storage later
			let finalized_headers = Self::verify_finality(
				storage,
				header,
				&finality_proof.expect("Checked for `finality_proof` before entering if-block"),
			)?;

			// TODO: Would need to prune blocks from the non-canonical chain at some point
			for header in finalized_headers.iter() {
				storage.write_header(&ImportedHeader::new(header.clone(), true));
			}

			return Ok(());
		}

		// Don't like having to take ownership of header...
		storage.write_header(&ImportedHeader::new(header.clone(), false));

		Ok(())
	}

	fn verify_finality(storage: &mut S, header: &H, proof: &FinalityProof) -> Result<Vec<H>, ImportError> {
		let current_authority_set = storage.current_authority_set();
		let justification = &proof;

		let is_finalized = prove_finality(&header, &current_authority_set, &justification);
		if !is_finalized {
			return Err(ImportError::UnfinalizedHeader);
		}

		let last_finalized = storage.best_finalized_header().expect("TODO");
		if let Some(ancestors) = are_ancestors(storage, last_finalized, header.clone()) {
			let current_set_id = current_authority_set.set_id;
			update_authority_set(storage, header, current_set_id)?;
			return Ok(ancestors);
		} else {
			return Err(ImportError::AncestryCheckFailed);
		}
	}
}

// Returns the lineage of headers from (ancestor, child]
fn are_ancestors<S, H>(storage: &S, ancestor: H, child: H) -> Option<Vec<H>>
where
	S: BridgeStorage<Header = H>,
	H: HeaderT,
{
	let mut ancestors = vec![];
	let mut current_header = child;

	while ancestor.hash() != current_header.hash() {
		// We've gotten to the same height and we're not related
		if ancestor.number() == current_header.number() {
			return None;
		}

		let parent = storage.get_header_by_hash(*current_header.parent_hash());
		ancestors.push(current_header);
		current_header = match parent {
			Some(h) => h.header,
			None => return None,
		}
	}

	return Some(ancestors);
}

fn update_authority_set<S, H>(storage: &mut S, header: &H, current_set_id: SetId) -> Result<(), ImportError>
where
	S: BridgeStorage<Header = H>,
	H: HeaderT,
{
	if let Some(scheduled_change) = find_scheduled_change(header) {
		// Adding two since we need to account for scheduled set which is about to be triggered
		let set_id = current_set_id + 2;
		let authority_set = AuthoritySet {
			authorities: scheduled_change.next_authorities,
			set_id,
		};

		// Maybe do some overflow checks here?
		let height = *header.number() + scheduled_change.delay;
		let scheduled_change = ScheduledChange::new(authority_set, height);

		let new_set = storage.scheduled_set_change().authority_set;
		storage.update_current_authority_set(new_set);
		storage.schedule_next_set_change(scheduled_change);

		Ok(())
	} else {
		Err(ImportError::MissingConsensusDigest)
	}
}

fn find_scheduled_change<H: HeaderT>(header: &H) -> Option<sp_finality_grandpa::ScheduledChange<H::Number>> {
	let id = OpaqueDigestItemId::Consensus(&GRANDPA_ENGINE_ID);

	let filter_log = |log: ConsensusLog<H::Number>| match log {
		ConsensusLog::ScheduledChange(change) => Some(change),
		_ => None,
	};

	// find the first consensus digest with the right ID which converts to
	// the right kind of consensus log.
	header.digest().convert_first(|l| l.try_to(id).and_then(filter_log))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::*;
	use crate::{BestFinalized, ImportedHeaders, PalletStorage};
	use frame_support::{assert_err, assert_ok};
	use frame_support::{StorageMap, StorageValue};
	use parity_scale_codec::Encode;
	use sp_finality_grandpa::{AuthorityId, AuthorityList};
	use sp_runtime::testing::UintAuthorityId;
	use sp_runtime::{Digest, DigestItem};

	type TestHeader = <TestRuntime as frame_system::Trait>::Header;
	type TestHash = <TestHeader as HeaderT>::Hash;
	type TestNumber = <TestHeader as HeaderT>::Number;

	fn get_authorities(authorities: Vec<(u64, u64)>) -> AuthorityList {
		authorities
			.iter()
			.map(|(id, weight)| (UintAuthorityId(*id).to_public_key::<AuthorityId>(), *weight))
			.collect()
	}

	fn schedule_next_change(
		authorities: Vec<(u64, u64)>,
		set_id: u64,
		height: TestNumber,
	) -> ScheduledChange<TestNumber> {
		let authorities = get_authorities(authorities);
		let authority_set = AuthoritySet::new(authorities, set_id);
		ScheduledChange::new(authority_set.clone(), height)
	}

	fn write_headers<S: BridgeStorage<Header = TestHeader>>(
		storage: &mut S,
		headers: Vec<(u64, bool)>,
	) -> Vec<TestHeader> {
		let mut imported_headers = vec![];
		let genesis = TestHeader::new_from_number(0);
		<BestFinalized<TestRuntime>>::put(&genesis);
		storage.write_header(&ImportedHeader::new(genesis.clone(), true));
		imported_headers.push(genesis);

		for (num, finalized) in headers {
			let mut h = TestHeader::new_from_number(num);
			h.parent_hash = imported_headers.last().unwrap().hash();
			storage.write_header(&ImportedHeader::new(h.clone(), finalized));
			imported_headers.push(h);
		}

		imported_headers
	}

	#[test]
	fn fails_to_import_old_header() {
		run_test(|| {
			let mut storage = PalletStorage::<TestRuntime>::new();
			let parent = TestHeader::new_from_number(5);
			<BestFinalized<TestRuntime>>::put(&parent);

			let header = TestHeader::new_from_number(1);
			assert_err!(
				Verifier::import_header(&mut storage, &header, None),
				ImportError::OldHeader
			);
		})
	}

	#[test]
	fn fails_to_import_header_without_parent() {
		run_test(|| {
			let mut storage = PalletStorage::<TestRuntime>::new();
			let parent = TestHeader::new_from_number(1);
			<BestFinalized<TestRuntime>>::put(&parent);

			// By default the parent is `0x00`
			let header = TestHeader::new_from_number(2);

			assert_err!(
				Verifier::import_header(&mut storage, &header, None),
				ImportError::MissingParent
			);
		})
	}

	#[test]
	fn fails_to_import_header_twice() {
		run_test(|| {
			let mut storage = PalletStorage::<TestRuntime>::new();
			let header = TestHeader::new_from_number(1);
			<BestFinalized<TestRuntime>>::put(&header);

			let imported_header = ImportedHeader {
				header: header.clone(),
				is_finalized: true,
			};

			<ImportedHeaders<TestRuntime>>::insert(header.hash(), &imported_header);

			assert_err!(
				Verifier::import_header(&mut storage, &header, None),
				ImportError::HeaderAlreadyExists
			);
		})
	}

	#[test]
	fn succesfully_imports_valid_but_unfinalized_header() {
		run_test(|| {
			let mut storage = PalletStorage::<TestRuntime>::new();
			let parent = TestHeader::new_from_number(1);
			let parent_hash = parent.hash();
			<BestFinalized<TestRuntime>>::put(&parent);

			let imported_header = ImportedHeader {
				header: parent.clone(),
				is_finalized: true,
			};

			<ImportedHeaders<TestRuntime>>::insert(parent_hash, &imported_header);

			let mut header = TestHeader::new_from_number(2);
			header.parent_hash = parent_hash;
			assert_ok!(Verifier::import_header(&mut storage, &header, None));

			let stored_header = storage.get_header_by_hash(header.hash());
			assert!(stored_header.is_some());
			assert_eq!(stored_header.unwrap().is_finalized, false);
		})
	}

	#[test]
	fn related_headers_are_ancestors() {
		run_test(|| {
			let mut storage = PalletStorage::<TestRuntime>::new();
			let mut headers = vec![];
			let num_headers = 4;

			let mut header = TestHeader::new_from_number(0);
			headers.push(header.clone());
			storage.import_unfinalized_header(header);

			for i in 1..num_headers {
				header = TestHeader::new_from_number(i as u64);
				header.parent_hash = headers[i - 1].hash();
				headers.push(header);
				storage.import_unfinalized_header(headers[i].clone());
			}

			for i in 0..num_headers {
				assert!(storage.header_exists(headers[i].hash()));
			}

			let ancestor = headers.remove(0);
			let child = headers.pop().unwrap();
			let ancestors = are_ancestors(&storage, ancestor, child);
			assert!(ancestors.is_some());
			assert_eq!(ancestors.unwrap().len(), num_headers - 1);
		})
	}

	#[test]
	fn unrelated_headers_are_not_ancestors() {
		run_test(|| {
			let mut storage = PalletStorage::<TestRuntime>::new();
			let mut headers = vec![];

			let mut header = TestHeader::new_from_number(0);
			headers.push(header.clone());
			storage.import_unfinalized_header(header);

			for i in 1..4 {
				header = TestHeader::new_from_number(i as u64);
				header.parent_hash = headers[i - 1].hash();
				headers.push(header);
				storage.import_unfinalized_header(headers[i].clone());
			}

			for i in 0..4 {
				assert!(storage.header_exists(headers[i].hash()));
			}

			let mut bad_ancestor = TestHeader::new_from_number(0);
			bad_ancestor.parent_hash = [1u8; 32].into();
			let child = headers.pop().unwrap();
			let ancestors = are_ancestors(&storage, bad_ancestor, child);
			assert!(ancestors.is_none());
		})
	}

	#[test]
	fn authority_set_is_updated_in_storage_correctly() {
		run_test(|| {
			let mut storage = PalletStorage::<TestRuntime>::new();
			let mut header = TestHeader::new_from_number(1);

			// Populate storage with a scheduled change
			let set_id = 2;
			let height = 1;
			let authorities = vec![(1, 1)];
			let change = schedule_next_change(authorities.clone(), set_id, height);
			storage.schedule_next_set_change(change);

			// Prepare next scheduled change
			let next_set_id = 3;
			let next_height = 3;
			let next_authorities = vec![(2, 1)];
			let scheduled_change = schedule_next_change(next_authorities.clone(), next_set_id, next_height);

			// Prepare header to schedule a change
			let consensus_log = ConsensusLog::<TestNumber>::ScheduledChange(sp_finality_grandpa::ScheduledChange {
				next_authorities: get_authorities(next_authorities),
				delay: 2,
			});
			header.digest = Digest::<TestHash> {
				logs: vec![DigestItem::Consensus(GRANDPA_ENGINE_ID, consensus_log.encode())],
			};

			let first_set_id = 1;
			assert_ok!(update_authority_set(&mut storage, &header, first_set_id));

			// Make sure that current authority set is the first change we scheduled
			assert_eq!(
				storage.current_authority_set(),
				AuthoritySet::new(get_authorities(authorities), set_id)
			);

			// Make sure that the next scheduled change is the one we just inserted
			assert_eq!(storage.scheduled_set_change(), scheduled_change);
		})
	}

	#[test]
	fn correctly_verifies_and_finalizes_chain_of_headers() {
		run_test(|| {
			let mut storage = PalletStorage::<TestRuntime>::new();
			let headers = vec![(1, false), (2, false)];
			let imported_headers = write_headers(&mut storage, headers);

			let mut header = TestHeader::new_from_number(3);
			header.parent_hash = imported_headers[2].hash();

			let set_id = 1;
			let height = *header.number();
			let authorities = vec![(1, 1)];
			let change = schedule_next_change(authorities, set_id, height);
			storage.schedule_next_set_change(change);

			let consensus_log = ConsensusLog::<TestNumber>::ScheduledChange(sp_finality_grandpa::ScheduledChange {
				next_authorities: get_authorities(vec![(1, 1)]),
				delay: 0,
			});

			header.digest = Digest::<TestHash> {
				logs: vec![DigestItem::Consensus(GRANDPA_ENGINE_ID, consensus_log.encode())],
			};

			assert!(Verifier::import_header(&mut storage, &header, Some(vec![4, 2])).is_ok());

			// Make sure we marked the our headers as finalized
			assert!(
				storage
					.get_header_by_hash(imported_headers[1].hash())
					.unwrap()
					.is_finalized
			);
			assert!(
				storage
					.get_header_by_hash(imported_headers[2].hash())
					.unwrap()
					.is_finalized
			);
			assert!(storage.get_header_by_hash(header.hash()).unwrap().is_finalized);
		});
	}
}
