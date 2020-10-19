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
//! Each test is depicted using beautiful ASCII art. The symbols used in the tests are the
//! following:
//!
//! - S|N: Schedules change in N blocks
//! - E: Enacts change
//! - F: Finalized
//! - FN: Finality proof imported for header N
//!
//! Each diagram also comes with an import order. This is important since we expect things to fail
//! when headers or proofs are imported in a certain order.
//!
//! Tests can be read as follows:
//!
//! ## Example Import 1
//!
//! (Type::Header(2, 1, None, None), Ok(()))
//!
//! Import header 2 on fork 1. This does not create a fork, or schedule an authority set change. We
//! expect this header import to be succesful.
//!
//! ## Example Import 2
//!
//! (Type::Header(2, 2, Some((3, 1)), Some(0)), Ok(()))
//!
//! Import header 2 on fork 2. This header starts a new fork from header 3 on fork 1. It also
//! schedules a change with a delay of 0 blocks. It should be succesfully imported.
//!
//! ## Example Import 3
//!
//! (Type::Finality(2, 1), Err(()))
//!
//! Import a finality proof for header 2 on fork 1. This finalty proof should fail to be imported.

use crate::justification::tests::*;
use crate::mock::helpers::*;
use crate::mock::*;
use crate::storage::{AuthoritySet, ImportedHeader};
use crate::verifier::*;
use crate::BridgeStorage;
use crate::{BestFinalized, ChainTipHeight, PalletStorage};
use codec::Encode;
use frame_support::{StorageMap, StorageValue};
use sp_finality_grandpa::{ConsensusLog, GRANDPA_ENGINE_ID};
use sp_runtime::{Digest, DigestItem};

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
			(Type::Header(num, fork_id, does_fork, schedules_change), expected_result) => {
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

						if let Some(delay) = schedules_change {
							header.digest = change_log(*delay as u64);
						}

						// Try and import into storage
						let res = verifier.import_header(header.clone()).map_err(|_| ());
						assert_eq!(
							res, *expected_result,
							"Expected {:?} while importing header {}",
							*expected_result, *num
						);
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

					if let Some(delay) = schedules_change {
						header.digest = change_log(*delay as u64);
					}

					// Try and import into storage
					// TODO: Should check errors
					let res = verifier.import_header(header.clone()).map_err(|_| ());
					assert_eq!(
						res, *expected_result,
						"Expected {:?} while importing header {}",
						*expected_result, *num
					);
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
				assert_eq!(
					res, *expected_result,
					"Expected {:?} while importing finality proof for header {}",
					*expected_result, *num
				);
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

fn change_log(delay: u64) -> Digest<TestHash> {
	let consensus_log = ConsensusLog::<TestNumber>::ScheduledChange(sp_finality_grandpa::ScheduledChange {
		next_authorities: vec![(alice(), 1), (bob(), 1)],
		delay,
	});

	Digest::<TestHash> {
		logs: vec![DigestItem::Consensus(GRANDPA_ENGINE_ID, consensus_log.encode())],
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
		assert_eq!(storage.best_finalized_header().header.number, 2);
	})
}

#[test]
fn fork_can_import_header_which_schedules_set_change() {
	run_test(|| {
		let mut storage = PalletStorage::<TestRuntime>::new();

		let mut chain = vec![
			(Type::Header(1, 1, None, None), Ok(())),
			(Type::Header(2, 1, None, Some(0)), Ok(())),
		];

		create_chain(&mut storage, &mut chain);
	})
}

// Order: 1, 2, 2', F2, F2'
//
// [1] <- [2: F]
//   \ [2']
//
// Not allowed to finalize 2'
//
#[test]
fn fork_does_not_allow_competing_finality_proofs() {
	run_test(|| {
		let mut storage = PalletStorage::<TestRuntime>::new();

		let mut chain = vec![
			(Type::Header(1, 1, None, None), Ok(())),
			(Type::Header(2, 1, None, None), Ok(())),
			(Type::Header(2, 2, Some((1, 1)), None), Ok(())),
			(Type::Finality(2, 1), Ok(())),
			(Type::Finality(2, 2), Err(())),
		];

		create_chain(&mut storage, &mut chain);
	})
}

// Order: 1, 2, 3, 4
//
// [1] <- [2: S|1] <- [3: E] <- [4]
//
// Not allowed to import 4
#[test]
// FIXME: This fails
fn fork_does_not_allow_importing_past_header_which_enacts_change() {
	run_test(|| {
		let mut storage = PalletStorage::<TestRuntime>::new();

		let mut chain = vec![
			(Type::Header(1, 1, None, None), Ok(())),
			(Type::Header(2, 1, None, Some(1)), Ok(())),
			(Type::Header(3, 1, None, None), Ok(())),
			(Type::Header(4, 1, None, None), Err(())),
		];

		create_chain(&mut storage, &mut chain);
	})
}

