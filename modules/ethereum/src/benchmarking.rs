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
	import_unsigned_header_worst_case {
		let n in 1..1000;

		// Initialize storage with some initial header
		let initial_header = build_genesis_header(&validator(0));
		let initial_header_hash = initial_header.compute_hash();
		let initial_difficulty = initial_header.difficulty;
		let initial_validators = validators_addresses(3);

		let mut storage = BridgeStorage::<T>::new();

		initialize_storage::<T>(
			&initial_header,
			initial_difficulty,
			&initial_validators,
		);

		let header1 = build_custom_header(
			&validator(1),
			&initial_header,
			|mut header| {
				header
			},
		);

		insert_header(&mut storage, header1.clone());

		// This _should_ finalize the first block
		// First block has 2 votes, which is 2/3 authorities
		let header2 = build_custom_header(
			&validator(2),
			&header1,
			|mut header| {
				header
			},
		);

	}: import_unsigned_header(RawOrigin::None, header2, None)
	verify {
		let storage = BridgeStorage::<T>::new();
		assert_eq!(storage.best_block().0.number, 2);
		assert_eq!(storage.finalized_block().number, 1);
	}

	// Our goal with this bench is to try and see the effect that finalizing difference ranges of
	// blocks has on our import time. As such we need to make sure that we keep the number of
	// validators fixed while changing the number blocks finalized (the complixity parameter) by
	// importing the last header.
	import_unsigned_header_finality {
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
		let mut ancestry: Vec<FinalityAncestor<Option<Address>>> = Vec::new();
		let mut parent = initial_header.clone();

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
	fn insert_unsigned_header_worst_case() {
		// I don't think the initial validators matters
		// I think we override them in the benchmark anyways
		run_test(1, |_| {
			assert_ok!(test_benchmark_import_unsigned_header_worst_case::<TestRuntime>());
		});
	}

	#[test]
	fn insert_unsigned_header_finality() {
		run_test(1, |_| {
			assert_ok!(test_benchmark_import_unsigned_header_finality::<TestRuntime>());
		});
	}
}
