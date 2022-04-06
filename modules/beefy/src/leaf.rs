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

//! BEEFY MMR leaf verification verification.

use crate::{
	BridgedBeefyMmrHasher, BridgedBeefyMmrLeaf, BridgedBeefyMmrLeafUnpacked,
	BridgedBeefyValidatorIdToMerkleLeaf, BridgedBeefyValidatorSet, BridgedBlockHash,
	BridgedBlockNumber, Config, Error,
};

use bp_beefy::{
	beefy_merkle_root, verify_mmr_leaf_proof, BeefyMmrHash, BeefyMmrProof, MmrDataOrHash,
	MmrLeafVersion,
};
use codec::Decode;
use frame_support::{ensure, traits::Get};
use sp_runtime::{traits::Convert, RuntimeDebug};
use sp_std::marker::PhantomData;

/// Artifacts of MMR leaf proof verification.
#[derive(RuntimeDebug)]
pub struct BeefyMmrLeafVerificationArtifacts<T: Config<I>, I: 'static> {
	/// Block number and hash of the finalized block parent.
	pub parent_number_and_hash: (BridgedBlockNumber<T, I>, BridgedBlockHash<T, I>),
	/// Next validator set, if handoff is happening.
	pub next_validator_set: Option<BridgedBeefyValidatorSet<T, I>>,
	/// Parachain heads merkle root at the imported block.
	pub parachain_heads: BeefyMmrHash,
}

/// Verify MMR proof of given leaf.
///
/// Returns new BEEFY validator set if it is enacted.
pub fn verify_beefy_mmr_leaf<T: Config<I>, I: 'static>(
	validators: &BridgedBeefyValidatorSet<T, I>,
	mmr_leaf: BridgedBeefyMmrLeafUnpacked<T, I>,
	mmr_proof: BeefyMmrProof,
	mmr_root: BeefyMmrHash,
) -> Result<BeefyMmrLeafVerificationArtifacts<T, I>, Error<T, I>>
where
	BridgedBeefyMmrHasher<T, I>: 'static + Send + Sync,
{
	// decode raw MMR leaf
	let raw_mmr_leaf = decode_raw_mmr_leaf::<T, I>(mmr_leaf.leaf())?;

	// TODO: is it the right condition? can id is increased by say +3?
	let is_updating_validator_set = raw_mmr_leaf.beefy_next_authority_set.id == validators.id() + 2;
	ensure!(
		raw_mmr_leaf.beefy_next_authority_set.id == validators.id() + 1 ||
			is_updating_validator_set,
		Error::<T, I>::InvalidNextValidatorsSetId,
	);
	// technically it is not an error, but we'd like to reduce tx size on real chains
	ensure!(
		!mmr_leaf.next_validators().is_some() || is_updating_validator_set,
		Error::<T, I>::RedundantNextValidatorsProvided,
	);
	ensure!(
		mmr_leaf.next_validators().is_some() || !is_updating_validator_set,
		Error::<T, I>::NextValidatorsAreNotProvided,
	);

	// verify mmr proof for the provided leaf
	let mmr_proof_leaf_index = mmr_proof.leaf_index;
	let mmr_proof_leaf_count = mmr_proof.leaf_count;
	let mmr_proof_length = mmr_proof.items.len();
	let mmr_leaf_hash =
		<BridgedBeefyMmrHasher<T, I> as bp_beefy::BeefyMmrHasher>::hash(mmr_leaf.leaf());
	verify_mmr_leaf_proof::<
		BridgedBeefyMmrHasherAdapter<BridgedBeefyMmrHasher<T, I>>,
		MmrDataOrHash<BridgedBeefyMmrHasherAdapter<BridgedBeefyMmrHasher<T, I>>, BridgedBeefyMmrLeaf<T, I>>,
	>(mmr_root, MmrDataOrHash::Hash(mmr_leaf_hash), mmr_proof)
	.map_err(|e| {
		log::error!(
			target: "runtime::bridge-beefy",
			"MMR proof of leaf {:?} (root: {:?} leaf: {} total: {} len: {}) verification has failed with error: {:?}",
			mmr_leaf_hash,
			mmr_root,
			mmr_proof_leaf_index,
			mmr_proof_leaf_count,
			mmr_proof_length,
			e,
		);

		Error::<T, I>::MmrProofVeriricationFailed
	})?;

	// if new validators are provided, ensure that they match data from the leaf
	let next_validator_set = if let Some(next_validators) = mmr_leaf.into_next_validators() {
		ensure!(!next_validators.is_empty(), Error::<T, I>::EmptyNextValidatorSet);

		let next_validator_addresses = next_validators
			.iter()
			.cloned()
			.map(BridgedBeefyValidatorIdToMerkleLeaf::<T, I>::convert)
			.collect::<Vec<_>>();
		let next_validator_addresses_root: BeefyMmrHash =
			beefy_merkle_root::<BridgedBeefyMmrHasher<T, I>, _, _>(next_validator_addresses).into();
		ensure!(
			next_validator_addresses_root == raw_mmr_leaf.beefy_next_authority_set.root,
			Error::<T, I>::InvalidNextValidatorSetRoot
		);

		Some(
			BridgedBeefyValidatorSet::<T, I>::new(
				next_validators,
				raw_mmr_leaf.beefy_next_authority_set.id,
			)
			.unwrap(),
		)
	} else {
		None
	};

	// TODO: ensure that ther parent_number_and_hash are actually for parent of
	// commitment.block_number

	Ok(BeefyMmrLeafVerificationArtifacts {
		parent_number_and_hash: raw_mmr_leaf.parent_number_and_hash,
		next_validator_set,
		parachain_heads: raw_mmr_leaf.parachain_heads,
	})
}

