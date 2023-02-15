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

use crate::{weights::WeightInfo, BridgedBlockHash, BridgedBlockNumber, Config, CurrentAuthoritySet, Error, Pallet};
use bp_header_chain::ChainWithGrandpa;
use bp_runtime::BlockNumberOf;
use codec::{Encode, MaxEncodedLen};
use frame_support::{dispatch::CallableCallFor, traits::IsSubType, weights::Weight, RuntimeDebug};
use sp_finality_grandpa::AuthorityId;
use sp_runtime::{
	traits::Header,
	transaction_validity::{InvalidTransaction, TransactionValidity, ValidTransaction},
	SaturatedConversion,
};

/// Info about a `SubmitParachainHeads` call which tries to update a single parachain.
#[derive(Copy, Clone, PartialEq, RuntimeDebug)]
pub struct SubmitFinalityProofInfo<N> {
	/// Number of the finality target.
	pub block_number: N,
	/// Extra weight that we assume is included in the call.
	///
	/// We have some assumptions about headers and justifications of the bridged chain.
	/// We know that if our assumptions are correct, then the call must not have the
	/// weight above some limit. The fee paid for weight above that limit, is never refunded.
	pub extra_weight: Weight,
	/// Extra size (in bytes) that we assume are included in the call.
	///
	/// We have some assumptions about headers and justifications of the bridged chain.
	/// We know that if our assumptions are correct, then the call must not have the
	/// weight above some limit. The fee paid for bytes above that limit, is never refunded.
	pub extra_size: u32,
}

/// Helper struct that provides methods for working with the `SubmitFinalityProof` call.
pub struct SubmitFinalityProofHelper<T: Config<I>, I: 'static> {
	_phantom_data: sp_std::marker::PhantomData<(T, I)>,
}

impl<T: Config<I>, I: 'static> SubmitFinalityProofHelper<T, I> {
	/// Check that the GRANDPA head provided by the `SubmitFinalityProof` is better than the best
	/// one we know.
	pub fn check_obsolete(
		finality_target: BlockNumberOf<T::BridgedChain>,
	) -> Result<(), Error<T, I>> {
		let best_finalized = crate::BestFinalized::<T, I>::get().ok_or_else(|| {
			log::trace!(
				target: crate::LOG_TARGET,
				"Cannot finalize header {:?} because pallet is not yet initialized",
				finality_target,
			);
			<Error<T, I>>::NotInitialized
		})?;

		if best_finalized.number() >= finality_target {
			log::trace!(
				target: crate::LOG_TARGET,
				"Cannot finalize obsolete header: bundled {:?}, best {:?}",
				finality_target,
				best_finalized,
			);

			return Err(Error::<T, I>::OldHeader)
		}

		Ok(())
	}

	/// Check if the `SubmitFinalityProof` was successfully executed.
	pub fn was_successful(finality_target: BlockNumberOf<T::BridgedChain>) -> bool {
		match crate::BestFinalized::<T, I>::get() {
			Some(best_finalized) => best_finalized.number() == finality_target,
			None => false,
		}
	}
}

/// Trait representing a call that is a sub type of this pallet's call.
pub trait CallSubType<T: Config<I, RuntimeCall = Self>, I: 'static>:
	IsSubType<CallableCallFor<Pallet<T, I>, T>>
{
	/// Extract the finality target from a `SubmitParachainHeads` call.
	fn submit_finality_proof_info(&self) -> Option<SubmitFinalityProofInfo<BridgedBlockNumber<T, I>>> {
		if let Some(crate::Call::<T, I>::submit_finality_proof { finality_target, justification }) =
			self.is_sub_type()
		{
			let block_number = *finality_target.number();

			let actual_call_weight = T::WeightInfo::submit_finality_proof(
				justification.commit.precommits.len().saturated_into(),
				justification.votes_ancestries.len().saturated_into(),
			);
			let actual_call_size: u32 = finality_target.encoded_size()
				.saturating_add(justification.encoded_size())
				.saturated_into();

			let authorities_set_sizegth = CurrentAuthoritySet::<T, I>::get().authorities.len().saturated_into();
			let required_precommits = bp_header_chain::justification::required_justification_precommits(authorities_set_sizegth);
			let max_expected_call_weight = max_expected_call_weight::<T, I>(required_precommits);
			let max_expected_call_size = max_expected_call_size::<T, I>(required_precommits);

			return Some(SubmitFinalityProofInfo {
				block_number,
				extra_weight: actual_call_weight.saturating_sub(max_expected_call_weight),
				extra_size: actual_call_size.saturating_sub(max_expected_call_size),
			})
		}

		None
	}

	/// Validate Grandpa headers in order to avoid "mining" transactions that provide outdated
	/// bridged chain headers. Without this validation, even honest relayers may lose their funds
	/// if there are multiple relays running and submitting the same information.
	fn check_obsolete_submit_finality_proof(&self) -> TransactionValidity
	where
		Self: Sized,
	{
		let finality_target = match self.submit_finality_proof_info() {
			Some(finality_proof) => finality_proof,
			_ => return Ok(ValidTransaction::default()),
		};

		match SubmitFinalityProofHelper::<T, I>::check_obsolete(finality_target.block_number) {
			Ok(_) => Ok(ValidTransaction::default()),
			Err(Error::<T, I>::OldHeader) => InvalidTransaction::Stale.into(),
			Err(_) => InvalidTransaction::Call.into(),
		}
	}
}

