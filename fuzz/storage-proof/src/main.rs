// Copyright 2019-2021 Parity Technologies (UK) Ltd.
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

//! Storage Proof Checker fuzzer.

#![warn(missing_docs)]

use honggfuzz::fuzz;
// Logic for checking Substrate storage proofs.

use bp_runtime::UnverifiedStorageProof;
use sp_core::{storage::StateVersion, Blake2Hasher};
use sp_std::vec::Vec;
use std::collections::HashMap;

fn transform_into_unique(
	input_vec: Vec<(Vec<u8>, Option<Vec<u8>>)>,
) -> Vec<(Vec<u8>, Option<Vec<u8>>)> {
	let mut output_hashmap = HashMap::new();
	let mut output_vec = Vec::new();
	for key_value_pair in input_vec {
		output_hashmap.insert(key_value_pair.0, key_value_pair.1); //Only 1 value per key
	}
	for (key, val) in output_hashmap.iter() {
		output_vec.push((key.clone(), val.clone()));
	}
	output_vec
}

fn run_fuzzer() {
	fuzz!(|input_vec: Vec<(Vec<u8>, Option<Vec<u8>>)>| {
		if input_vec.is_empty() {
			return
		}
		let unique_input_vec = transform_into_unique(input_vec);
		let (root, storage_proof) = UnverifiedStorageProof::try_from_entries::<Blake2Hasher>(
			StateVersion::default(),
			&unique_input_vec,
		)
		.expect("UnverifiedStorageProof::try_from_entries() shouldn't fail");
		let mut storage = storage_proof
			.verify::<Blake2Hasher>(StateVersion::V1, &root)
			.expect("UnverifiedStorageProof::verify() shouldn't fail");

		for key_value_pair in &unique_input_vec {
			log::info!("Reading value for pair {:?}", key_value_pair);
			assert_eq!(storage.get(&key_value_pair.0), Ok(&key_value_pair.1));
		}
	})
}

fn main() {
	env_logger::init();

	loop {
		run_fuzzer();
	}
}
