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
	BridgedBeefyValidatorSet, BridgedBlockHash, BridgedBlockNumber, Config, Error,
};

use bp_beefy::{
	beefy_merkle_root, verify_mmr_leaf_proof, BeefyMmrHash, BeefyMmrProof, MmrDataOrHash,
};
use codec::Encode;
use frame_support::ensure;
use sp_runtime::{traits::Convert, RuntimeDebug};

/// Artifacts of MMR leaf proof verification.
#[derive(RuntimeDebug)]
pub struct BeefyMmrLeafVerificationArtifacts<T: Config<I>, I: 'static> {
	/// Block number and hash of the finalized block parent.
	pub parent_number_and_hash: (BridgedBlockNumber<T, I>, BridgedBlockHash<T, I>),
	/// Next validator set, if handoff is happening.
	pub next_validators: Option<BridgedBeefyValidatorSet<T, I>>,
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
	BridgedBeefyMmrHasher<T, I>: sp_runtime::traits::Hash<Output = BeefyMmrHash>,
{
	// TODO: ensure!(mmr_leaf.leaf().version == T::MmrLeafVersion::get(), Error::<T,
	// I>::UnsupportedMmrLeafVersion); TODO: is it the right condition? can id is increased by say
	// +2?
	let is_updating_validator_set =
		mmr_leaf.leaf().beefy_next_authority_set.id == validators.id() + 1;
	ensure!(
		mmr_leaf.leaf().beefy_next_authority_set.id == validators.id() || is_updating_validator_set,
		Error::<T, I>::InvalidNextValidatorsSetId,
	);
	// technically it is not an error, but we'd like to reduce tx size on real chains
	ensure!(
		is_updating_validator_set == mmr_leaf.next_validators().is_some(),
		Error::<T, I>::RedundantNextValidatorsProvided,
	);

	// verify mmr proof for the provided leaf
	let mmr_leaf_hash =
		<BridgedBeefyMmrHasher<T, I> as bp_beefy::BeefyMmrHasher>::hash(&mmr_leaf.leaf().encode());
	verify_mmr_leaf_proof::<BridgedBeefyMmrHasher<T, I>, BridgedBeefyMmrLeaf<T, I>>(
		mmr_root,
		MmrDataOrHash::Hash(mmr_leaf_hash),
		mmr_proof,
	)
	.map_err(|e| {
		log::error!(
			target: "runtime::bridge-beefy",
			"MMR proof verification has failed with error: {:?}",
			e,
		);

		Error::<T, I>::MmrProofVeriricationFailed
	})?;

	// if new validators are provided, ensure that they match data from the leaf
	let next_validators = if let Some(ref next_validators) = mmr_leaf.next_validators() {
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
		next_validators,
	})
}
