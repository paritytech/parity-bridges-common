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
use crate::validators::{Validators, ValidatorsConfiguration};

use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;
use hex_literal::hex;
use primitives::{Address, Receipt, H256, U256};

// We want to try and benchmark scenario which are going to cause a lot for work for our runtime.
// Some of the ones which we should test that are still missing are:
//    - Importing a header with transaction receipts
//    - An import which causes a chain re-org
benchmarks! {
	_ { }

	// Benchmark `import_unsigned_header` extrinsic with the best possible conditions:
	// * Parent header is finalized.
	// * New header doesn't require receipts.
	// * Nothing is finalized by new header.
	// * Nothing is pruned by new header.
	import_unsigned_header_best_case {
		let n in 1..1000;

		let num_validators = 2;
		let initial_header = initialize_bench::<T>(num_validators);

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
	// validators fixed while changing the number blocks finalized (the complexity parameter) by
	// importing the last header.
	//
	// One important thing to keep in mind is that the runtime provides a finality cache in order to
	// reduce the overhead of header finalization. However, this is only triggered every 16 blocks.
	import_unsigned_finality {
		// Our complexity parameter, n, will represent the number of blocks imported before
		// finalization.
		let n in 1..7;

		let mut storage = BridgeStorage::<T>::new();
		let num_validators: u32 = 2;
		let initial_header = initialize_bench::<T>(num_validators as usize);

		// Since we only have two validators we need to make sure the number of blocks is even to
		// make sure the right validator signs the final block
		let num_blocks = 2 * n;
		let mut headers = Vec::new();
		let mut parent = initial_header.clone();

		// Import a bunch of headers without any verification, will ensure that they're not
		// finalized prematurely
		for i in 1..=num_blocks {
			let header = build_custom_header(
				&validator(0),
				&parent,
				|mut header| {
					header
				},
			);

			let id = header.compute_id();
			insert_header(&mut storage, header.clone());
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
		assert_eq!(storage.best_block().0.number, (num_blocks + 1) as u64);
		assert_eq!(storage.finalized_block().number, num_blocks as u64);
	}

	// Basically the exact same as `import_unsigned_finality` but with a different range for the
	// complexity parameter. In this bench we use a larger range of blocks to see how performance
	// changes when the finality cache kicks in (>16 blocks).
	//
	// There's definitely a better way to do this will less code duplication, but I'll deal with that
	// later.
	import_unsigned_finality_with_cache {
		// Our complexity parameter, n, will represent the number of blocks imported before
		// finalization.
		let n in 8..15;

		let mut storage = BridgeStorage::<T>::new();
		let num_validators: u32 = 2;
		let initial_header = initialize_bench::<T>(num_validators as usize);

		// Since we only have two validators we need to make sure the number of blocks is even to
		// make sure the right validator signs the final block
		let num_blocks = 2 * n;
		let mut headers = Vec::new();
		let mut parent = initial_header.clone();

		// Import a bunch of headers without any verification, will ensure that they're not
		// finalized prematurely
		for i in 1..=num_blocks {
			let header = build_custom_header(
				&validator(0),
				&parent,
				|mut header| {
					header
				},
			);

			let id = header.compute_id();
			insert_header(&mut storage, header.clone());
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
		assert_eq!(storage.best_block().0.number, (num_blocks + 1) as u64);
		assert_eq!(storage.finalized_block().number, num_blocks as u64);
	}

	// The default pruning range is 10 blocks behind. We'll start with this for the bench, but we
	// should move to a "dynamic" strategy based off the complexity parameter
	//
	// Look at `headers_are_pruned_during_import()` test from `import.rs`
	//
	// So it looks like we're constrained by: MAX_BLOCKS_TO_PRUNE_IN_SINGLE_IMPORT= 8
	//
	// So it doesn't matter how we set the pruning window or how many blocks we build because at the
	// end of the day we can only prune that many blocks
	import_unsigned_pruning {
		// The default pruning strategy is to keep 10 headers, so let's build more than 10
		let n in 10..20;

		let mut storage = BridgeStorage::<T>::new();

		let num_validators = 3;
		let initial_header = initialize_bench::<T>(num_validators as usize);
		let validators = validators(num_validators);

		// Want to prune eligible blocks between [0, 10)
		BlocksToPrune::put(PruningRange {
			oldest_unpruned_block: 0,
			oldest_block_to_keep: 10,
		});

		let mut parent = initial_header;
		for i in 1..=n {
			let header = HeaderBuilder::with_parent(&parent).sign_by_set(&validators);
			let id = header.compute_id();
			insert_header(&mut storage, header.clone());
			parent = header;
		}

		let header = HeaderBuilder::with_parent(&parent).sign_by_set(&validators);
	}: import_unsigned_header(RawOrigin::None, header, None)
	verify {
		let storage = BridgeStorage::<T>::new();
		assert_eq!(storage.best_block().0.number, (n + 1) as u64);

		// We're limited to pruning only 8 blocks per import
		assert!(HeadersByNumber::get(&0).is_none());
		assert!(HeadersByNumber::get(&7).is_none());
	}

	// The goal of this bench is to import a block which contains a transaction receipt. The receipt
	// will contain a validator set change. Verifying the receipt root is an expensive operation to
	// do, which is why we're interested in benchmarking it.
	import_unsigned_with_receipts {
		let n in 1..10;

		let mut storage = BridgeStorage::<T>::new();

		let num_validators = 1;
		let initial_header = initialize_bench::<T>(num_validators as usize);
		let receipts = vec![validators_change_receipt(Default::default())];

		// We need this extra header since this is what signals a validator set transition. This
		// will ensure that the next header is within the "Contract" window
		let header1 = HeaderBuilder::with_parent(&initial_header).sign_by(&validator(0));
		insert_header(&mut storage, header1.clone());

		let header = build_custom_header(
			&validator(0),
			&header1,
			|mut header| {
				// Logs Bloom signals a change in validator set
				header.log_bloom = (&[0xff; 256]).into();
				header.receipts_root =
					hex!("81ce88dc524403b796222046bf3daf543978329b87ffd50228f1d3987031dc45").into();
				header
			},
		);
	}: import_unsigned_header(RawOrigin::None, header, Some(receipts))
	verify {
		let storage = BridgeStorage::<T>::new();
		assert_eq!(storage.best_block().0.number, 2);

		// assert_eq!(
		// 	validators.extract_validators_change(&header, Some(receipts)),
		// 	Ok((Some(vec![[7; 20].into()]), None)),
		// );
	}
}

fn initialize_bench<T: Trait>(num_validators: usize) -> Header {
	// Initialize storage with some initial header
	let initial_header = build_genesis_header(&validator(0));
	let initial_header_hash = initial_header.compute_hash();
	let initial_difficulty = initial_header.difficulty;
	let initial_validators = validators_addresses(num_validators as usize);

	initialize_storage::<T>(&initial_header, initial_difficulty, &initial_validators);

	initial_header
}

// TODO: Stole these from validators.rs, need to move them to a shared place

/// The hash of InitiateChange event of the validators set contract.
const CHANGE_EVENT_HASH: &'static [u8; 32] = &[
	0x55, 0x25, 0x2f, 0xa6, 0xee, 0xe4, 0x74, 0x1b, 0x4e, 0x24, 0xa7, 0x4a, 0x70, 0xe9, 0xc1, 0x1f, 0xd2, 0xc2, 0x28,
	0x1d, 0xf8, 0xd6, 0xea, 0x13, 0x12, 0x6f, 0xf8, 0x45, 0xf7, 0x82, 0x5c, 0x89,
];

fn validators_change_receipt(parent_hash: H256) -> Receipt {
	use primitives::{LogEntry, TransactionOutcome};

	Receipt {
		gas_used: 0.into(),
		log_bloom: (&[0xff; 256]).into(),
		outcome: TransactionOutcome::Unknown,
		logs: vec![LogEntry {
			address: [3; 20].into(),
			topics: vec![CHANGE_EVENT_HASH.into(), parent_hash],
			data: vec![
				0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
				0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 7, 7, 7, 7,
				7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
			],
		}],
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

	#[test]
	fn insert_unsigned_header_pruning() {
		run_test(1, |_| {
			assert_ok!(test_benchmark_import_unsigned_pruning::<TestRuntime>());
		});
	}

	#[test]
	fn insert_unsigned_header_receipts() {
		run_test(1, |_| {
			assert_ok!(test_benchmark_import_unsigned_with_receipts::<TestRuntime>());
		});
	}
}
