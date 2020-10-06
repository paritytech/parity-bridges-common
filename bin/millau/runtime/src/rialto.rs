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

//! Configuration parameters for the Rialto Substrate chain.

use bp_rialto::{BlockNumber, Header};
use pallet_substrate_bridge::storage::{AuthoritySet, ScheduledChange};
use sp_core::crypto::Public;
use sp_finality_grandpa::AuthorityId;
use sp_std::vec;

/// The first header known to the pallet.
///
/// Note that this does not need to be the genesis header of the Rialto
/// chain since the pallet may start at any arbirary header.
pub fn initial_header() -> Header {
	Header {
		parent_hash: Default::default(),
		number: Default::default(),
		state_root: Default::default(),
		extrinsics_root: Default::default(),
		digest: Default::default(),
	}
}

/// The first set of Grandpa authorities known to the pallet.
///
/// Note that this doesn't have to be the "genesis" authority set, as the
/// pallet can be configured to start from any height.
pub fn initial_authority_set() -> AuthoritySet {
	let set_id = 0;
	let authorities = vec![(AuthorityId::from_slice(&[1; 32]), 1)];
	AuthoritySet::new(authorities, set_id)
}

/// The first authority set change that the pallet should be aware of.
pub fn first_scheduled_change() -> ScheduledChange<BlockNumber> {
	let set_id = 1;
	let authorities = vec![(AuthorityId::from_slice(&[2; 32]), 1)];
	let first_change = AuthoritySet::new(authorities, set_id);

	let height = 3;
	ScheduledChange {
		authority_set: first_change,
		height,
	}
}
