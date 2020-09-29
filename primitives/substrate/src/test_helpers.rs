// Copyright 2019-2020 Parity Technologies (UK) Ltd.
// This file is part of Parity Bridges Common.

// Parity Bridges Common is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
// Parity Bridges Common is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity Bridges Common.  If not, see <http://www.gnu.org/licenses/>.

#![cfg(test)]

use finality_grandpa::voter_set::VoterSet;
use sp_finality_grandpa::{AuthorityId, AuthorityList};
use sp_keyring::Ed25519Keyring;
use sp_runtime::testing::{Header, H256};
use sp_std::vec;

// pub type TestHeader = <TestRuntime as Trait>::BridgedHeader;
// pub type TestNumber = <TestRuntime as Trait>::BridgedBlockNumber;
// pub type TestHash = <TestRuntime as Trait>::BridgedBlockHash;
pub type HeaderId = (H256, u64);

pub fn test_header(num: u64) -> Header {
	let mut header = Header::new_from_number(num);
	header.parent_hash = if num == 0 {
		Default::default()
	} else {
		test_header(num - 1).hash()
	};

	header
}

pub fn header_id(index: u8) -> HeaderId {
	(test_header(index.into()).hash(), index as _)
}

pub fn extract_keyring(id: &AuthorityId) -> Ed25519Keyring {
	let mut raw_public = [0; 32];
	raw_public.copy_from_slice(id.as_ref());
	Ed25519Keyring::from_raw_public(raw_public).unwrap()
}

pub fn voter_set() -> VoterSet<AuthorityId> {
	VoterSet::new(authority_list()).unwrap()
}

pub fn authority_list() -> AuthorityList {
	vec![(alice(), 1), (bob(), 1), (charlie(), 1)]
}

pub fn alice() -> AuthorityId {
	Ed25519Keyring::Alice.public().into()
}

pub fn bob() -> AuthorityId {
	Ed25519Keyring::Bob.public().into()
}

pub fn charlie() -> AuthorityId {
	Ed25519Keyring::Charlie.public().into()
}
