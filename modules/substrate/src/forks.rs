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

//! Tests for checking that behaviour of importing header and finality
//! proofs works correctly, especially around forking conditions.
//!
//! Each test is depicted using beautiful ASCII art, and uses the following symbols:
//!
//!
//! S|N: Schedules change in N blocks
//! E: Enacts change
//! F: Finalized
//! FN: Finality proof imported for header N
//!
//! Order: Import order

use crate::justification::tests::*;
use crate::mock::helpers::*;
use crate::mock::*;
use crate::storage::{AuthoritySet, ImportedHeader};
use crate::verifier::*;
use crate::BridgeStorage;
use crate::{BestFinalized, ChainTipHeight, PalletStorage};
use codec::Encode;
use frame_support::{StorageMap, StorageValue};

use std::collections::BTreeMap;

type ForkId = u32;

// (Parent Num, ForkId)
type ForksAt = Option<(TestNumber, ForkId)>;

// Delay, not block number
type ScheduledChangeAt = Option<u32>;

#[derive(Debug)]
enum Type {
	Header(u32, ForkId, ForksAt, ScheduledChangeAt),
	Finality(TestNumber, ForkId),
}

fn create_chain<S>(storage: &mut S, chain: &mut Vec<(Type, Result<(), ()>)>)
where
	S: BridgeStorage<Header = TestHeader> + Clone,
{
	let mut map = BTreeMap::new();

	let mut verifier = Verifier {
		storage: storage.clone(),
	};

	if let (Type::Header(g_num, g_fork, None, None), _) = chain.remove(0) {
		let genesis = test_header(g_num.into());
		map.insert(g_fork, vec![genesis.clone()]);

		// Bootstrap stuff, maybe move out of this function
		let genesis = ImportedHeader {
			header: genesis,
			requires_justification: false,
			is_finalized: true,
		};

		<BestFinalized<TestRuntime>>::put(genesis.hash());
		storage.write_header(&genesis);
	}

	// Maybe also move this to an init() helper
	let set_id = 1;
	let authorities = authority_list();
	let authority_set = AuthoritySet::new(authorities.clone(), set_id);
	storage.update_current_authority_set(authority_set);

	for h in chain {
		match h {
			(Type::Header(num, fork_id, does_fork, _), expected_result) => {
				// If we've never seen this fork before
				if !map.contains_key(&fork_id) {
					// Let's get the info about where to start the fork
					if let Some((parent_num, forked_from_id)) = does_fork {
						let fork = &*map.get(&forked_from_id).unwrap();
						let parent = fork
							.iter()
							.find(|h| h.number == *parent_num)
							.expect("Trying to fork on a parent which doesn't exist");

						// TODO: Handle numbers better
						let mut header = test_header(*num as u64);
						header.parent_hash = parent.hash();
						header.state_root = [*fork_id as u8; 32].into();

						// Try and import into storage
						let res = verifier.import_header(header.clone()).map_err(|_| ());
						assert_eq!(res, *expected_result);
						match res {
							Ok(_) => {
								// Let's mark the header down in a new fork
								map.insert(*fork_id, vec![header]);
							}
							Err(_) => {
								eprintln!("Unable to import header ({:?}, {:?})", num, fork_id);
							}
						}
					}
				} else {
					// We've seen this fork before so let's append our new header to it
					let parent_hash = {
						let fork = &*map.get(&fork_id).unwrap();
						fork.last().unwrap().hash()
					};

					// TODO: Handle numbers better here
					let mut header = test_header(*num as u64);
					header.parent_hash = parent_hash;

					// Doing this to make sure headers at the same height but on
					// different forks have different hashes
					header.state_root = [*fork_id as u8; 32].into();

					// Try and import into storage
					// TODO: Should check errors
					let res = verifier.import_header(header.clone()).map_err(|_| ());
					assert_eq!(res, *expected_result);
					match res {
						Ok(_) => {
							map.get_mut(&fork_id).unwrap().push(header);
						}
						Err(_) => {
							eprintln!("Unable to import header ({:?}, {:?})", num, fork_id);
						}
					}
				}
			}
			(Type::Finality(num, fork_id), expected_result) => {
				let header = map[fork_id]
					.iter()
					.find(|h| h.number == *num)
					.expect("Trying to finalize block that doesn't exist");

				// TODO: Tests pass when I create multiple justifications even though I don't change
				// these, is that a problem?
				let grandpa_round = 1;
				let set_id = 1;
				let authorities = authority_list();
				let justification =
					make_justification_for_header(&header, grandpa_round, set_id, &authorities).encode();

				let res = verifier
					.import_finality_proof(header.hash(), justification.into())
					.map_err(|_| ());
				assert_eq!(res, *expected_result);
				match res {
					Ok(_) => {}
					Err(_) => {
						eprintln!("Unable to import finality proof for header ({:?}, {:?})", num, fork_id);
					}
				}
			}
		}
	}

	for (key, value) in map.iter() {
		println!("{}: {:#?}", key, value);
	}
}

#[test]
fn fork_can_import_headers_on_same_fork() {
	run_test(|| {
		let mut storage = PalletStorage::<TestRuntime>::new();

		let mut chain = vec![
			(Type::Header(1, 1, None, None), Ok(())),
			(Type::Header(2, 1, None, None), Ok(())),
			(Type::Header(3, 1, None, None), Ok(())),
		];

		create_chain(&mut storage, &mut chain);
	})
}

#[test]
fn fork_can_import_headers_on_different_forks() {
	run_test(|| {
		let mut storage = PalletStorage::<TestRuntime>::new();

		let mut chain = vec![
			(Type::Header(1, 1, None, None), Ok(())),
			(Type::Header(2, 1, None, None), Ok(())),
			(Type::Header(2, 2, Some((1, 1)), None), Ok(())),
			(Type::Header(3, 1, None, None), Ok(())),
			(Type::Header(3, 3, Some((2, 2)), None), Ok(())),
		];

		create_chain(&mut storage, &mut chain);

		let best_headers: Vec<TestHeader> = storage.best_headers().into_iter().map(|i| i.header).collect();
		assert_eq!(best_headers.len(), 2);
		assert_eq!(<ChainTipHeight<TestRuntime>>::get(), 3);
	})
}

#[test]
fn fork_can_import_finality_proof() {
	run_test(|| {
		let mut storage = PalletStorage::<TestRuntime>::new();

		let mut chain = vec![
			(Type::Header(1, 1, None, None), Ok(())),
			(Type::Header(2, 1, None, None), Ok(())),
			(Type::Finality(2, 1), Ok(())),
		];

		create_chain(&mut storage, &mut chain);
		assert_eq!(storage.best_finalized_header().header.number, 3);
	})
}
