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

use crate::test_utils::{build_custom_header, build_genesis_header, validator_utils::*};

use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;
use primitives::U256;

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
		// Make sure that the header got stored by the pallet
		assert_eq!(BridgeStorage::<T>::new().best_block().0.number, 1);
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

		// TODO: Wrap this in a nicer way
		let mut header_to_import = HeaderToImport {
			context: storage.import_context(None, &header1.parent_hash).unwrap(),
			is_best: true,
			id: header1.compute_id(),
			header: header1.clone(),
			total_difficulty: header1.difficulty, // 0.into(),
			enacted_change: None,
			scheduled_change: None,
			finality_votes: Default::default(), // Maybe update
		};

		storage.insert_header(header_to_import);

		// This _should_ finalize the genesis block
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
		// assert_eq!(storage.finalized_block().number, 0);
	}
}
