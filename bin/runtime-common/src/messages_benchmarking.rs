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

//! Everything required to run benchmarks of messages module, based on
//! `bridge_runtime_common::messages` implementation.

#![cfg(feature = "runtime-benchmarks")]

use crate::{
	messages::{
		source::FromBridgedChainMessagesDeliveryProof, target::FromBridgedChainMessagesProof,
		AccountIdOf, BridgedChain, HashOf, HasherOf, MessageBridge, ThisChain,
	},
	messages_generation::{
		encode_all_messages, encode_lane_data, grow_trie, prepare_messages_storage_proof,
	},
};

use bp_messages::storage_keys;
use bp_polkadot_core::parachains::ParaHash;
use bp_runtime::{
	record_all_trie_keys, Chain, Parachain, RawStorageProof, StorageProofSize, UnderlyingChainOf,
};
use codec::Encode;
use frame_support::weights::Weight;
use pallet_bridge_messages::benchmarking::{MessageDeliveryProofParams, MessageProofParams};
use sp_runtime::traits::{Header, Zero};
use sp_std::prelude::*;
use sp_trie::{trie_types::TrieDBMutBuilderV1, LayoutV1, MemoryDB, TrieMut};

/// Prepare proof of messages for the `receive_messages_proof` call.
///
/// In addition to returning valid messages proof, environment is prepared to verify this message
/// proof.
///
/// This method is intended to be used when benchmarking pallet, linked to the chain that
/// uses GRANDPA finality. For parachains, please use the `prepare_message_proof_from_parachain`
/// function.
pub fn prepare_message_proof_from_grandpa_chain<R, FI, B>(
	params: MessageProofParams,
) -> (FromBridgedChainMessagesProof<HashOf<BridgedChain<B>>>, Weight)
where
	R: pallet_bridge_grandpa::Config<FI, BridgedChain = UnderlyingChainOf<BridgedChain<B>>>,
	FI: 'static,
	B: MessageBridge,
{
	// prepare storage proof
	let (state_root, storage_proof) = prepare_messages_storage_proof::<B>(
		params.lane,
		params.message_nonces.clone(),
		params.outbound_lane_data,
		params.size,
		match params.size {
			StorageProofSize::Minimal(ref size) => vec![0u8; *size as _],
			_ => vec![],
		},
		encode_all_messages,
		encode_lane_data,
	);

	// update runtime storage
	let (_, bridged_header_hash) = insert_header_to_grandpa_pallet::<R, FI>(state_root);

	(
		FromBridgedChainMessagesProof {
			bridged_header_hash,
			storage_proof,
			lane: params.lane,
			nonces_start: *params.message_nonces.start(),
			nonces_end: *params.message_nonces.end(),
		},
		Weight::zero(),
	)
}

/// Prepare proof of messages for the `receive_messages_proof` call.
///
/// In addition to returning valid messages proof, environment is prepared to verify this message
/// proof.
///
/// This method is intended to be used when benchmarking pallet, linked to the chain that
/// uses parachain finality. For GRANDPA chains, please use the
/// `prepare_message_proof_from_grandpa_chain` function.
pub fn prepare_message_proof_from_parachain<R, PI, B>(
	params: MessageProofParams,
) -> (FromBridgedChainMessagesProof<HashOf<BridgedChain<B>>>, Weight)
where
	R: pallet_bridge_parachains::Config<PI>,
	PI: 'static,
	B: MessageBridge,
	UnderlyingChainOf<BridgedChain<B>>: Chain<Hash = ParaHash> + Parachain,
{
	// prepare storage proof
	let (state_root, storage_proof) = prepare_messages_storage_proof::<B>(
		params.lane,
		params.message_nonces.clone(),
		params.outbound_lane_data,
		params.size,
		match params.size {
			StorageProofSize::Minimal(ref size) => vec![0u8; *size as _],
			_ => vec![],
		},
		encode_all_messages,
		encode_lane_data,
	);

	// update runtime storage
	let (_, bridged_header_hash) =
		insert_header_to_parachains_pallet::<R, PI, UnderlyingChainOf<BridgedChain<B>>>(state_root);

	(
		FromBridgedChainMessagesProof {
			bridged_header_hash,
			storage_proof,
			lane: params.lane,
			nonces_start: *params.message_nonces.start(),
			nonces_end: *params.message_nonces.end(),
		},
		Weight::zero(),
	)
}

/// Prepare proof of messages delivery for the `receive_messages_delivery_proof` call.
///
/// This method is intended to be used when benchmarking pallet, linked to the chain that
/// uses GRANDPA finality. For parachains, please use the
/// `prepare_message_delivery_proof_from_parachain` function.
pub fn prepare_message_delivery_proof_from_grandpa_chain<R, FI, B>(
	params: MessageDeliveryProofParams<AccountIdOf<ThisChain<B>>>,
) -> FromBridgedChainMessagesDeliveryProof<HashOf<BridgedChain<B>>>
where
	R: pallet_bridge_grandpa::Config<FI, BridgedChain = UnderlyingChainOf<BridgedChain<B>>>,
	FI: 'static,
	B: MessageBridge,
{
	// prepare storage proof
	let lane = params.lane;
	let (state_root, storage_proof) = prepare_message_delivery_proof::<B>(params);

	// update runtime storage
	let (_, bridged_header_hash) = insert_header_to_grandpa_pallet::<R, FI>(state_root);

	FromBridgedChainMessagesDeliveryProof {
		bridged_header_hash: bridged_header_hash.into(),
		storage_proof,
		lane,
	}
}

