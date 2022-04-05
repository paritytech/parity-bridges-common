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

use bp_beefy::{BeefyMmrHash, BeefyRuntimeAppPublic};
use codec::Encode;
use frame_support::ensure;
use sp_runtime::{traits::Hash, RuntimeDebug};

/// Artifacts of BEEFY commitment verification.
#[derive(RuntimeDebug, PartialEq)]
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
		Error::<T, I>::InvalidValidatorSetId
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
			if validator_public.verify_prehashed(signature, &commitment_hash) {
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

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{mock::*, mock_chain::*, *};
	use bp_beefy::{BeefyPayload, Commitment, MMR_ROOT_PAYLOAD_ID};
	use frame_support::assert_noop;

	#[test]
	fn fails_to_import_commitment_if_signed_by_unexpected_validator_set() {
		run_test_with_initialize(1, || {
			// when `validator_set_id` is different from what's stored in the runtime
			let mut commitment: HeaderAndCommitment =
				ChainBuilder::new(1).append_finalized_header().into();
			commitment.commitment.as_mut().unwrap().commitment.validator_set_id += 1;

			assert_noop!(
				import_commitment(commitment),
				Error::<TestRuntime, ()>::InvalidValidatorSetId,
			);
		});
	}

	#[test]
	fn fails_to_import_commitment_if_number_of_signatures_is_invalid() {
		run_test_with_initialize(8, || {
			// when additional signature is provided
			let mut commitment: HeaderAndCommitment =
				ChainBuilder::new(1).append_finalized_header().into();
			commitment.commitment.as_mut().unwrap().signatures.push(Default::default());

			assert_noop!(
				import_commitment(commitment.clone()),
				Error::<TestRuntime, ()>::InvalidSignaturesLength,
			);

			// when there's lack of signatures
			commitment.commitment.as_mut().unwrap().signatures.pop();
			commitment.commitment.as_mut().unwrap().signatures.pop();

			assert_noop!(
				import_commitment(commitment),
				Error::<TestRuntime, ()>::InvalidSignaturesLength,
			);
		});
	}

	#[test]
	fn fails_to_import_commitment_if_it_does_not_improve_best_block() {
		run_test_with_initialize(1, || {
			BestBlockNumber::<TestRuntime>::put(10);

			// when commitment is for the same block
			let mut commitment: HeaderAndCommitment =
				ChainBuilder::new(1).append_finalized_header().into();
			commitment.commitment.as_mut().unwrap().commitment.block_number = 10;

			assert_noop!(
				import_commitment(commitment.clone()),
				Error::<TestRuntime, ()>::OldCommitment,
			);

			// when commitment is for the ancestor of best block
			commitment.commitment.as_mut().unwrap().commitment.block_number = 5;

			assert_noop!(import_commitment(commitment), Error::<TestRuntime, ()>::OldCommitment,);
		});
	}

	#[test]
	fn fails_to_import_commitment_if_it_has_no_enough_valid_signatures() {
		run_test_with_initialize(1, || {
			// invalidate single signature
			let mut commitment: HeaderAndCommitment =
				ChainBuilder::new(1).append_finalized_header().into();
			*commitment
				.commitment
				.as_mut()
				.unwrap()
				.signatures
				.iter_mut()
				.find(|s| s.is_some())
				.unwrap() = Default::default();

			assert_noop!(
				import_commitment(commitment),
				Error::<TestRuntime, ()>::NotEnoughCorrectSignatures,
			);
		});
	}

	#[test]
	fn fails_to_import_commitment_if_there_is_no_mmr_root_in_the_payload() {
		run_test_with_initialize(1, || {
			// remove MMR root from the payload
			let mut commitment: HeaderAndCommitment =
				ChainBuilder::new(1).append_finalized_header().into();
			commitment.commitment = Some(sign_commitment(
				Commitment {
					payload: BeefyPayload::new(*b"xy", vec![]),
					block_number: commitment.commitment.as_ref().unwrap().commitment.block_number,
					validator_set_id: commitment
						.commitment
						.as_ref()
						.unwrap()
						.commitment
						.validator_set_id,
				},
				&validator_keys(0, 1),
			));

			assert_noop!(
				import_commitment(commitment),
				Error::<TestRuntime, ()>::MmrRootMissingFromCommitment,
			);
		});
	}

	#[test]
	fn fails_to_import_commitment_if_mmr_root_decode_fails() {
		run_test_with_initialize(1, || {
			// MMR root is a 32-byte array and we have replaced it with single byte
			let mut commitment: HeaderAndCommitment =
				ChainBuilder::new(1).append_finalized_header().into();
			commitment.commitment = Some(sign_commitment(
				Commitment {
					payload: BeefyPayload::new(MMR_ROOT_PAYLOAD_ID, vec![42]),
					block_number: commitment.commitment.as_ref().unwrap().commitment.block_number,
					validator_set_id: commitment
						.commitment
						.as_ref()
						.unwrap()
						.commitment
						.validator_set_id,
				},
				&validator_keys(0, 1),
			));

			assert_noop!(
				import_commitment(commitment),
				Error::<TestRuntime, ()>::MmrRootMissingFromCommitment,
			);
		});
	}

	#[test]
	fn verify_beefy_signed_commitment_works() {
		let artifacts = verify_beefy_signed_commitment::<TestRuntime, ()>(
			0,
			&BridgedBeefyValidatorSet::<TestRuntime, ()>::new(validator_ids(0, 8), 0).unwrap(),
			&sign_commitment(
				Commitment {
					payload: BeefyPayload::new(
						MMR_ROOT_PAYLOAD_ID,
						BeefyMmrHash::from([42u8; 32]).encode(),
					),
					block_number: 20,
					validator_set_id: 0,
				},
				&validator_keys(0, 8),
			),
		)
		.unwrap();

		assert_eq!(
			artifacts,
			CommitmentVerificationArtifacts {
				finalized_block_number: 20,
				mmr_root: BeefyMmrHash::from([42u8; 32]),
			}
		);
	}
}