// TODO: This is basically a duplicate of the above
#[test]
// FIXME: This fails
fn fork_does_not_import_headers_past_once_which_enacts_a_change() {
	run_test(|| {
		let mut storage = PalletStorage::<TestRuntime>::new();

		let mut chain = vec![
			(Type::Header(1, 1, None, None), Ok(())),
			(Type::Header(2, 1, None, Some(0)), Ok(())),
			(Type::Header(3, 1, None, None), Err(())),
			(Type::Finality(2, 1), Ok(())),
			(Type::Header(3, 1, None, None), Ok(())),
		];

		create_chain(&mut storage, &mut chain);
	})
}

// Order: 1, 2, 2', 3, 3'
//
//   / [2: S|0] <- [3]
// [1] <- [2': S|0] <- [3']
//
//
// Should not be allowed to import 3 or 3'
//    Ancestor enacts an authority set change
// Will resolve fork depending on which finality proof for {2|2'} we get first
//
// NOTE: The test above is basically a duplicate of the above two tests, maybe
// just keep this one
#[test]
// FIXME: This fails
fn fork_does_not_allow_importing_past_header_that_enacts_changes_on_forks() {
	run_test(|| {
		let mut storage = PalletStorage::<TestRuntime>::new();

		let mut chain = vec![
			(Type::Header(1, 1, None, None), Ok(())),
			(Type::Header(2, 1, None, Some(0)), Ok(())),
			(Type::Header(2, 2, Some((1, 1)), Some(0)), Ok(())),
			(Type::Header(3, 1, None, None), Err(())),
			(Type::Header(3, 2, None, None), Err(())),
		];

		create_chain(&mut storage, &mut chain);
	})
}

// Order: 1, 2, 3, 2', 3'
//
//   / [2'] <- [3']
// [1] <- [2: S|0] <- [3]
//
// Not allowed to import 3
// Fine to import 2' and 3'
#[test]
// FIXME: This fails
fn fork_allows_importing_on_different_fork_while_waiting_for_finality_proof() {
	run_test(|| {
		let mut storage = PalletStorage::<TestRuntime>::new();

		let mut chain = vec![
			(Type::Header(1, 1, None, None), Ok(())),
			(Type::Header(2, 1, None, Some(0)), Ok(())),
			(Type::Header(3, 1, None, None), Err(())),
			(Type::Header(2, 2, Some((1, 1)), None), Ok(())),
			(Type::Header(3, 2, None, None), Ok(())),
		];

		create_chain(&mut storage, &mut chain);
	})
}

// Order: 1, 2, 2', F2, 3, 3'
//
//   / [2'] <- [3']
// [1] <- [2: F] <- [3]
//
// Allowed to import 3
// Should not be allowed to import 3'
//   Will need to check ancestry with `last_finalized` upon import
// In current impl we'd be allowed to import 3', but we'd never finalize anything
// on that fork
//
// NOTE: I don't think we should allow importing 3'
#[test]
fn fork_does_not_allow_importing_on_different_fork_past_finalized_header() {
	run_test(|| {
		let mut storage = PalletStorage::<TestRuntime>::new();

		let mut chain = vec![
			(Type::Header(1, 1, None, None), Ok(())),
			(Type::Header(2, 1, None, Some(0)), Ok(())),
			(Type::Header(2, 2, Some((1, 1)), None), Ok(())),
			(Type::Finality(2, 1), Ok(())),
			(Type::Header(3, 1, None, None), Ok(())),
			(Type::Header(3, 2, None, None), Ok(())), // I think this should be an Err...
		];

		create_chain(&mut storage, &mut chain);
	})
}

// Order: 1, 2, 3, 4, 3', 4'
//
//                  / [3': E] <- [4']
// [1] <- [2: S|1] <- [3: E] <- [4]
//
// Not allowed to import {4, 4'}
#[test]
// FIXME: This fails
fn fork_can_track_scheduled_changes_across_forks() {
	run_test(|| {
		let mut storage = PalletStorage::<TestRuntime>::new();

		let mut chain = vec![
			(Type::Header(1, 1, None, None), Ok(())),
			(Type::Header(2, 1, None, Some(1)), Ok(())),
			(Type::Header(3, 1, None, None), Ok(())),
			(Type::Header(4, 1, None, None), Err(())),
			(Type::Header(3, 2, Some((2, 1)), None), Ok(())),
			(Type::Header(4, 2, None, None), Err(())),
		];

		create_chain(&mut storage, &mut chain);
	})
}
