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

//! The verifier's role is to check the validity of headers being imported, and also determine if
//! they can be finalized.
//!
//! When importing headers it performs checks to ensure that no invariants are broken (like
//! importing the same header twice). When it imports finality proofs it will ensure that the proof
//! has been signed off by the correct GRANDPA authorities, and also enact any authority set changes
//! if required.

use crate::storage::{ImportedHeader, ScheduledChange};
use crate::BridgeStorage;

use bp_header_chain::{justification::verify_justification, AuthoritySet};
use finality_grandpa::voter_set::VoterSet;
use sp_finality_grandpa::{ConsensusLog, GRANDPA_ENGINE_ID};
use sp_runtime::generic::OpaqueDigestItemId;
use sp_runtime::traits::{CheckedAdd, Header as HeaderT, One};
use sp_runtime::RuntimeDebug;
use sp_std::{prelude::Vec, vec};

/// The finality proof used by the pallet.
///
/// For a Substrate based chain using GRANDPA this will
/// be an encoded GRANDPA Justification.
#[derive(RuntimeDebug)]
pub struct FinalityProof(Vec<u8>);

impl From<&[u8]> for FinalityProof {
	fn from(proof: &[u8]) -> Self {
		Self(proof.to_vec())
	}
}

impl From<Vec<u8>> for FinalityProof {
	fn from(proof: Vec<u8>) -> Self {
		Self(proof)
	}
}

