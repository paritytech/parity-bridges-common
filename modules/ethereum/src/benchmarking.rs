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

use super::*;

use crate::finality::FinalityAncestor;
use crate::test_utils::{build_custom_header, build_genesis_header, insert_header, validator_utils::*, HeaderBuilder};

use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;
use primitives::{Address, U256};

// Benchmark `import_unsigned_header` extrinsic with the worst possible conditions
//
// Internally this calls `import_header`, which will finalize a bunch of blocks if it can
//	   How many should we finalize for the benchmark? The numebr of finalized headers will
//	   affect the benchmark
//
// We want to require receipts
//
// We also want to trigger some pruning as well
//   Need to look at the pruning strategy, I think it's 10 blocks behind right now
//
// The new block should schedule a validator change
//
// Look at the tests (e.g import.rs) for inspiration
benchmarks! {
	_ { }

	// Benchmark `import_unsigned_header` extrinsic with the best possible conditions:
	// * Parent header is finalized.
	// * New header doesn't require receipts.
	// * Nothing is finalized by new header.
	// * Nothing is pruned by new header.
	import_unsigned_header_best_case {
		let n in 1..1000;

		// Initialize storage with some initial header
		let initial_header = build_genesis_header(&validator(0));
		let initial_header_hash = initial_header.compute_hash();
		let initial_difficulty = initial_header.difficulty;
		initialize_storage::<T>(
			&initial_header,
			initial_difficulty,
			&validators_addresses(2),
		);

		// prepare header to be inserted
		let header = build_custom_header(
			&validator(1),
			&initial_header,
			|mut header| {
				header.gas_limit = header.gas_limit + U256::from(n);
				header
			},
		);

	}: import_unsigned_header(RawOrigin::None, header, None)
	verify {
		let storage = BridgeStorage::<T>::new();
		assert_eq!(storage.best_block().0.number, 1);
		assert_eq!(storage.finalized_block().number, 0);
	}

	// Our goal with this bench is to try and see the effect that finalizing difference ranges of
	// blocks has on our import time. As such we need to make sure that we keep the number of
	// validators fixed while changing the number blocks finalized (the complixity parameter) by
	// importing the last header.
	import_unsigned_finality {
		// Our complexity parameter, n, will represent the number of blocks imported before
		// finalization.
		//
		// For two validators this is only going to work for even numbers...
		let n in 4..10;

		// This should remain fixed for the bench.
		let num_validators: u32 = 2;

		let mut storage = BridgeStorage::<T>::new();

		// Initialize storage with some initial header
		let initial_header = build_genesis_header(&validator(0));
		let initial_header_hash = initial_header.compute_hash();
		let initial_difficulty = initial_header.difficulty;
		let initial_validators = validators_addresses(num_validators as usize);

		initialize_storage::<T>(
			&initial_header,
			initial_difficulty,
			&initial_validators,
		);

		let mut headers = Vec::new();
		let mut ancestry: Vec<FinalityAncestor<Option<Address>>> = Vec::new(); // Is this needed?
		let mut parent = initial_header.clone();

		// Q: Is this a sketchy way of "delaying" finality, or should I be manually editing the
		// "step" of the new blocks?
		for i in 1..=n {
			let header = build_custom_header(
				&validator(0),
				&parent,
				|mut header| {
					header
				},
			);

			let id = header.compute_id();
			insert_header(&mut storage, header.clone());
			ancestry.push(FinalityAncestor {
				id: header.compute_id(),
				submitter: None,
				signers: vec![header.author].into_iter().collect(),
			});
			headers.push(header.clone());
			parent = header;
		}

		// NOTE: I should look into using `sign_by_set()`
		let last_header = headers.last().unwrap().clone();
		let last_authority = validator(1);

		// Need to make sure that the header we're going to import hasn't been inserted
		// into storage already
		let header = build_custom_header(
			&last_authority,
			&last_header,
			|mut header| {
				header
			},
		);
	}: import_unsigned_header(RawOrigin::None, header, None)
	verify {
		let storage = BridgeStorage::<T>::new();
		assert_eq!(storage.best_block().0.number, (n + 1) as u64);
		assert_eq!(storage.finalized_block().number, n as u64);
	}

	// The default pruning range is 10 blocks behind. We'll start with this for the bench, but we
	// should move to a "dynamic" strategy based off the complexity parameter
	//
	// Look at `headers_are_pruned_during_import()` test from `import.rs`
	import_unsigned_prune {
		let n = 10..10;

		// TODO: Wrap this so we don't repeat so much between benches
		// Initialize storage with some initial header
		let initial_header = build_genesis_header(&validator(0));
		let initial_header_hash = initial_header.compute_hash();
		let initial_difficulty = initial_header.difficulty;
		let initial_validators = validators_addresses(3);

		initialize_storage::<T>(
			&initial_header,
			initial_difficulty,
			&initial_validators,
		);

		for i in 1..=n {
			let header = HeaderBuilder::with_parent_number(i - 1).sign_by_set(&initial_validators);
			let id = header.compute_id();
			insert_header(&mut storage, header.clone());
		}

		// We want block 11 to finalize header 10 as well as schedule a validator set change
		// This will allow us to prune blocks [0, 10]
		storage.scheduled_change(...).unwrap();


	} verify {
		let storage = BridgeStorage::<T>::new();
		assert_eq!(storage.best_block().0.number, (n + 1) as u64);
		assert_eq!(storage.finalized_block().number, n as u64);
	}

	import_unsigned_scheduled_changes {
		ScheduledChanges::insert(
			hash,
			ScheduledChange {
				validators: validators_addresses(5),
				prev_signal_block: None,
			},
		);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{run_test, TestRuntime};
	use frame_support::assert_ok;

	#[test]
	fn insert_unsigned_header_best_case() {
		run_test(2, |_| {
			assert_ok!(test_benchmark_import_unsigned_header_best_case::<TestRuntime>());
		});
	}

	#[test]
	fn insert_unsigned_header_finality() {
		run_test(1, |_| {
			assert_ok!(test_benchmark_import_unsigned_finality::<TestRuntime>());
		});
	}
}
