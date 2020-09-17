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
//! This pallet is an on-chain light client for chains which have a notion of finality.
//!
//! It has a simple interface for achieving this. First it can import headers to the runtime
//! storage. During this it will check the validity of the headers and ensure they don't conflict
//! with any existing headers (e.g they're on a different finalized chain). Secondly it can finalize
//! an already imported header (and its ancestors) given a valid Grandpa justification.
//!
//! With these two functions the pallet is able to form a "source of truth" for what headers have
//! been finalized on a given Substrate chain. This can be a useful source of info for other
//! higher-level applications.

#![cfg_attr(not(feature = "std"), no_std)]
// Runtime-generated enums
#![allow(clippy::large_enum_variant)]

use bp_substrate::{AuthoritySet, ImportedHeader, ScheduledChange};
use frame_support::{decl_error, decl_module, decl_storage, dispatch};
use frame_system::ensure_signed;
use sp_runtime::traits::Header as HeaderT;
use sp_std::{marker::PhantomData, prelude::*};

mod justification;
mod storage_proof;
mod verifier;

#[cfg(test)]
mod mock;

type Hash<T> = <T as HeaderT>::Hash;
type Number<T> = <T as HeaderT>::Number;

pub trait Trait: frame_system::Trait {}

decl_storage! {
	trait Store for Module<T: Trait> as SubstrateBridge {
		/// Hash of the best finalized header.
		BestFinalized: T::Hash;
		/// Headers which have been imported into the pallet.
		ImportedHeaders: map hasher(identity) T::Hash => Option<ImportedHeader<T::Header>>;
		/// The current Grandpa Authority set.
		CurrentAuthoritySet: AuthoritySet;
		/// The next scheduled authority set change.
		NextScheduledChange: ScheduledChange<Number<T::Header>>;
	}
	add_extra_genesis {
		config(initial_header): Option<T::Header>;
		config(initial_authority_list): sp_finality_grandpa::AuthorityList;
		config(initial_set_id): sp_finality_grandpa::SetId;
		config(first_scheduled_change): Option<ScheduledChange<Number<T::Header>>>;
		build(|config| {
			assert!(
				!config.initial_authority_list.is_empty(),
				"An initial authority list is needed."
			);

			let initial_header = config
				.initial_header
				.clone()
				.expect("An initial header is needed");

			let first_scheduled_change = config
				.first_scheduled_change
				.as_ref()
				.expect("An initial authority set is needed");

			<BestFinalized<T>>::put(initial_header.hash());
			<ImportedHeaders<T>>::insert(
				initial_header.hash(),
				ImportedHeader::new(initial_header, false, true),
			);

			let authority_set =
				AuthoritySet::new(config.initial_authority_list.clone(), config.initial_set_id);
			CurrentAuthoritySet::put(authority_set);

			<NextScheduledChange<T>>::put(first_scheduled_change);
		})
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
		/// This will perform some basic checks to make sure it is fine to
		/// import into the runtime. However, it does not perform any checks
		/// related to finality.
		// TODO: Update weights [#78]
		#[weight = 0]
		pub fn import_signed_header(
			origin,
			header: T::Header,
		) -> dispatch::DispatchResult {
			let _ = ensure_signed(origin)?;
			frame_support::debug::trace!(target: "sub-bridge", "Got header {:?}", header);

			let mut verifier = verifier::Verifier {
				storage: PalletStorage::<T>::new(),
			};

			let _ =
				verifier.import_header(header).map_err(|_| <Error<T>>::InvalidHeader)?;

			Ok(())
		}

		/// Import a finalty proof for a particular header.
		///
		/// This will take care of finalizing any already imported headers
		/// which get finalized when importing this particular proof, as well
		/// as updating the current and next validator sets.
		// TODO: Update weights [#78]
		#[weight = 0]
		pub fn finalize_header(
			origin,
			hash: Hash<T::Header>,
			finality_proof: Vec<u8>,
		) -> dispatch::DispatchResult {
			let _ = ensure_signed(origin)?;
			frame_support::debug::trace!(target: "sub-bridge", "Got header hash {:?}", hash);

			let mut verifier = verifier::Verifier {
				storage: PalletStorage::<T>::new(),
			};

			let _ = verifier
				.verify_finality(hash, &finality_proof)
				.map_err(|_| <Error<T>>::UnfinalizedHeader)?;

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
	fn best_finalized_header(&self) -> ImportedHeader<Self::Header>;

	/// Update the best finalized header the pallet knows of.
	fn update_best_finalized(&self, hash: <Self::Header as HeaderT>::Hash);

	/// Check if a particular header is known to the pallet.
	fn header_exists(&self, hash: <Self::Header as HeaderT>::Hash) -> bool;

	/// Get a specific header by its hash.
	///
	/// Returns None if it is not known to the pallet.
	fn header_by_hash(&self, hash: <Self::Header as HeaderT>::Hash) -> Option<ImportedHeader<Self::Header>>;

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

	fn best_finalized_header(&self) -> ImportedHeader<T::Header> {
		let hash = <BestFinalized<T>>::get();
		self.header_by_hash(hash)
			.expect("A finalized header was added at genesis, therefore this must always exist")
	}

	fn update_best_finalized(&self, hash: Hash<T::Header>) {
		<BestFinalized<T>>::put(hash)
	}

	fn header_exists(&self, hash: Hash<T::Header>) -> bool {
		self.header_by_hash(hash).is_some()
	}

	fn header_by_hash(&self, hash: Hash<T::Header>) -> Option<ImportedHeader<T::Header>> {
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
}