pub(crate) fn find_scheduled_change<H: HeaderT>(header: &H) -> Option<sp_finality_grandpa::ScheduledChange<H::Number>> {
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
	use crate::{BestFinalized, BestHeight, HeaderId, ImportedHeaders, PalletStorage};
	use bp_test_utils::{alice, authority_list, bob, make_justification_for_header};
	use codec::Encode;
	use frame_support::{assert_err, assert_ok};
	use frame_support::{StorageMap, StorageValue};
	use sp_finality_grandpa::{AuthorityId, SetId};
	use sp_runtime::{Digest, DigestItem};

	fn schedule_next_change(
		authorities: Vec<AuthorityId>,
		set_id: SetId,
		height: TestNumber,
	) -> ScheduledChange<TestNumber> {
		let authorities = authorities.into_iter().map(|id| (id, 1u64)).collect();
		let authority_set = AuthoritySet::new(authorities, set_id);
		ScheduledChange { authority_set, height }
	}

	// Useful for quickly writing a chain of headers to storage
	// Input is expected in the form: vec![(num, requires_justification, is_finalized)]
	fn write_headers<S: BridgeStorage<Header = TestHeader>>(
		storage: &mut S,
		headers: Vec<(u64, bool, bool)>,
	) -> Vec<ImportedHeader<TestHeader>> {
		let mut imported_headers = vec![];
		let genesis = ImportedHeader {
			header: test_header(0),
			requires_justification: false,
			is_finalized: true,
			signal_hash: None,
		};

		<BestFinalized<TestRuntime>>::put(genesis.hash());
		storage.write_header(&genesis);
		imported_headers.push(genesis);

		for (num, requires_justification, is_finalized) in headers {
			let header = ImportedHeader {
				header: test_header(num),
				requires_justification,
				is_finalized,
				signal_hash: None,
			};

			storage.write_header(&header);
			imported_headers.push(header);
		}

		imported_headers
	}

	// Given a block number will generate a chain of headers which don't require justification and
	// are not considered to be finalized.
	fn write_default_headers<S: BridgeStorage<Header = TestHeader>>(
		storage: &mut S,
		headers: Vec<u64>,
	) -> Vec<ImportedHeader<TestHeader>> {
		let headers = headers.iter().map(|num| (*num, false, false)).collect();
		write_headers(storage, headers)
	}

	// TODO: Modify this test for new code, still a good check to have
	#[ignore]
	#[test]
	fn fails_to_import_old_header() {
		run_test(|| {
			let mut storage = PalletStorage::<TestRuntime>::new();
			let parent = unfinalized_header(5);
			storage.write_header(&parent);
			storage.update_best_finalized(parent.hash());

			let header = test_header(1);
			let mut verifier = Verifier { storage };
			assert_err!(verifier.import_header(header.hash(), header), ImportError::OldHeader);
		})
	}

	// TODO: Modify this test for new code, still a good check to have
	#[ignore]
	#[test]
	fn fails_to_import_header_twice() {
		run_test(|| {
			let storage = PalletStorage::<TestRuntime>::new();
			let header = test_header(1);
			<BestFinalized<TestRuntime>>::put(header.hash());

			let imported_header = ImportedHeader {
				header: header.clone(),
				requires_justification: false,
				is_finalized: false,
				signal_hash: None,
			};
			<ImportedHeaders<TestRuntime>>::insert(header.hash(), &imported_header);

			let mut verifier = Verifier { storage };
			assert_err!(verifier.import_header(header.hash(), header), ImportError::OldHeader);
		})
	}

	// TODO: Modify this test for new code, still a good check to have
	#[ignore]
	#[test]
	fn correctly_updates_the_best_header_given_a_better_header() {
		run_test(|| {
			let mut storage = PalletStorage::<TestRuntime>::new();

			// We want to write the genesis header to storage
			let _ = write_headers(&mut storage, vec![]);

			// Write two headers at the same height to storage.
			let best_header = test_header(1);
			let mut also_best_header = test_header(1);

			// We need to change _something_ to make it a different header
			also_best_header.state_root = [1; 32].into();

			let mut verifier = Verifier {
				storage: storage.clone(),
			};

			// It should be fine to import both
			assert_ok!(verifier.import_header(best_header.hash(), best_header.clone()));
			assert_ok!(verifier.import_header(also_best_header.hash(), also_best_header));

			// The headers we manually imported should have been marked as the best
			// upon writing to storage. Let's confirm that.
			assert_eq!(storage.best_headers().len(), 2);
			assert_eq!(<BestHeight<TestRuntime>>::get(), 1);

			// Now let's build something at a better height.
			let mut better_header = test_header(2);
			better_header.parent_hash = best_header.hash();

			assert_ok!(verifier.import_header(better_header.hash(), better_header.clone()));

			// Since `better_header` is the only one at height = 2 we should only have
			// a single "best header" now.
			let best_headers = storage.best_headers();
			assert_eq!(best_headers.len(), 1);
			assert_eq!(
				best_headers[0],
				HeaderId {
					number: *better_header.number(),
					hash: better_header.hash()
				}
			);
			assert_eq!(<BestHeight<TestRuntime>>::get(), 2);
		})
	}

	// TODO: Modify this test for new code, still a good check to have
	#[ignore]
	#[test]
	fn doesnt_import_header_which_schedules_change_with_invalid_authority_set() {
		run_test(|| {
			let mut storage = PalletStorage::<TestRuntime>::new();
			let _imported_headers = write_default_headers(&mut storage, vec![1]);
			let mut header = test_header(2);

			// This is an *invalid* authority set because the combined weight of the
			// authorities is greater than `u64::MAX`
			let consensus_log = ConsensusLog::<TestNumber>::ScheduledChange(sp_finality_grandpa::ScheduledChange {
				next_authorities: vec![(alice(), u64::MAX), (bob(), u64::MAX)],
				delay: 0,
			});

			header.digest = Digest::<TestHash> {
				logs: vec![DigestItem::Consensus(GRANDPA_ENGINE_ID, consensus_log.encode())],
			};

			let mut verifier = Verifier { storage };

			assert_eq!(
				verifier.import_header(header.hash(), header).unwrap_err(),
				ImportError::InvalidAuthoritySet
			);
		})
	}

	// TODO: Modify this test for new code, still a good check to have
	#[ignore]
	#[test]
	fn finalizes_header_which_doesnt_enact_or_schedule_a_new_authority_set() {
		run_test(|| {
			let mut storage = PalletStorage::<TestRuntime>::new();
			let _imported_headers = write_default_headers(&mut storage, vec![1]);

			// Nothing special about this header, yet GRANDPA may have created a justification
			// for it since it does that periodically
			let header = test_header(2);

			let set_id = 1;
			let authorities = authority_list();
			let authority_set = AuthoritySet::new(authorities.clone(), set_id);
			storage.update_current_authority_set(authority_set);

			// We'll need this justification to finalize the header
			let grandpa_round = 1;
			let justification = make_justification_for_header(&header, grandpa_round, set_id, &authorities).encode();

			let mut verifier = Verifier {
				storage: storage.clone(),
			};

			assert_ok!(verifier.import_header(header.hash(), header.clone()));
			assert_ok!(verifier.import_finality_proof(header.hash(), justification.into()));
			assert_eq!(storage.best_finalized_header().header, header);
		})
	}

	// TODO: Modify this test for new code, still a good check to have
	#[ignore]
	#[test]
	fn updates_authority_set_upon_finalizing_header_which_enacts_change() {
		run_test(|| {
			let mut storage = PalletStorage::<TestRuntime>::new();
			let genesis_hash = write_headers(&mut storage, vec![])[0].hash();

			// We want this header to indicate that there's an upcoming set change on this fork
			let parent = ImportedHeader {
				header: test_header(1),
				requires_justification: false,
				is_finalized: false,
				signal_hash: Some(genesis_hash),
			};
			storage.write_header(&parent);

			let set_id = 1;
			let authorities = authority_list();
			let initial_authority_set = AuthoritySet::new(authorities.clone(), set_id);
			storage.update_current_authority_set(initial_authority_set);

			// This header enacts an authority set change upon finalization
			let header = test_header(2);

			let grandpa_round = 1;
			let justification = make_justification_for_header(&header, grandpa_round, set_id, &authorities).encode();

			// Schedule a change at the height of our header
			let set_id = 2;
			let height = *header.number();
			let authorities = vec![alice()];
			let change = schedule_next_change(authorities, set_id, height);
			storage.schedule_next_set_change(genesis_hash, change.clone());

			let mut verifier = Verifier {
				storage: storage.clone(),
			};

			assert_ok!(verifier.import_header(header.hash(), header.clone()));
			assert_eq!(storage.missing_justifications().len(), 1);
			assert_eq!(storage.missing_justifications()[0].hash, header.hash());

			assert_ok!(verifier.import_finality_proof(header.hash(), justification.into()));
			assert_eq!(storage.best_finalized_header().header, header);

			// Make sure that we have updated the set now that we've finalized our header
			assert_eq!(storage.current_authority_set(), change.authority_set);
			assert!(storage.missing_justifications().is_empty());
		})
	}

	// TODO: Modify this test for new code, still a good check to have
	#[ignore]
	#[test]
	fn importing_finality_proof_for_already_finalized_header_doesnt_work() {
		run_test(|| {
			let mut storage = PalletStorage::<TestRuntime>::new();
			let genesis = test_header(0);

			let genesis = ImportedHeader {
				header: genesis,
				requires_justification: false,
				is_finalized: true,
				signal_hash: None,
			};

			// Make sure that genesis is the best finalized header
			<BestFinalized<TestRuntime>>::put(genesis.hash());
			storage.write_header(&genesis);

			let mut verifier = Verifier { storage };

			// Now we want to try and import it again to see what happens
			assert_eq!(
				verifier
					.import_finality_proof(genesis.hash(), vec![4, 2].into())
					.unwrap_err(),
				FinalizationError::OldHeader
			);
		});
	}
}