/// Decode MMR leaf of given major version.
fn decode_raw_mmr_leaf<T: Config<I>, I: 'static>(
	encoded_leaf: &[u8],
) -> Result<BridgedBeefyMmrLeaf<T, I>, Error<T, I>> {
	// decode version first, so that we know that the leaf format hasn't changed
	let version = MmrLeafVersion::decode(&mut &encoded_leaf[..]).map_err(|e| {
		// this shall never happen, because (as of now) leaf version is simple `u8`
		// and we can't fail to decode `u8`. So this is here to support potential
		// future changes
		log::error!(
			target: "runtime::bridge-beefy",
			"MMR leaf version decode has failed with error: {:?}",
			e,
		);

		Error::<T, I>::FailedToDecodeMmrLeafVersion
	})?;
	ensure!(
		version.split().0 == T::ExpectedMmrLeafMajorVersion::get(),
		Error::<T, I>::UnsupportedMmrLeafVersion
	);

	// decode the whole leaf
	BridgedBeefyMmrLeaf::<T, I>::decode(&mut &encoded_leaf[..]).map_err(|e| {
		log::error!(
			target: "runtime::bridge-beefy",
			"MMR leaf decode has failed with error: {:?}",
			e,
		);

		Error::<T, I>::FailedToDecodeMmrLeaf
	})
}

#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
struct BridgedBeefyMmrHasherAdapter<H>(PhantomData<H>);

#[cfg(feature = "std")]
impl<H> sp_std::fmt::Debug for BridgedBeefyMmrHasherAdapter<H> {
	fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		write!(f, "BridgedBeefyMmrHasherAdapter")
	}
}

impl<H> Eq for BridgedBeefyMmrHasherAdapter<H> {}

impl<H> PartialEq<BridgedBeefyMmrHasherAdapter<H>> for BridgedBeefyMmrHasherAdapter<H> {
	fn eq(&self, _: &Self) -> bool {
		true
	}
}

impl<H> Clone for BridgedBeefyMmrHasherAdapter<H> {
	fn clone(&self) -> Self {
		BridgedBeefyMmrHasherAdapter(Default::default())
	}
}

impl<H> sp_core::Hasher for BridgedBeefyMmrHasherAdapter<H>
where
	H: beefy_merkle_tree::Hasher + Send + Sync,
{
	type Out = BeefyMmrHash;
	type StdHasher = hash256_std_hasher::Hash256StdHasher;
	const LENGTH: usize = 32;

	fn hash(s: &[u8]) -> Self::Out {
		H::hash(s)
	}
}

