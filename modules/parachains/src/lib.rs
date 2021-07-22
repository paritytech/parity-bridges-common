// Copyright 2021 Parity Technologies (UK) Ltd.
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

//! Parachains finality module.
//!
//! This module needs to be deployed with GRANDPA module, which is syncing relay
//! chain blocks. The main entry point of this module is `submit_parachain_heads`, which
//! accepts storage proof of some parachain `Heads` entries from bridged relay chain.
//! It requires corresponding relay headers to be already synced.

#![cfg_attr(not(feature = "std"), no_std)]

use bp_runtime::{BlockNumberOf, HashOf, HasherOf};
use codec::{Decode, Encode};
use frame_support::RuntimeDebug;
use sp_runtime::traits::Header as HeaderT;

// Re-export in crate namespace for `construct_runtime!`.
pub use pallet::*;

#[cfg(test)]
mod mock;

/// Block hash of the bridged relay chain.
pub type RelayBlockHash<T> = HashOf<<T as pallet_bridge_grandpa::Config>::BridgedChain>;
/// Block number of the bridged relay chain.
pub type RelayBlockNumber<T> = BlockNumberOf<<T as pallet_bridge_grandpa::Config>::BridgedChain>;
/// Hasher of the bridged relay chain.
pub type RelayBlockHasher<T> = HasherOf<<T as pallet_bridge_grandpa::Config>::BridgedChain>;

/// Parachain id.
pub type ParaId = u32;

/// Parachain head, which is and encoded parachain header.
pub type ParaHead = Vec<u8>;

/// Parachain heads storage proof.
pub type ParachainHeadsProof = Vec<Vec<u8>>;

/// Parachain head as it is stored in the runtime storage.
#[derive(Decode, Encode, PartialEq, RuntimeDebug)]
pub struct StoredParaHead<RelayBlockNumber> {
	/// Number of relay block where this head has been updated.
	pub at_relay_block_number: RelayBlockNumber,
	/// Parachain head.
	pub head: ParaHead,
}

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use super::*;

	#[pallet::error]
	pub enum Error<T> {
		/// Relay chain block is unknown to us.
		UnknownRelayChainBlock,
		/// Invalid storage proof has been passed.
		InvalidStorageProof,
	}

	#[pallet::config]
	#[pallet::disable_frame_system_supertrait_check]
	pub trait Config: pallet_bridge_grandpa::Config {
	}

	/// Parachain heads.
	#[pallet::storage]
	pub type ParaHeads<T: Config> = StorageMap<_, Identity, ParaId, StoredParaHead<RelayBlockNumber<T>>>;

	#[pallet::pallet]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Submit proof of one or several parachain heads.
		///
		/// The proof is supposed to be proof of some `Heads` entries from the
		/// `polkadot-runtime-parachains::paras` pallet instance, deployed at the bridged chain.
		/// The proof is supposed to be crafted at the `relay_header_hash` that must already be
		/// imported by corresponding GRANDPA pallet at this chain.
		#[pallet::weight(0)] // TODO
		pub fn submit_parachain_heads(
			_origin: OriginFor<T>,
			relay_block_hash: RelayBlockHash<T>,
			parachains: Vec<ParaId>,
			parachain_heads_proof: ParachainHeadsProof,
		) -> DispatchResult {
			// we'll need relay chain header to verify that parachains heads are always increasing.
			let relay_block = pallet_bridge_grandpa::ImportedHeaders::<T>::get(relay_block_hash)
				.ok_or(Error::<T>::UnknownRelayChainBlock)?;
			let relay_block_number = *relay_block.number();

			// now parse storage proof and read parachain heads
			pallet_bridge_grandpa::Pallet::<T>::parse_finalized_storage_proof(
				relay_block_hash,
				sp_trie::StorageProof::new(parachain_heads_proof),
				move |storage| {
					for parachain in parachains {
						let parachain_head = match read_parachain_head::<T>(&storage, parachain) {
							Some(parachain_head) => parachain_head,
							None => {
								log::trace!(
									target: "runtime::bridge-parachains",
									"The head of parachain {} has been declared, but is missing from the proof",
									parachain,
								);
								continue
							},
						};

						let _ = ParaHeads::<T>::try_mutate(parachain, |parachain_head_data| {
							match parachain_head_data {
								Some(stored_data) if stored_data.at_relay_block_number <= relay_block_number => (),
								None => (),
								Some(stored_data) => {
									log::trace!(
										target: "runtime::bridge-parachains",
										"The head of parachain {} can't be updated, because it has been already updated\
										at better relay chain block: {} > {}",
										parachain,
										stored_data.at_relay_block_number,
										relay_block_number,
									);
									return Err(());
								},
							}

							log::trace!(
								target: "runtime::bridge-parachains",
								"The head of parachain {} has been updated at relay block {}",
								parachain,
								relay_block_number,
							);

							*parachain_head_data = Some(StoredParaHead {
								at_relay_block_number: relay_block_number,
								head: parachain_head,
							});
							Ok(())
						});
					}
				},
			).map_err(|_| Error::<T>::InvalidStorageProof)?;

			// TODO: save previous heads??? pruning???

			Ok(().into())
		}
	}
}

