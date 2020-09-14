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

//! Configuration paramters for a generic Substrate chain.
//!
//! Should be replaced in the future with parameters for a specific
//! chain, e.g Millau.

use crate::{BlockNumber, Header};
use bp_substrate::{AuthoritySet, ScheduledChange};
use sp_core::crypto::Public;
use sp_finality_grandpa::AuthorityId;
use sp_std::vec;

pub fn genesis_header() -> Header {
	Header {
		parent_hash: Default::default(),
		number: Default::default(),
		state_root: Default::default(),
		extrinsics_root: Default::default(),
		digest: Default::default(),
	}
}

pub fn initial_authority_set() -> AuthoritySet {
	let set_id = 0;
	let authorities = vec![alice()];
	AuthoritySet::new(authorities, set_id)
}

pub fn first_scheduled_change() -> ScheduledChange<BlockNumber> {
	let set_id = 1;
	let authorities = vec![bob()];
	let first_change = AuthoritySet::new(authorities, set_id);

	let height = 3;
	ScheduledChange::new(first_change, height)
}

fn alice() -> (AuthorityId, u64) {
	(AuthorityId::from_slice(&[1; 32]), 1)
}

fn bob() -> (AuthorityId, u64) {
	(AuthorityId::from_slice(&[2; 32]), 1)
}
