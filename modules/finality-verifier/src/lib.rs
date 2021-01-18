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

//! Substrate Finality Verifier Pallet

#![cfg_attr(not(feature = "std"), no_std)]
// Runtime-generated enums
#![allow(clippy::large_enum_variant)]

use bp_header_chain::{justification::verify_justification, AncestryChecker, HeaderChain};
use bp_runtime::{BlockNumberOf, Chain, HashOf, HasherOf, HeaderOf};
use finality_grandpa::voter_set::VoterSet;
use frame_support::{decl_error, decl_module, decl_storage, dispatch::DispatchResult, ensure, traits::Get, Parameter};
use frame_system::ensure_signed;
use sp_runtime::traits::Header as HeaderT;

#[cfg(test)]
mod mock;

/// Block number of the bridged chain.
pub(crate) type BridgedBlockNumber<T> = BlockNumberOf<<T as Config>::BridgedChain>;
/// Block hash of the bridged chain.
pub(crate) type BridgedBlockHash<T> = HashOf<<T as Config>::BridgedChain>;
/// Hasher of the bridged chain.
pub(crate) type BridgedBlockHasher<T> = HasherOf<<T as Config>::BridgedChain>;
/// Header of the bridged chain.
pub(crate) type BridgedHeader<T> = HeaderOf<<T as Config>::BridgedChain>;

pub trait Config: frame_system::Config {
	type BridgedChain: Chain;
	type HeaderChain: HeaderChain<<Self::BridgedChain as Chain>::Header>;
	type AncestryChecker: AncestryChecker<
		<Self::BridgedChain as Chain>::Header,
		Vec<<Self::BridgedChain as Chain>::Header>,
	>;
}

decl_storage! {
	trait Store for Module<T: Config> as FinalityVerifier {}
}

decl_error! {
	pub enum Error for Module<T: Config> {
		/// The given justification is invalid for the given header.
		InvalidJustification,
		/// The given ancestry proof is unable to verify that the child and ancestor headers are
		/// related.
		InvalidAncestryProof,
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		#[weight = 0]
		fn submit_finality_proof(
			origin,
			finality_target: BridgedHeader<T>,
			justification: Vec<u8>,
			ancestry_proof: Vec<BridgedHeader<T>>,
		) -> DispatchResult {
			let _ = ensure_signed(origin)?;

			let authority_set = T::HeaderChain::authority_set();
			let voter_set = VoterSet::new(authority_set.authorities).expect("TODO");
			let set_id = authority_set.set_id;

			verify_justification::<BridgedHeader<T>>(
				(finality_target.hash(), *finality_target.number()),
				set_id,
				voter_set,
				&justification
			)
			.map_err(|_| <Error<T>>::InvalidJustification)?;

			let best_finalized = T::HeaderChain::best_finalized();
			ensure!(
				T::AncestryChecker::are_ancestors(&best_finalized, &finality_target, &ancestry_proof),
				<Error<T>>::InvalidAncestryProof
			);

			// If for whatever reason we are unable to fully import headers and the corresponding
			// finality proof we want to avoid writing to the base pallet storage
			use frame_support::storage::{with_transaction, TransactionOutcome};
			with_transaction(|| {
				// TODO: We should probably bound this
				for header in ancestry_proof {
					if T::HeaderChain::import_header(header).is_err() {
						return TransactionOutcome::Rollback(())
					}
				}

				if T::HeaderChain::import_finality_proof(finality_target, justification).is_err() {
					return TransactionOutcome::Rollback(())
				}

				TransactionOutcome::Commit(())
			});

			Ok(())
		}
	}
}

impl<T: Config> Module<T> {
	pub fn bar() {
		todo!()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{run_test, test_header, Origin, TestRuntime};
	use bp_test_utils::{authority_list, make_justification_for_header};
	use codec::Encode;
	use frame_support::{assert_err, assert_ok};

	fn initialize_substrate_bridge() {
		let genesis = test_header(0);

		let init_data = pallet_substrate_bridge::InitializationData {
			header: genesis,
			authority_list: authority_list(),
			set_id: 1,
			scheduled_change: None,
			is_halted: false,
		};

		assert_ok!(pallet_substrate_bridge::Module::<TestRuntime>::initialize(
			Origin::root(),
			init_data
		));
	}

	#[test]
	fn succesfully_imports_header_with_valid_finality_and_ancestry_proofs() {
		run_test(|| {
			initialize_substrate_bridge();

			let child = test_header(1);
			let header = test_header(2);

			let set_id = 1;
			let grandpa_round = 1;
			let justification =
				make_justification_for_header(&header, grandpa_round, set_id, &authority_list()).encode();
			let ancestry_proof = vec![child.clone(), header.clone()];

			assert_ok!(Module::<TestRuntime>::submit_finality_proof(
				Origin::signed(1),
				header.clone(),
				justification,
				ancestry_proof,
			));

			// TODO: Remove [0] once #653 is merged
			assert_eq!(
				pallet_substrate_bridge::Module::<TestRuntime>::best_headers()[0],
				(*header.number(), header.hash())
			);

			assert_eq!(pallet_substrate_bridge::Module::<TestRuntime>::best_finalized(), header);
		})
	}

	#[test]
	fn does_not_import_header_with_invalid_finality_proof() {
		run_test(|| {
			initialize_substrate_bridge();

			let child = test_header(1);
			let header = test_header(2);

			let justification = [1u8; 32].encode();
			let ancestry_proof = vec![child.clone(), header.clone()];

			assert_err!(
				Module::<TestRuntime>::submit_finality_proof(
					Origin::signed(1),
					header.clone(),
					justification,
					ancestry_proof,
				),
				<Error<TestRuntime>>::InvalidJustification
			);
		})
	}

	#[test]
	fn does_not_import_header_with_invalid_ancestry_proof() {
		run_test(|| {
			initialize_substrate_bridge();

			let header = test_header(2);

			let set_id = 1;
			let grandpa_round = 1;
			let justification =
				make_justification_for_header(&header, grandpa_round, set_id, &authority_list()).encode();

			// For testing, we've made it so that an empty ancestry proof is invalid
			let ancestry_proof = vec![];

			assert_err!(
				Module::<TestRuntime>::submit_finality_proof(Origin::signed(1), header, justification, ancestry_proof,),
				<Error<TestRuntime>>::InvalidAncestryProof
			);
		})
	}
}
