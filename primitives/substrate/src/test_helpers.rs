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

use finality_grandpa::voter_set::VoterSet;
use sp_finality_grandpa::{AuthorityId, AuthorityList, AuthoritySignature, AuthorityWeight, SetId};
use sp_keyring::Ed25519Keyring;
use sp_runtime::testing::{Header, H256};
use sp_runtime::traits::Header as _;
use sp_std::vec;

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

pub fn signed_precommit(
	signer: Ed25519Keyring,
	target: HeaderId,
	round: u64,
	set_id: SetId,
) -> finality_grandpa::SignedPrecommit<H256, u64, AuthoritySignature, AuthorityId> {
	let precommit = finality_grandpa::Precommit {
		target_hash: target.0,
		target_number: target.1,
	};
	let encoded =
		sp_finality_grandpa::localized_payload(round, set_id, &finality_grandpa::Message::Precommit(precommit.clone()));
	let signature = signer.sign(&encoded[..]).into();
	finality_grandpa::SignedPrecommit {
		precommit,
		signature,
		id: signer.public().into(),
	}
}

pub fn make_justification_for_header(
	header: &Header,
	round: u64,
	set_id: SetId,
	authorities: &[(AuthorityId, AuthorityWeight)],
) -> crate::justification::GrandpaJustification<Header> {
	let (target_hash, target_number) = (header.hash(), *header.number());
	let mut precommits = vec![];
	let mut votes_ancestries = vec![];

	// We want to make sure that the header included in the vote ancestries
	// is actually related to our target header
	let mut precommit_header = test_header(target_number + 1);
	precommit_header.parent_hash = target_hash;

	// I'm using the same header for all the voters since it doesn't matter as long
	// as they all vote on blocks _ahead_ of the one we're interested in finalizing
	for (id, _weight) in authorities.iter() {
		let signer = extract_keyring(&id);
		let precommit = signed_precommit(
			signer,
			(precommit_header.hash(), *precommit_header.number()),
			round,
			set_id,
		);
		precommits.push(precommit);
		votes_ancestries.push(precommit_header.clone());
	}

	crate::justification::GrandpaJustification {
		round,
		commit: finality_grandpa::Commit {
			target_hash,
			target_number,
			precommits,
		},
		votes_ancestries,
	}
}
