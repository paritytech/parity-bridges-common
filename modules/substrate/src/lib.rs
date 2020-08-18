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

// I think to start I want to expose one method: import_header(). This will only
// accept finalized blocks. This will be a sort of minimum viable bridge in a sense.
// We will only store finalized blocks. This will give us a chance to develop a finality
// chain interface.
//
// Once this is done we can move on to a "full" headerchain/light client. This could then
// add things like tracking different forks. This would also need to implement some sort
// of pruning mechanism once we accept a finalized block.

#![cfg_attr(not(feature = "std"), no_std)]
// Runtime-generated enums
#![allow(clippy::large_enum_variant)]

use bp_header_chain::{BaseHeaderChain, FinalityHeaderChain, FullHeaderChain};
use frame_support::{decl_error, decl_module, decl_storage, dispatch, traits::Get};
use frame_system::{ensure_none, ensure_signed};

pub trait Trait: frame_system::Trait {
	type Blockchain: BaseHeaderChain + FullHeaderChain<Self::AccountId> + FinalityHeaderChain;
}

decl_storage! {
	trait Store for Module<T: Trait> as SubstrateBridge {
		// Maybe make this a HeaderId?
		BestHeader: T::Hash;
		// Best finalized header we know of.
		BestFinalized: T::Hash;
		// Headers which have been imported into the runtime.
		// Maybe made a HeaderId?
		ImportedHeaders: map hasher(identity) T::Hash => Option<T::Header>;
	}
}

decl_error! {
	pub enum Error for Module<T: Trait> {
		/// Errors should have helpful documentation associated with them.
		UnfinalizedHeader,
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		#[weight = 0]
		pub fn import_signed_header(
			origin,
			header: <T::Blockchain as FullHeaderChain<T::AccountId>>::Header,
			extra_data: Option<<T::Blockchain as FullHeaderChain<T::AccountId>>::Extra>,
		) -> dispatch::DispatchResult {
			let who = ensure_signed(origin)?;

			let successful = T::Blockchain::import_header(Some(who), header, extra_data);

			if !successful {
				return Err(<Error<T>>::UnfinalizedHeader.into())
			}

			Ok(())
		}
	}
}