impl<T: Config<I>, I: 'static> CallSubType<T, I> for T::RuntimeCall where
	T::RuntimeCall: IsSubType<CallableCallFor<Pallet<T, I>, T>>
{
}

/// Returns maximal expected `submit_finality_proof` call weight.
fn max_expected_call_weight<T: Config<I>, I: 'static>(
	required_precommits: u32,
) -> Weight {
	T::WeightInfo::submit_finality_proof(
		required_precommits,
		T::BridgedChain::REASONABLE_HEADERS_IN_JUSTIFICATON_ANCESTRY,
	)
}

/// Returns maximal expected size of `submit_finality_proof` call arguments.
fn max_expected_call_size<T: Config<I>, I: 'static>(
	required_precommits: u32,
) -> u32 {
	// we don't need precise results here - just estimations, so some details
	// are removed from computations (e.g. bytes required to encode vector length)

	// structures in `finality_grandpa` crate are not implementing `MaxEncodedLength`, so
	// here's our estimation for the `finality_grandpa::Commit` struct size
	//
	// precommit is: hash + number
	// signed precommit is: precommit + signature (64b) + authority id
	// commit is: hash + number + vec of signed precommits
	let signed_precommit_size: u32 = BridgedBlockNumber::<T, I>::max_encoded_len()
		.saturating_add(BridgedBlockHash::<T, I>::max_encoded_len().saturated_into())
		.saturating_add(64)
		.saturating_add(AuthorityId::max_encoded_len().saturated_into())
		.saturated_into();
	let max_expected_signed_commit_size = signed_precommit_size
		.saturating_mul(required_precommits)
		.saturating_add(BridgedBlockNumber::<T, I>::max_encoded_len().saturated_into())
		.saturating_add(BridgedBlockHash::<T, I>::max_encoded_len().saturated_into());

	// justification is a signed GRANDPA commit, `votes_ancestries` vector and round number
	let max_expected_votes_ancestries_size = T::BridgedChain::REASONABLE_HEADERS_IN_JUSTIFICATON_ANCESTRY
		.saturating_mul(T::BridgedChain::AVERAGE_HEADER_SIZE_IN_JUSTIFICATION);
	let max_expected_justification_size = 8u32
		.saturating_add(max_expected_signed_commit_size)
		.saturating_add(max_expected_votes_ancestries_size);

	// call arguments are header and jsutification
	T::BridgedChain::MAX_HEADER_SIZE.saturating_add(max_expected_justification_size)
}

#[cfg(test)]
mod tests {
	use crate::{
		call_ext::CallSubType,
		mock::{run_test, test_header, RuntimeCall, TestNumber, TestRuntime},
		BestFinalized,
	};
	use bp_runtime::HeaderId;
	use bp_test_utils::make_default_justification;

	fn validate_block_submit(num: TestNumber) -> bool {
		let bridge_grandpa_call = crate::Call::<TestRuntime, ()>::submit_finality_proof {
			finality_target: Box::new(test_header(num)),
			justification: make_default_justification(&test_header(num)),
		};
		RuntimeCall::check_obsolete_submit_finality_proof(&RuntimeCall::Grandpa(
			bridge_grandpa_call,
		))
		.is_ok()
	}

	fn sync_to_header_10() {
		let header10_hash = sp_core::H256::default();
		BestFinalized::<TestRuntime, ()>::put(HeaderId(10, header10_hash));
	}

	#[test]
	fn extension_rejects_obsolete_header() {
		run_test(|| {
			// when current best finalized is #10 and we're trying to import header#5 => tx is
			// rejected
			sync_to_header_10();
			assert!(!validate_block_submit(5));
		});
	}

	#[test]
	fn extension_rejects_same_header() {
		run_test(|| {
			// when current best finalized is #10 and we're trying to import header#10 => tx is
			// rejected
			sync_to_header_10();
			assert!(!validate_block_submit(10));
		});
	}

	#[test]
	fn extension_accepts_new_header() {
		run_test(|| {
			// when current best finalized is #10 and we're trying to import header#15 => tx is
			// accepted
			sync_to_header_10();
			assert!(validate_block_submit(15));
		});
	}
}