/// Prepare proof of messages delivery for the `receive_messages_delivery_proof` call.
///
/// This method is intended to be used when benchmarking pallet, linked to the chain that
/// uses parachain finality. For GRANDPA chains, please use the
/// `prepare_message_delivery_proof_from_grandpa_chain` function.
pub fn prepare_message_delivery_proof_from_parachain<R, PI, B>(
	params: MessageDeliveryProofParams<AccountIdOf<ThisChain<B>>>,
) -> FromBridgedChainMessagesDeliveryProof<HashOf<BridgedChain<B>>>
where
	R: pallet_bridge_parachains::Config<PI>,
	PI: 'static,
	B: MessageBridge,
	UnderlyingChainOf<BridgedChain<B>>: Chain<Hash = ParaHash> + Parachain,
{
	// prepare storage proof
	let lane = params.lane;
	let (state_root, storage_proof) = prepare_message_delivery_proof::<B>(params);

	// update runtime storage
	let (_, bridged_header_hash) =
		insert_header_to_parachains_pallet::<R, PI, UnderlyingChainOf<BridgedChain<B>>>(state_root);

	FromBridgedChainMessagesDeliveryProof {
		bridged_header_hash: bridged_header_hash.into(),
		storage_proof,
		lane,
	}
}

/// Prepare in-memory message delivery proof, without inserting anything to the runtime storage.
fn prepare_message_delivery_proof<B>(
	params: MessageDeliveryProofParams<AccountIdOf<ThisChain<B>>>,
) -> (HashOf<BridgedChain<B>>, RawStorageProof)
where
	B: MessageBridge,
{
	// prepare Bridged chain storage with inbound lane state
	let storage_key =
		storage_keys::inbound_lane_data_key(B::BRIDGED_MESSAGES_PALLET_NAME, &params.lane).0;
	let mut root = Default::default();
	let mut mdb = MemoryDB::default();
	{
		let mut trie =
			TrieDBMutBuilderV1::<HasherOf<BridgedChain<B>>>::new(&mut mdb, &mut root).build();
		trie.insert(&storage_key, &params.inbound_lane_data.encode())
			.map_err(|_| "TrieMut::insert has failed")
			.expect("TrieMut::insert should not fail in benchmarks");
	}
	root = grow_trie(root, &mut mdb, params.size);

	// generate storage proof to be delivered to This chain
	let storage_proof = record_all_trie_keys::<LayoutV1<HasherOf<BridgedChain<B>>>, _>(&mdb, &root)
		.map_err(|_| "record_all_trie_keys has failed")
		.expect("record_all_trie_keys should not fail in benchmarks");

	(root, storage_proof)
}

/// Insert header to the bridge GRANDPA pallet.
pub(crate) fn insert_header_to_grandpa_pallet<R, GI>(
	state_root: bp_runtime::HashOf<R::BridgedChain>,
) -> (bp_runtime::BlockNumberOf<R::BridgedChain>, bp_runtime::HashOf<R::BridgedChain>)
where
	R: pallet_bridge_grandpa::Config<GI>,
	GI: 'static,
	R::BridgedChain: bp_runtime::Chain,
{
	let bridged_block_number = Zero::zero();
	let bridged_header = bp_runtime::HeaderOf::<R::BridgedChain>::new(
		bridged_block_number,
		Default::default(),
		state_root,
		Default::default(),
		Default::default(),
	);
	let bridged_header_hash = bridged_header.hash();
	pallet_bridge_grandpa::initialize_for_benchmarks::<R, GI>(bridged_header);
	(bridged_block_number, bridged_header_hash)
}

/// Insert header to the bridge parachains pallet.
pub(crate) fn insert_header_to_parachains_pallet<R, PI, PC>(
	state_root: bp_runtime::HashOf<PC>,
) -> (bp_runtime::BlockNumberOf<PC>, bp_runtime::HashOf<PC>)
where
	R: pallet_bridge_parachains::Config<PI>,
	PI: 'static,
	PC: Chain<Hash = ParaHash> + Parachain,
{
	let bridged_block_number = Zero::zero();
	let bridged_header = bp_runtime::HeaderOf::<PC>::new(
		bridged_block_number,
		Default::default(),
		state_root,
		Default::default(),
		Default::default(),
	);
	let bridged_header_hash = bridged_header.hash();
	pallet_bridge_parachains::initialize_for_benchmarks::<R, PI, PC>(bridged_header);
	(bridged_block_number, bridged_header_hash)
}
