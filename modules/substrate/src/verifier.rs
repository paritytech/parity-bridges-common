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
use parity_scale_codec::Decode;
use sp_finality_grandpa::{ConsensusLog, GRANDPA_ENGINE_ID};
use sp_runtime::traits::Header as HeaderT;
use sp_runtime::DigestItem;
use sp_std::prelude::Vec;

pub type FinalityProof = Vec<u8>;

#[derive(Debug, PartialEq)]
pub enum ImportError {
	OldHeader,
	HeaderAlreadyExists,
	MissingParent,
	UnfinalizedHeader,
	AncestryCheckFailed,
}

/// A trait for verifying whether a header is valid for a particular blockchain.
pub trait ChainVerifier<S, H> {
	/// Import a header to the pallet.
	fn import_header(storage: &mut S, header: &H, finality_proof: Option<FinalityProof>) -> Result<(), ImportError>;

	/// Verify that the given header has been finalized and is part of the canonical chain.
	fn verify_finality(storage: &mut S, header: &H, proof: &FinalityProof) -> Result<(), ImportError>;
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

		let mut is_finalized = false;
		// A block at this height should come with a justification and signal a new
		// authority set. We'll want to make sure it is valid
		//
		// This defaults to 0, should it maybe be an Option?
		let scheduled_change_height = storage.scheduled_set_change().height;
		if *header.number() == scheduled_change_height {
			// Maybe pass the scheduled_change in here so we don't have to query storage later
			Self::verify_finality(storage, header, &finality_proof.expect("TODO"))?;
			is_finalized = true;
		}

		let h = ImportedHeader {
			header: header.clone(), // I don't like having to do this...
			is_finalized,
		};

		storage.write_header(&h);

		Ok(())
	}

	fn verify_finality(storage: &mut S, header: &H, proof: &FinalityProof) -> Result<(), ImportError> {
		let digest = header.digest().logs().last().expect("TODO");
		if let DigestItem::Consensus(id, item) = digest {
			if *id == GRANDPA_ENGINE_ID {
				let current_authority_set = storage.current_authority_set();
				let current_set_id = current_authority_set.set_id;
				let justification = &proof;

				let is_finalized = prove_finality(&header, &current_authority_set, &justification);
				if !is_finalized {
					return Err(ImportError::UnfinalizedHeader);
				}

				let last_finalized = storage.best_finalized_header().expect("TODO");
				let are_ancestors = are_ancestors(storage, last_finalized, header.clone());
				if !are_ancestors {
					return Err(ImportError::AncestryCheckFailed);
				}

				// We need to update the `next_validator_set` storage item if it's appropriate
				let log: ConsensusLog<H::Number> = ConsensusLog::decode(&mut &item[..]).expect("TODO");
				let scheduled_change = match log {
					ConsensusLog::ScheduledChange(scheduled_change) => {
						let authority_set = AuthoritySet {
							authorities: scheduled_change.next_authorities,
							set_id: current_set_id + 1,
						};

						// Maybe do some overflow checks here?
						let height = *header.number() + scheduled_change.delay;

						ScheduledChange { authority_set, height }
					}
					ConsensusLog::ForcedChange(_n, _forced_change) => todo!(),
					_ => todo!("idk what to do here"),
				};

				let new_set = storage.scheduled_set_change().authority_set;
				storage.update_current_authority_set(new_set);
				storage.schedule_next_set_change(scheduled_change);
			}
		} else {
			// This block doesn't have a justification
			todo!()
		}

		Ok(())
	}
}

fn are_ancestors<S, H>(storage: &S, ancestor: H, child: H) -> bool
where
	S: BridgeStorage<Header = H>,
	H: HeaderT,
{
	let mut current_header = child;

	while ancestor.hash() != current_header.hash() {
		// We've gotten to the same height and we're not related
		if ancestor.number() == current_header.number() {
			return false;
		}

		let parent = storage.get_header_by_hash(*current_header.parent_hash());
		current_header = match parent {
			Some(h) => h.header,
			None => return false,
		}
	}

	return true;
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::*;
	use crate::{BestFinalized, ImportedHeaders, PalletStorage};
	use frame_support::{assert_err, assert_ok};
	use frame_support::{StorageMap, StorageValue};

	type TestHeader = <TestRuntime as frame_system::Trait>::Header;

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

			let ancestor = headers.remove(0);
			let child = headers.pop().unwrap();
			assert!(are_ancestors(&storage, ancestor, child));
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
			assert_eq!(are_ancestors(&storage, bad_ancestor, child), false);
		})
	}
}
