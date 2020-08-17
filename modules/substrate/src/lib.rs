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
//! This pallet is an on-chain light client for chains which use the Grandpa finality
//! gadget. It will be able to verify that headers have been included and subsequenty
//! finalized by Grandpa.

#![cfg_attr(not(feature = "std"), no_std)]
// Runtime-generated enums
#![allow(clippy::large_enum_variant)]

use frame_support::{decl_error, decl_module, decl_storage, dispatch, traits::Get};
use frame_system::{ensure_signed, ensure_none};

pub trait Trait: frame_system::Trait {}

decl_storage! {
	trait Store for Module<T: Trait> as SubstrateBridge {
		Something get(fn something): Option<u32>;
	}
}

decl_error! {
	pub enum Error for Module<T: Trait> {
		/// Error names should be descriptive.
		NoneValue,
		/// Errors should have helpful documentation associated with them.
		StorageOverflow,
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		#[weight = 0]
		pub fn import_unsiged_header(origin, something: u32) -> dispatch::DispatchResult {
			let who = ensure_none(origin)?;

			// Return a successful DispatchResult
			Ok(())
		}

		#[weight = 0]
		pub fn import_siged_header(origin, something: u32) -> dispatch::DispatchResult {
			let who = ensure_signed(origin)?;

			// Return a successful DispatchResult
			Ok(())
		}
	}
}
