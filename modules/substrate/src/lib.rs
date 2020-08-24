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

use bp_header_chain::{BridgeStorage, ChainVerifier};
use frame_support::{decl_error, decl_module, decl_storage, dispatch};
use frame_system::ensure_signed;
use parity_scale_codec::{Codec, EncodeLike};
use sp_finality_grandpa::{AuthorityList, SetId};
use sp_runtime::traits::Header;
use sp_std::{marker::PhantomData, prelude::*};

pub trait Trait: frame_system::Trait {
	type Verifier: ChainVerifier;
}

decl_storage! {
	trait Store for Module<T: Trait> as SubstrateBridge {
		/// Best finalized header.
		// Maybe make this a HeaderId?
		BestFinalized: Option<T::Header>;
		/// Headers which have been imported into the pallet.
		// Maybe made a HeaderId?
		// Should maybe have some sort of notion of ancestry here.
		ImportedHeaders: map hasher(identity) T::Hash => Option<ImportedHeader<T>>;
		/// The current Grandpa Authority set id.
		AuthoritySetId: SetId;
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
			header: <T::Verifier as ChainVerifier>::Header,
			extra_data: Option<<T::Verifier as ChainVerifier>::Extra>,
			finality_proof: Option<<T::Verifier as ChainVerifier>::Proof>,
		) -> dispatch::DispatchResult {
			let _ = ensure_signed(origin)?;

			let mut storage = PalletStorage::<T>::new();
			let is_valid = T::Verifier::import_header(&mut storage, &header, extra_data, finality_proof);

			if !is_valid {
				return Err(<Error<T>>::InvalidHeader.into())
			}

			Ok(())
		}
	}
}

#[derive(Default, Encode, Decode)]
struct ImportedHeader<T: Trait> {
	header: T::Header,
	is_finalized: bool,
}

#[derive(Default)]
pub struct PalletStorage<T>(PhantomData<T>);

impl<T> PalletStorage<T> {
	fn new() -> Self {
		Self(PhantomData::<T>::default())
	}
}

impl<T: Trait> BridgeStorage for PalletStorage<T> {
	type Header = T::Header;
	type Hash = T::Hash;

	fn write_header(&mut self, imported_header: ImportedHeader<T>) -> bool {
		<ImportedHeaders<T>>::insert(imported_header.header.hash(), header);
		true
	}

	fn best_finalized_header(&self) -> Option<T::Header> {
		<BestFinalized<T>>::get()
	}

	fn header_exists(&self, hash: T::Hash) -> bool {
		<ImportedHeaders<T>>::get(hash).is_some()
	}

	fn authority_set_id(&self) -> SetId {
		AuthoritySetId::get()
	}
}
