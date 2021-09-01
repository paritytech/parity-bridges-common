// Copyright 2019-2021 Parity Technologies (UK) Ltd.
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

//! Primitives of polkadot-like chains, that are related to parachains functionality.
//!
//! Even though this (bridges) repository references polkadot repository, we can't
//! reference polkadot crates from pallets. That's because bridges repository is
//! included in the polkadot repository and included pallets are used by polkadot
//! chains. Having pallets that are referencing polkadot, would mean that there may
//! be two versions of polkadot crates included in the runtime. Which is bad.

use sp_core::Hasher;
use sp_std::vec::Vec;

/// Parachain id.
pub type ParaId = u32;

/// Parachain head, which is (at least in Cumulus) a SCALE-encoded parachain header.
pub type ParaHead = Vec<u8>;

/// Parachain head hash.
pub type ParaHash = crate::Hash;

/// Raw storage proof of parachain heads, stored in polkadot-like chain runtime.
pub type ParachainHeadsProof = Vec<Vec<u8>>;

/// Return hash of the parachain head.
pub fn parachain_head_hash(head: &[u8]) -> ParaHash {
	sp_runtime::traits::BlakeTwo256::hash(head)
}
