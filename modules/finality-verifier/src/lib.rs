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
use frame_support::{decl_error, decl_module, decl_storage, dispatch::DispatchResult, traits::Get, Parameter};
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
	type AncestryProof: Parameter + Get<Vec<<Self::BridgedChain as Chain>::Header>>;
}

decl_storage! {
	trait Store for Module<T: Config> as FinalityVerifier {}
}

decl_error! {
	pub enum Error for Module<T: Config> {
		InvalidJustification,
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
			ancestry_proof: Vec<BridgedHeader<T>>, // T::AncestryProof,
		) -> DispatchResult {
			let _ = ensure_signed(origin)?;

			let authority_set = T::HeaderChain::authority_set();

			let voter_set = VoterSet::new(authority_set.authorities).expect("TODO");
			let set_id = 1;

			let header_id = (finality_target.hash(), *finality_target.number());
			verify_justification::<BridgedHeader<T>>(
				header_id,
				set_id,
				voter_set,
				&justification
			)
			.map_err(|_| <Error<T>>::InvalidJustification)?;

			let best_finalized = T::HeaderChain::best_finalized();
			T::AncestryChecker::are_ancestors(&best_finalized, &finality_target, &ancestry_proof);

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
	use bp_test_utils::{alice, authority_list, bob, make_justification_for_header};
	use frame_support::assert_ok;

	#[test]
	fn it_works() {
		run_test(|| {
			let genesis = test_header(0);
			let header1 = test_header(1);
			let header2 = test_header(2);

			let init_data = pallet_substrate_bridge::InitializationData {
				header: genesis,
				authority_list: vec![(alice(), 1), (bob(), 1)],
				set_id: 1,
				scheduled_change: None,
				is_halted: false,
			};

			assert_ok!(pallet_substrate_bridge::Module::<TestRuntime>::initialize(
				Origin::root(),
				init_data.clone()
			));

			let justification = vec![1u8];
			let ancestry_proof = vec![header1.clone(), header2.clone()];

			// Just a wee rename
			let finality_target = header2;

			assert_ok!(Module::<TestRuntime>::submit_finality_proof(
				Origin::signed(1),
				finality_target.clone(),
				justification,
				ancestry_proof,
			));

			assert_eq!(
				pallet_substrate_bridge::Module::<TestRuntime>::best_headers(),
				vec![(*finality_target.number(), finality_target.hash())]
			);

			assert_eq!(
				pallet_substrate_bridge::Module::<TestRuntime>::best_finalized(),
				finality_target
			);
		})
	}
}