impl<H> sp_runtime::traits::Hash for BridgedBeefyMmrHasherAdapter<H>
where
	H: 'static + beefy_merkle_tree::Hasher + Send + Sync,
{
	type Output = BeefyMmrHash;

	fn ordered_trie_root(
		_input: Vec<Vec<u8>>,
		_state_version: sp_runtime::StateVersion,
	) -> Self::Output {
		unreachable!("MMR never needs trie root functions; qed")
	}

	fn trie_root(
		_input: Vec<(Vec<u8>, Vec<u8>)>,
		_state_version: sp_runtime::StateVersion,
	) -> Self::Output {
		unreachable!("MMR never needs trie root functions; qed")
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{mock::*, mock_chain::*, *};
	use codec::Encode;
	use frame_support::assert_noop;

	#[test]
	fn fails_to_import_commitment_if_leaf_version_is_unexpected() {
		run_test_with_initialize(1, || {
			// let's change leaf version to something lesser than expected
			let commitment = ChainBuilder::new(1)
				.custom_header()
				.customize_leaf(|leaf| {
					let mut raw_leaf =
						BridgedRawMmrLeaf::decode(&mut &leaf.leaf()[..]).unwrap();
					raw_leaf.version = MmrLeafVersion::new(EXPECTED_MMR_LEAF_MAJOR_VERSION - 1, 0);
					leaf.set_leaf(raw_leaf.encode())
				})
				.finalize()
				.to_header();

			assert_noop!(
				import_commitment(commitment),
				Error::<TestRuntime, ()>::UnsupportedMmrLeafVersion,
			);
		});
	}

	#[test]
	fn fails_to_import_commitment_if_leaf_decode_fails() {
		run_test_with_initialize(1, || {
			// let's leave leaf version, but replace other leaf data with something that can't be
			// decoded
			let commitment = ChainBuilder::new(1)
				.custom_header()
				.customize_leaf(|leaf| {
					let mut raw_leaf =
						MmrLeafVersion::new(EXPECTED_MMR_LEAF_MAJOR_VERSION, 0).encode();
					raw_leaf.push(42);
					leaf.set_leaf(raw_leaf)
				})
				.finalize()
				.to_header();

			assert_noop!(
				import_commitment(commitment),
				Error::<TestRuntime, ()>::FailedToDecodeMmrLeaf,
			);
		});
	}

	#[test]
	fn fails_to_import_commitment_if_signed_by_wrong_validator_set_id() {
		run_test_with_initialize(1, || {
			// let's change next validator set id, so that it won't match next
			// validator set id and new valdiator set id
			let commitment = ChainBuilder::new(1)
				.custom_header()
				.customize_leaf(|leaf| {
					let mut raw_leaf =
						BridgedRawMmrLeaf::decode(&mut &leaf.leaf()[..]).unwrap();
					raw_leaf.beefy_next_authority_set.id += 10;
					leaf.set_leaf(raw_leaf.encode())
				})
				.finalize()
				.to_header();

			assert_noop!(
				import_commitment(commitment),
				Error::<TestRuntime, ()>::InvalidNextValidatorsSetId,
			);
		});
	}

	#[test]
	fn fails_to_import_commitment_if_leaf_provides_redundant_new_validator_set() {
		run_test_with_initialize(1, || {
			// let's change leaf so that signals handoff where handoff is not happening
			let commitment = ChainBuilder::new(1)
				.custom_header()
				.customize_leaf(|leaf| leaf.set_next_validators(Some(Vec::new())))
				.finalize()
				.to_header();

			assert_noop!(
				import_commitment(commitment),
				Error::<TestRuntime, ()>::RedundantNextValidatorsProvided,
			);
		});
	}

	#[test]
	fn fails_to_import_commitment_if_new_validator_set_is_not_provided() {
		run_test_with_initialize(1, || {
			// let's change leaf so that it should provide new validator set, but it does not
			let commitment = ChainBuilder::new(1)
				.custom_header()
				.customize_leaf(|leaf| {
					let mut raw_leaf =
						BridgedRawMmrLeaf::decode(&mut &leaf.leaf()[..]).unwrap();
					raw_leaf.beefy_next_authority_set.id += 1;
					leaf.set_leaf(raw_leaf.encode())
				})
				.finalize()
				.to_header();

			assert_noop!(
				import_commitment(commitment),
				Error::<TestRuntime, ()>::NextValidatorsAreNotProvided,
			);
		});
	}

	#[test]
	fn fails_to_import_commitment_if_mmr_proof_is_wrong() {
		run_test_with_initialize(1, || {
			// let's change proof so that its verification fails
			let commitment = ChainBuilder::new(1)
				.custom_header()
				.customize_proof(|mut proof| {
					proof.leaf_index += 1;
					proof
				})
				.finalize()
				.to_header();

			assert_noop!(
				import_commitment(commitment),
				Error::<TestRuntime, ()>::MmrProofVeriricationFailed,
			);
		});
	}

	#[test]
	fn fails_to_import_commitment_if_new_validator_set_is_empty() {
		run_test_with_initialize(1, || {
			// let's change leaf so that it handoffs to empty validator set
			let commitment = ChainBuilder::new(1)
				.custom_handoff_header(1)
				.customize_leaf(|leaf| leaf.set_next_validators(Some(Vec::new())))
				.finalize()
				.to_header();

			assert_noop!(
				import_commitment(commitment),
				Error::<TestRuntime, ()>::EmptyNextValidatorSet,
			);
		});
	}

	#[test]
	fn fails_to_import_commitment_if_validators_merkle_root_mismatch() {
		run_test_with_initialize(1, || {
			// let's change leaf so that merkle root of new validators is wrong
			let commitment = ChainBuilder::new(1)
				.custom_handoff_header(1)
				.customize_leaf(|leaf| {
					let mut raw_leaf =
						BridgedRawMmrLeaf::decode(&mut &leaf.leaf()[..]).unwrap();
					raw_leaf.beefy_next_authority_set.root = Default::default();
					leaf.set_leaf(raw_leaf.encode())
				})
				.finalize()
				.to_header();

			assert_noop!(
				import_commitment(commitment),
				Error::<TestRuntime, ()>::InvalidNextValidatorSetRoot,
			);
		});
	}
}
