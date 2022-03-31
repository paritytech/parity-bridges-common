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

//! BEEFY commitment verification.

use crate::{
	BridgedBeefyCommitmentHasher, BridgedBeefySignedCommitment, BridgedBeefyValidatorSet,
	BridgedBlockNumber, Config, Error,
};

use bp_beefy::BeefyMmrHash;
use codec::Encode;
use frame_support::ensure;
use sp_runtime::{traits::Hash, RuntimeAppPublic, RuntimeDebug};

/// Artifacts of BEEFY commitment verification.
#[derive(RuntimeDebug)]
pub struct CommitmentVerificationArtifacts<BlockNumber> {
	/// Finalized block number.
	pub finalized_block_number: BlockNumber,
	/// MMR root at the finalized block.
	pub mmr_root: BeefyMmrHash,
}

/// Verify that the commitment is valid and signed by the current validator set.
///
/// Returns MMR root, extracted from commitment payload.
pub fn verify_beefy_signed_commitment<T: Config<I>, I: 'static>(
	best_block_number: BridgedBlockNumber<T, I>,
	validators: &BridgedBeefyValidatorSet<T, I>,
	commitment: &BridgedBeefySignedCommitment<T, I>,
) -> Result<CommitmentVerificationArtifacts<BridgedBlockNumber<T, I>>, Error<T, I>> {
	// ensure that the commitment is signed by the best known BEEFY validators set
	ensure!(
		commitment.commitment.validator_set_id == validators.id(),
		Error::<T, I>::InvalidValidatorsSetId
	);
	ensure!(
		commitment.signatures.len() == validators.len(),
		Error::<T, I>::InvalidSignaturesLength
	);

	// ensure that the commitment is for the better block that we know of
	ensure!(commitment.commitment.block_number > best_block_number, Error::<T, I>::OldCommitment);

	// ensure that the enough validators have signed on commitment
	let commitment_hash =
		BridgedBeefyCommitmentHasher::<T, I>::hash(&commitment.commitment.encode());
	let correct_signatures_required = signatures_required(validators.len());
	let mut correct_signatures = 0;
	for (validator_index, signature) in commitment.signatures.iter().enumerate() {
		if let Some(signature) = signature {
			let validator_public = &validators.validators()[validator_index];
			// TODO: this is not correct - verify is hashing `commitment_hash` itself with
			// blake2_256 we shall use `verify_prehashed` instead. The problem is that the
			// `RuntimeAppPublic` doesn't have this method :( So we need to change to something like
			// `RiuntimeAppPublicWithVerifyPrehashed` and impl it at least for ECDSA
			if validator_public.verify(&commitment_hash, signature) {
				correct_signatures += 1;
				if correct_signatures >= correct_signatures_required {
					break
				}
			} else {
				log::debug!(
					target: "runtime::bridge-beefy",
					"Signed commitment contains incorrect signature of validator {} ({:?}): {:?}",
					validator_index,
					validator_public,
					signature,
				);
			}
		}
	}
	ensure!(
		correct_signatures >= correct_signatures_required,
		Error::<T, I>::NotEnoughCorrectSignatures
	);

	extract_mmr_root(commitment).map(|mmr_root| CommitmentVerificationArtifacts {
		finalized_block_number: commitment.commitment.block_number,
		mmr_root,
	})
}

/// Number of correct signatures, required from given validators set to accept signed commitment.
///
/// We're using 'conservative' approach here, where signatures of `2/3+1` validators are required..
pub(crate) fn signatures_required(validators_len: usize) -> usize {
	validators_len - validators_len.saturating_sub(1) / 3
}

/// Extract MMR root from commitment payload.
fn extract_mmr_root<T: Config<I>, I: 'static>(
	commitment: &BridgedBeefySignedCommitment<T, I>,
) -> Result<BeefyMmrHash, Error<T, I>> {
	commitment
		.commitment
		.payload
		.get_decoded(&bp_beefy::MMR_ROOT_PAYLOAD_ID)
		.ok_or(Error::MmrRootMissingFromCommitment)
}
