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

use crate::mock::helpers::*;
use crate::mock::*;
use crate::verifier::*;
use crate::BridgeStorage;
use crate::PalletStorage;

use std::collections::BTreeMap;

type ForkId = u32;

// (Parent Num, ForkId)
type ForksAt = Option<(TestNumber, ForkId)>;

// Delay, not block number
type ScheduledChangeAt = Option<u32>;

#[derive(Debug)]
enum Type {
	Header(u32, ForkId, ForksAt, ScheduledChangeAt),
	Finality,
}

fn create_chain<S>(storage: &mut S, chain: &mut Vec<Type>)
where
	S: BridgeStorage<Header = TestHeader>,
{
	let mut map = BTreeMap::new();

	if let Type::Header(g_num, g_fork, None, None) = chain.remove(0) {
		let genesis = test_header(g_num.into());
		map.insert(g_fork, vec![genesis]);
	}

	for h in chain {
		match h {
			Type::Header(num, fork_id, does_fork, _) => {
				// If we've never seen this fork before
				if !map.contains_key(&fork_id) {
					// Let's get the info about where to start the fork
					if let Some((parent_num, forked_from_id)) = does_fork {
						let fork = &*map.get(&forked_from_id).unwrap();
						let parent = fork.iter().find(|h| h.number == *parent_num).unwrap();

						// TODO: Handle numbers better
						let mut header = test_header(*num as u64);
						header.parent_hash = parent.hash();
						header.state_root = [*fork_id as u8; 32].into();

						// Start a new fork
						map.insert(*fork_id, vec![header]);
					}
				} else {
					// We've seen this fork before, just append
					let parent_hash = {
						let fork = &*map.get(&fork_id).unwrap();
						let parent = fork.last().unwrap();
						parent.hash()
					};

					// TODO: Handle numbers better here
					let mut header = test_header(*num as u64);
					header.parent_hash = parent_hash;

					// Doing this to make sure headers at the same height but on
					// different forks have different hashes
					header.state_root = [*fork_id as u8; 32].into();

					map.get_mut(&fork_id).unwrap().push(header);
				}
			}
			Type::Finality => todo!(),
		}
	}

	for (key, value) in map.iter() {
		println!("{}: {:#?}", key, value);
	}
}

#[test]
fn fork_test_importing_headers_with_new_method() {
	run_test(|| {
		let mut storage = PalletStorage::<TestRuntime>::new();

		let mut chain = vec![
			Type::Header(1, 1, None, None),
			Type::Header(2, 1, None, None),
			Type::Header(3, 2, Some((2, 1)), None),
			Type::Header(3, 1, None, None),
		];

		create_chain(&mut storage, &mut chain);
		panic!()
	})
}
