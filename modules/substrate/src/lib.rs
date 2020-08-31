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
use bp_substrate::{AuthoritySet, ImportedHeader, ScheduledChange};
use frame_support::{decl_error, decl_module, decl_storage, dispatch};
use frame_system::ensure_signed;
use sp_runtime::traits::Header as HeaderT;
use sp_std::{marker::PhantomData, prelude::*};

mod verifier;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

type Hash<T> = <T as HeaderT>::Hash;
type Number<T> = <T as HeaderT>::Number;

pub trait Trait: frame_system::Trait {}

decl_storage! {
	trait Store for Module<T: Trait> as SubstrateBridge {
		/// Best finalized header.
		// Maybe make this a HeaderId?
		BestFinalized: Option<T::Header>;
		/// Headers which have been imported into the pallet.
		// Maybe made a HeaderId?
		ImportedHeaders: map hasher(identity) T::Hash => Option<ImportedHeader<T::Header>>;
		/// The current Grandpa Authority set.
		CurrentAuthoritySet: AuthoritySet;
		/// The next scheduled authority set change.
		NextScheduledChange: ScheduledChange<Number<T::Header>>;
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

		/// Import a signed Substrate header into the runtime.
		///
		/// Will check for finality proofs, and if available finalize any already imported
		/// headers which are finalized by the newly imported header.
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

		// TODO: Remove this
		#[weight = 0]
		pub fn test(origin) -> dispatch::DispatchResult {
			Ok(())
		}
	}
}

/// Expected interface for interacting with bridge pallet storage.
pub trait BridgeStorage {
	/// The header type being used by the pallet.
	type Header: HeaderT;

	/// Write a header to storage.
	fn write_header(&mut self, header: &ImportedHeader<Self::Header>);

	/// Get the best finalized header the pallet knows of.
	///
	/// Returns None if there are no finalized headers.
	fn best_finalized_header(&self) -> Option<Self::Header>;

	/// Check if a particular header is known to the pallet.
	fn header_exists(&self, hash: <Self::Header as HeaderT>::Hash) -> bool;

	/// Get a specific header by its hash.
	///
	/// Returns None if it is not known to the pallet.
	fn get_header_by_hash(&self, hash: <Self::Header as HeaderT>::Hash) -> Option<ImportedHeader<Self::Header>>;

	/// Get the current Grandpa authority set.
	fn current_authority_set(&self) -> AuthoritySet;

	/// Update the current Grandpa authority set.
	///
	/// Should only be updated when a scheduled change has been triggered.
	fn update_current_authority_set(&self, new_set: AuthoritySet);

	/// Get the next scheduled Grandpa authority set change.
	fn scheduled_set_change(&self) -> ScheduledChange<<Self::Header as HeaderT>::Number>;

	/// Schedule a Grandpa authority set change in the future.
	fn schedule_next_set_change(&self, next_change: ScheduledChange<<Self::Header as HeaderT>::Number>);

	/// Helper function to store an unfinalized header.
	#[cfg(test)]
	fn import_unfinalized_header(&mut self, header: Self::Header);
}

/// Used to interact with the pallet storage in a more abstract way.
#[derive(Default)]
pub struct PalletStorage<T>(PhantomData<T>);

impl<T> PalletStorage<T> {
	fn new() -> Self {
		Self(PhantomData::<T>::default())
	}
}

impl<T: Trait> BridgeStorage for PalletStorage<T> {
	type Header = T::Header;

	fn write_header(&mut self, header: &ImportedHeader<T::Header>) {
		let hash = header.header.hash();
		<ImportedHeaders<T>>::insert(hash, header);
	}

	fn best_finalized_header(&self) -> Option<T::Header> {
		<BestFinalized<T>>::get()
	}

	fn header_exists(&self, hash: Hash<T::Header>) -> bool {
		self.get_header_by_hash(hash).is_some()
	}

	fn get_header_by_hash(&self, hash: Hash<T::Header>) -> Option<ImportedHeader<T::Header>> {
		<ImportedHeaders<T>>::get(hash)
	}

	fn current_authority_set(&self) -> AuthoritySet {
		CurrentAuthoritySet::get()
	}

	fn update_current_authority_set(&self, new_set: AuthoritySet) {
		CurrentAuthoritySet::put(new_set)
	}

	fn scheduled_set_change(&self) -> ScheduledChange<Number<T::Header>> {
		<NextScheduledChange<T>>::get()
	}

	fn schedule_next_set_change(&self, next_change: ScheduledChange<Number<T::Header>>) {
		<NextScheduledChange<T>>::put(next_change)
	}

	#[cfg(test)]
	fn import_unfinalized_header(&mut self, header: T::Header) {
		let h = ImportedHeader {
			header,
			is_finalized: false,
		};

		self.write_header(&h);
	}
}
