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

use crate::verifier::{ChainVerifier, FinalityProof};
use bp_substrate::{AuthoritySet, ScheduledChange};
use frame_support::{decl_error, decl_module, decl_storage, dispatch};
use frame_system::ensure_signed;
use parity_scale_codec::{Decode, Encode};
use sp_runtime::traits::Header as HeaderT;
use sp_std::{marker::PhantomData, prelude::*};

mod verifier;

pub trait Trait: frame_system::Trait {
	// type Verifier: ChainVerifier;
}

decl_storage! {
	trait Store for Module<T: Trait> as SubstrateBridge {
		/// Best finalized header.
		// Maybe make this a HeaderId?
		BestFinalized: Option<T::Header>;
		/// Headers which have been imported into the pallet.
		// Maybe made a HeaderId?
		// Should maybe have some sort of notion of ancestry here.
		ImportedHeaders: map hasher(identity) T::Hash => Option<T::Header>;
		/// The current Grandpa Authority set.
		CurrentAuthoritySet: AuthoritySet;
		/// The next scheduled authority set change.
		NextScheduledChange: ScheduledChange<<T::Header as HeaderT>::Number>;
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
			header: T::Header,
			finality_proof: Option<FinalityProof>,
		) -> dispatch::DispatchResult {
			let _ = ensure_signed(origin)?;

			let mut storage = PalletStorage::<T>::new();
			let _ =
				verifier::Verifier::import_header(&mut storage, &header, None).map_err(|_| <Error<T>>::InvalidHeader)?;

			Ok(())
		}
	}
}

#[derive(Default, Encode, Decode)]
struct ImportedHeader<T: Trait> {
	header: T::Header,
	is_finalized: bool,
}

pub trait BridgeStorage {
	type Header: HeaderT;

	fn best_finalized_header(&self) -> Option<Self::Header>;
	fn write_header(&mut self, header: &Self::Header);
	fn header_exists(&self, hash: <Self::Header as HeaderT>::Hash) -> bool;
	fn get_header_by_hash(&self, hash: <Self::Header as HeaderT>::Hash) -> Option<Self::Header>;
	fn current_authority_set(&self) -> AuthoritySet;
	fn update_current_authority_set(&self, new_set: AuthoritySet);
	fn scheduled_set_change(&self) -> ScheduledChange<<Self::Header as HeaderT>::Number>;
	fn schedule_next_set_change(&self, next_change: ScheduledChange<<Self::Header as HeaderT>::Number>);
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

	fn write_header(&mut self, header: &T::Header) {
		<ImportedHeaders<T>>::insert(header.hash(), header);
	}

	fn best_finalized_header(&self) -> Option<T::Header> {
		<BestFinalized<T>>::get()
	}

	fn header_exists(&self, hash: <T::Header as HeaderT>::Hash) -> bool {
		self.get_header_by_hash(hash).is_some()
	}

	fn get_header_by_hash(&self, hash: <T::Header as HeaderT>::Hash) -> Option<T::Header> {
		<ImportedHeaders<T>>::get(hash)
	}

	fn current_authority_set(&self) -> AuthoritySet {
		CurrentAuthoritySet::get()
	}

	fn update_current_authority_set(&self, new_set: AuthoritySet) {
		CurrentAuthoritySet::put(new_set)
	}

	fn scheduled_set_change(&self) -> ScheduledChange<<T::Header as HeaderT>::Number> {
		<NextScheduledChange<T>>::get()
	}

	fn schedule_next_set_change(&self, next_change: ScheduledChange<<T::Header as HeaderT>::Number>) {
		<NextScheduledChange<T>>::put(next_change)
	}
}
