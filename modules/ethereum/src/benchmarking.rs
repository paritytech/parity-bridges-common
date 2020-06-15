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

use frame_system::RawOrigin;
use frame_benchmarking::benchmarks;
use primitives::public_to_address;

benchmarks! {
	_ { }

	// Benchmark `import_unsigned_header` extrinsic with the best possible conditions:
	// * Parent header is finalized.
	// * New header doesn't require receipts.
	// * Nothing is finalized by new header.
	// * Nothing is pruned by new header.
	import_unsigned_header_best_case {
		let n in 1..1000;

		// Substrate uses compressed pubkeys, and we need full pubkey to compute
		// ethereum address => will use workaround here
		let initial_validators = vec![
			prepare_validator(0),
			prepare_validator(1),
		];

		// initialize storage with some initial header
		let initial_header = prepare_header(
			0,
			Default::default(),
			0,
			&initial_validators[0],
			|header| header,
		);
		let initial_header_hash = initial_header.compute_hash();
		let initial_difficulty = 0.into();
		initialize_storage::<T>(
			&initial_header,
			initial_difficulty,
			&initial_validators.iter().map(|(_, address)| *address).collect::<Vec<_>>(),
		);

		// prepare header to be inserted
		let header = prepare_header(
			1,
			initial_header_hash,
			1,
			&initial_validators[1],
			|mut header| {
				header.gas_limit = header.gas_limit + U256::from(n);
				header
			}
		);
	}: import_unsigned_header(RawOrigin::None, header, None)
	verify {
		assert_eq!(BridgeStorage::<T>::new().best_block().0.number, 1);
	}
}

fn prepare_validator(index: u8) -> (secp256k1::SecretKey, Address) {
	let secret_key = secp256k1::SecretKey::parse(&[index + 1; 32]).unwrap();
	let public_key = secp256k1::PublicKey::from_secret_key(&secret_key);
	let mut public_key_raw = [0u8; 64];
	public_key_raw.copy_from_slice(&public_key.serialize()[1..]);
	let address = public_to_address(&public_key_raw);
	(secret_key, address)
}

fn prepare_header(
	number: u64,
	parent_hash: H256,
	step: u64,
	validator: &(secp256k1::SecretKey, Address),
	customize: impl FnOnce(Header) -> Header,
) -> Header {
	let mut header = customize(Header {
		number,
		parent_hash,
		gas_limit: 0x2000.into(),
		author: validator.1,
		seal: vec![
			primitives::rlp_encode(&step),
			vec![],
		],
		difficulty: 0x2000.into(),
		..Default::default()
	});
	// TODO: fn signed_header()
	let message = secp256k1::Message::parse(header.seal_hash(false).unwrap().as_fixed_bytes());
	let (signature, recovery_id) = secp256k1::sign(&message, &validator.0);
	let mut serialized_signature = [0u8; 65];
	serialized_signature[0..64].copy_from_slice(&signature.serialize());
	serialized_signature[64] = recovery_id.serialize();
	let signature = primitives::H520::from(serialized_signature);
	header.seal[1] = primitives::rlp_encode(&signature);
	header
}
