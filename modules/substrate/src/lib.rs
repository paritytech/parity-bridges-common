// Copyright 2020 Parity Technologies (UK) Ltd.
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

//! Substrate Bridge Pallet
//!
//! This pallet is an on-chain light client for chains which have a notion of finality. It's main
//! jobs are to verify that a header is valid and that is has been finalized correctly. In doing so
//! it will form a sparse header-chain of finalized headers which can be trusted checkpoints for
//! other applications.

#![cfg_attr(not(feature = "std"), no_std)]
// Runtime-generated enums
#![allow(clippy::large_enum_variant)]

use bp_header_chain::{FinalityVerifier, HeaderVerifier};
use frame_support::{decl_error, decl_module, decl_storage, dispatch};
use frame_system::ensure_signed;

pub trait Trait: frame_system::Trait {
	type HeaderVerifier: HeaderVerifier;
	type FinalityVerifier: FinalityVerifier;
}

decl_storage! {
	trait Store for Module<T: Trait> as SubstrateBridge {
		/// Best finalized header.
		// Maybe make this a HeaderId?
		BestFinalized: Option<T::Header>;
		/// Headers which have been imported into the runtime.
		// Maybe made a HeaderId?
		// Should maybe have some sort of notion of ancestry here.
		ImportedHeaders: map hasher(identity) T::Hash => Option<T::Header>;
	}
}

decl_error! {
	pub enum Error for Module<T: Trait> {
		/// This header has failed basic verification.
		InvalidHeader,
		/// This header has not been finalized.
		UnfinalizedHeader,
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		#[weight = 0]
		pub fn import_signed_header(
			origin,
			header: <T::HeaderVerifier as HeaderVerifier>::Header,
			extra_data: Option<<T::HeaderVerifier as HeaderVerifier>::Extra>,
			finalty_proof: <T::FinalityVerifier as FinalityVerifier>::Proof,
		) -> dispatch::DispatchResult {
			let who = ensure_signed(origin)?;

			let is_valid = T::HeaderVerifier::verify_header(&header, &extra_data);
			if !is_valid {
				return Err(<Error<T>>::InvalidHeader.into())
			}

			let is_finalized = T::FinalityVerifier::verify_finality(&header, &finalty_proof);
			if !is_finalized {
				return Err(<Error<T>>::UnfinalizedHeader.into())
			}

			<BestFinalized<T>>::put(&header);
			<ImportedHeaders<T>>::insert(header.hash(), header);

			Ok(())
		}
	}
}
