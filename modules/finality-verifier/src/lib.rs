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

use bp_runtime::{BlockNumberOf, Chain, HashOf, HasherOf, HeaderOf};
use frame_support::{decl_error, decl_module, decl_storage, dispatch::DispatchResult};
use frame_system::ensure_signed;
use sp_runtime::traits::Header as HeaderT;

/// Header of the bridged chain.
pub(crate) type BridgedHeader<T> = HeaderOf<<T as Config>::BridgedChain>;

trait AncestryChecker {
	type Chain: Chain;
	type Proof;

	fn are_ancestors(
		ancestor: <Self::Chain as Chain>::Header,
		child: <Self::Chain as Chain>::Header,
		proof: Self::Proof,
	) -> bool;
}

trait HeaderChain {
	type Chain: Chain;

	fn best_finalized() -> <Self::Chain as Chain>::Header;
}

pub trait Config: frame_system::Config {
	type BridgedChain: Chain;
	type HeaderChain: HeaderChain;
	type AncestryChecker: AncestryChecker;
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
			ancestry_proof: Vec<T::Hash>,
		) -> DispatchResult {
			let _ = ensure_signed(origin)?;

			verify_justification().map_err(|_| <Error<T>>::InvalidJustification)?;

			let best_finalized = T::HeaderChain::best_finalized();
			T::AncestryChecker::are_ancestors(best_finalized, finality_target, ancestry_proof);

			todo!("Write known good headers to storage.")
		}
	}
}

impl<T: Config> Module<T> {
	pub fn bar() {
		todo!()
	}
}

fn verify_justification() -> Result<(), ()> {
	todo!()
}