pub mod storage_keys {
	use super::*;
	use bp_runtime::storage_keys::storage_map_final_key;
	use sp_core::storage::StorageKey;

	/// Storage key of the parachain head in the runtime storage of relay chain.
	pub fn parachain_head_key(parachain: ParaId) -> StorageKey {
		storage_map_final_key("Heads", &parachain.encode())
	}
}

/// Read parachain head from storage proof.
fn read_parachain_head<T: Config>(
	storage: &bp_runtime::StorageProofChecker<RelayBlockHasher<T>>,
	parachain: ParaId,
) -> Option<ParaHead> {
	let parachain_head_key = storage_keys::parachain_head_key(parachain);
	let parachain_head = storage.read_value(parachain_head_key.0.as_ref()).ok()?;
	parachain_head
}

#[cfg(test)]
mod tests {
	use crate::mock::{Origin, TestHash, TestHasher, TestNumber, TestRuntime, run_test, test_header};
	use super::*;

	use bp_test_utils::authority_list;
	use frame_support::assert_ok;
	use sp_trie::{record_all_keys, trie_types::TrieDBMut, Layout, MemoryDB, Recorder, TrieMut};

	fn initialize(state_root: TestHash) {
		pallet_bridge_grandpa::Pallet::<TestRuntime>::initialize(
			Origin::root(),
			bp_header_chain::InitializationData {
				header: test_header(0, state_root),
				authority_list: authority_list(),
				set_id: 1,
				is_halted: false,
			},
		).unwrap();
	}

	fn prepare_parachain_heads_proof(heads: Vec<(ParaId, ParaId)>) -> (TestHash, ParachainHeadsProof) {
		let mut root = Default::default();
		let mut mdb = MemoryDB::default();
		{
			let mut trie = TrieDBMut::<TestHasher>::new(&mut mdb, &mut root);
			for (parachain, head) in heads {
				let storage_key = storage_keys::parachain_head_key(parachain);
				trie.insert(&storage_key.0, &head.encode())
					.map_err(|_| "TrieMut::insert has failed")
					.expect("TrieMut::insert should not fail in tests");
			}
		}

		// generate storage proof to be delivered to This chain
		let mut proof_recorder = Recorder::<TestHash>::new();
		record_all_keys::<Layout<TestHasher>, _>(&mdb, &root, &mut proof_recorder)
			.map_err(|_| "record_all_keys has failed")
			.expect("record_all_keys should not fail in benchmarks");
		let storage_proof = proof_recorder.drain().into_iter().map(|n| n.data.to_vec()).collect();

		(root, storage_proof)
	}

	fn stored_head(parachain: ParaId) -> StoredParaHead<TestNumber> {
		StoredParaHead {
			at_relay_block_number: 0,
			head: parachain.encode(),
		}
	}

	#[test]
	fn imports_parachain_heads() {
		let (state_root, proof) = prepare_parachain_heads_proof(vec![
			(1, 1),
		]);
		run_test(|| {
			initialize(state_root);
			assert_ok!(
				Pallet::<TestRuntime>::submit_parachain_heads(
					Origin::signed(1),
					test_header(0, state_root).hash(),
					vec![1, 2],
					proof,
				),
			);

			assert_eq!(ParaHeads::<TestRuntime>::get(1), Some(stored_head(1)));
			assert_eq!(ParaHeads::<TestRuntime>::get(2), None);
		});
	}
}
