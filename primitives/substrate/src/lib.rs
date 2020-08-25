// Copyright 2019-2020 Parity Technologies (UK) Ltd.
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

#![cfg_attr(not(feature = "std"), no_std)]

use core::default::Default;
use parity_scale_codec::{Decode, Encode};
use sp_finality_grandpa::{AuthorityList, SetId};
use sp_runtime::traits::Header as HeaderT;

#[derive(Default, Encode, Decode)]
pub struct AuthoritySet {
	pub authorities: AuthorityList,
	pub set_id: SetId,
}

#[derive(Default, Encode, Decode)]
pub struct ScheduledChange<N> {
	pub authority_set: AuthoritySet,
	pub height: N,
}

#[derive(Default, Encode, Decode)]
pub struct ImportedHeader<H: HeaderT> {
	pub header: H,
	pub is_finalized: bool,
}

pub fn prove_finality<H>(_header: &H, _set: &AuthoritySet, _justification: &[u8]) -> bool {
	true
}
