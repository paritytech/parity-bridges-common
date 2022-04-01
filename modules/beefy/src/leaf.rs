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

//use crate::{BeefyMmrHasher, BeefyMmrLeaf, BeefyValidatorSet, Config, Error};

use crate::{
	BridgedBeefyMmrHasher, BridgedBeefyMmrLeaf, BridgedBeefyValidatorIdToMerkleLeaf,
	BridgedBeefyValidatorSet, BridgedBlockHash, BridgedBlockNumber, BridgedRawBeefyMmrLeaf, Config,
	Error,
};

use bp_beefy::{
	beefy_merkle_root, verify_mmr_leaf_proof, BeefyMmrHash, BeefyMmrProof, MmrDataOrHash,
};
use codec::Encode;
use frame_support::ensure;
use scale_info::TypeInfo;
use sp_runtime::{traits::Convert, RuntimeDebug};
use sp_std::marker::PhantomData;

/// Artifacts of MMR leaf proof verification.
#[derive(RuntimeDebug)]
pub struct BeefyMmrLeafVerificationArtifacts<T: Config<I>, I: 'static> {
	/// Block number and hash of the finalized block parent.
	pub parent_number_and_hash: (BridgedBlockNumber<T, I>, BridgedBlockHash<T, I>),
	/// Next validator set, if handoff is happening.
	pub next_validator_set: Option<BridgedBeefyValidatorSet<T, I>>,
}

/// Verify MMR proof of given leaf.
///
/// Returns new BEEFY validator set if it is enacted.
pub fn verify_beefy_mmr_leaf<T: Config<I>, I: 'static>(
	validators: &BridgedBeefyValidatorSet<T, I>,
	mmr_leaf: &BridgedBeefyMmrLeaf<T, I>,
	mmr_proof: BeefyMmrProof,
	mmr_root: BeefyMmrHash,
) -> Result<BeefyMmrLeafVerificationArtifacts<T, I>, Error<T, I>>
where
	BridgedBeefyMmrHasher<T, I>: 'static + Send + Sync,
{
	// TODO: ensure!(mmr_leaf.leaf().version == T::MmrLeafVersion::get(), Error::<T,
	// I>::UnsupportedMmrLeafVersion);

	// TODO: is it the right condition? can id is increased by say +3?
	let is_updating_validator_set =
		mmr_leaf.leaf().beefy_next_authority_set.id == validators.id() + 2;
	ensure!(
		mmr_leaf.leaf().beefy_next_authority_set.id == validators.id() + 1 ||
			is_updating_validator_set,
		Error::<T, I>::InvalidNextValidatorsSetId,
	);
	// technically it is not an error, but we'd like to reduce tx size on real chains
	ensure!(
		is_updating_validator_set == mmr_leaf.next_validators().is_some(),
		Error::<T, I>::RedundantNextValidatorsProvided,
	);

	// verify mmr proof for the provided leaf
	let mmr_proof_leaf_index = mmr_proof.leaf_index;
	let mmr_proof_leaf_count = mmr_proof.leaf_count;
	let mmr_proof_length = mmr_proof.items.len();
	let mmr_leaf_hash =
		<BridgedBeefyMmrHasher<T, I> as bp_beefy::BeefyMmrHasher>::hash(&mmr_leaf.leaf().encode());
	verify_mmr_leaf_proof::<
		BridgedBeefyMmrHasherAdapter<BridgedBeefyMmrHasher<T, I>>,
		MmrDataOrHash<BridgedBeefyMmrHasherAdapter<BridgedBeefyMmrHasher<T, I>>, BridgedRawBeefyMmrLeaf<T, I>>,
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
	let next_validator_set = if let Some(ref next_validators) = mmr_leaf.next_validators() {
		ensure!(!next_validators.is_empty(), Error::<T, I>::EmptyNextValidatorSet);

		let next_validator_addresses = next_validators
			.iter()
			.cloned()
			.map(BridgedBeefyValidatorIdToMerkleLeaf::<T, I>::convert)
			.collect::<Vec<_>>();
		let next_validator_addresses_root: BeefyMmrHash =
			beefy_merkle_root::<BridgedBeefyMmrHasher<T, I>, _, _>(next_validator_addresses).into();
		ensure!(
			next_validator_addresses_root == mmr_leaf.leaf().beefy_next_authority_set.root,
			Error::<T, I>::InvalidNextValidatorSetRoot
		);

		// TODO: avoid clone?
		Some(
			BridgedBeefyValidatorSet::<T, I>::new(
				next_validators.iter().cloned(),
				mmr_leaf.leaf().beefy_next_authority_set.id,
			)
			.expect("TODO"),
		)
	} else {
		None
	};

	// TODO: ensure that ther parent_number_and_hash are actually for parent of
	// commitment.block_number

	Ok(BeefyMmrLeafVerificationArtifacts {
		parent_number_and_hash: mmr_leaf.leaf().parent_number_and_hash,
		next_validator_set,
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
		input: Vec<Vec<u8>>,
		state_version: sp_runtime::StateVersion,
	) -> Self::Output {
		unreachable!("TODO: do we need this?")
	}

	fn trie_root(
		input: Vec<(Vec<u8>, Vec<u8>)>,
		state_version: sp_runtime::StateVersion,
	) -> Self::Output {
		unreachable!("TODO: do we need this?")
	}
}
