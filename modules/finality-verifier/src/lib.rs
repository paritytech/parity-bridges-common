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

use frame_support::{decl_error, decl_module, decl_storage, dispatch::DispatchResult};

pub trait Config: frame_system::Config {}

decl_storage! {
	trait Store for Module<T: Config> as SubstrateBridge {}
}

decl_error! {
	pub enum Error for Module<T: Config> {}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		#[weight = 0]
		fn foo(origin) -> DispatchResult {
			todo!()
		}
	}
}

impl<T: Config> Module<T> {
	pub fn bar() {
		todo!()
	}
}
