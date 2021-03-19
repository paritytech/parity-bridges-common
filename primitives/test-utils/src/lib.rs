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

//! Utilities for testing runtime code.

#![cfg_attr(not(feature = "std"), no_std)]

use bp_header_chain::justification::GrandpaJustification;
use ed25519_dalek::{Keypair, PublicKey, SecretKey, Signature, Signer};
use finality_grandpa::voter_set::VoterSet;
use sp_application_crypto::{Public, TryFrom};
use sp_finality_grandpa::{AuthorityId, AuthorityList, AuthorityWeight};
use sp_finality_grandpa::{AuthoritySignature, SetId};
use sp_runtime::traits::{Header as HeaderT, One, Zero};
use sp_runtime::RuntimeDebug;
use sp_std::prelude::*;

pub const TEST_GRANDPA_ROUND: u64 = 1;
pub const TEST_GRANDPA_SET_ID: SetId = 1;

/// Make a valid GRANDPA justification with sensible defaults
pub fn make_default_justification<H: HeaderT>(header: &H) -> GrandpaJustification<H> {
	make_justification_for_header(header, TEST_GRANDPA_ROUND, TEST_GRANDPA_SET_ID, &keyring(), 2, 1)
}

/// Generate justifications in a way where we are able to tune the number of pre-commits
/// and vote ancestries which are included in the justification.
///
/// This is useful for benchmarkings where we want to generate valid justifications with
/// a specific number of pre-commits (tuned with the "forks" parameter) and/or a specific
/// number of vote ancestries (tuned with the "depth" parameter).
///
/// Note: This needs at least three authorities or else the verifier will complain about
/// being given an invalid commit.
pub fn make_justification_for_header<H: HeaderT>(
	header: &H,
	round: u64,
	set_id: SetId,
	authorities: &[(Keyring, AuthorityWeight)],
	depth: u32,
	forks: u32,
) -> GrandpaJustification<H> {
	let (target_hash, target_number) = (header.hash(), *header.number());
	let mut precommits = vec![];
	let mut votes_ancestries = vec![];

	assert!(
		forks as usize <= authorities.len(),
		"If we have more forks than authorities we can't create valid pre-commits for all the forks."
	);

	let mut chains = vec![];
	for i in 0..forks {
		let chain = generate_chain(i as u8, depth, header);

		// We don't include our finality target header in the vote ancestries
		for child in &chain[1..] {
			votes_ancestries.push(child.clone());
		}

		chains.push(chain);
	}

	for (i, (id, _weight)) in authorities.iter().enumerate() {
		// Assign chains to authorities in a round-robin fashion
		let chain = chains[i % forks as usize].clone();

		// The header we need to use when pre-commiting is the one at the highest height
		// on our chain.
		let (precommit_hash, precommit_number) = chain.last().map(|h| (h.hash(), *h.number())).unwrap();

		let precommit = signed_precommit::<H>(&id, (precommit_hash, precommit_number), round, set_id);
		precommits.push(precommit);
	}

	GrandpaJustification {
		round,
		commit: finality_grandpa::Commit {
			target_hash,
			target_number,
			precommits,
		},
		votes_ancestries,
	}
}

fn generate_chain<H: HeaderT>(fork_id: u8, depth: u32, ancestor: &H) -> Vec<H> {
	let mut headers = vec![];
	headers.push(ancestor.clone());

	for i in 1..depth {
		let parent = &headers[(i - 1) as usize];
		let (hash, num) = (parent.hash(), *parent.number());

		let mut precommit_header = test_header::<H>(num + One::one());
		precommit_header.set_parent_hash(hash);

		// Modifying the digest so headers at the same height but in different forks have different
		// hashes
		let digest = precommit_header.digest_mut();
		*digest = sp_runtime::Digest {
			logs: vec![sp_runtime::DigestItem::Other(vec![fork_id])],
		};

		headers.push(precommit_header);
	}

	headers
}

fn signed_precommit<H: HeaderT>(
	signer: &Keyring,
	target: (H::Hash, H::Number),
	round: u64,
	set_id: SetId,
) -> finality_grandpa::SignedPrecommit<H::Hash, H::Number, AuthoritySignature, AuthorityId> {
	let precommit = finality_grandpa::Precommit {
		target_hash: target.0,
		target_number: target.1,
	};

	let encoded =
		sp_finality_grandpa::localized_payload(round, set_id, &finality_grandpa::Message::Precommit(precommit.clone()));

	let signature = signer.pair().sign(&encoded);
	let raw_signature: Vec<u8> = signature.to_bytes().into();

	// Need to wrap our signature and id types that they match what our `SignedPrecommit` is expecting
	let signature = AuthoritySignature::try_from(raw_signature).expect(
		"We know our Keypair is good,
		so our signature must also be good.",
	);
	let id = (*signer).into();

	finality_grandpa::SignedPrecommit {
		precommit,
		signature,
		id,
	}
}

/// Get a header for testing.
///
/// The correct parent hash will be used if given a non-zero header.
pub fn test_header<H: HeaderT>(number: H::Number) -> H {
	let mut header = H::new(
		number,
		Default::default(),
		Default::default(),
		Default::default(),
		Default::default(),
	);

	if number != Zero::zero() {
		let parent_hash = test_header::<H>(number - One::one()).hash();
		header.set_parent_hash(parent_hash);
	}

	header
}

/// Convenience function for generating a Header ID at a given block number.
pub fn header_id<H: HeaderT>(index: u8) -> (H::Hash, H::Number) {
	(test_header::<H>(index.into()).hash(), index.into())
}

/// Set of test accounts.
#[derive(RuntimeDebug, Clone, Copy)]
pub enum Keyring {
	Alice,
	Bob,
	Charlie,
	Dave,
	Eve,
	Ferdie,
}

impl Keyring {
	pub fn public(&self) -> PublicKey {
		(&self.secret()).into()
	}

	pub fn secret(&self) -> SecretKey {
		SecretKey::from_bytes(&[*self as u8; 32]).expect("A static array of the correct length is a known good.")
	}

	pub fn pair(self) -> Keypair {
		let mut pair: [u8; 64] = [0; 64];

		let secret = self.secret();
		pair[..32].copy_from_slice(&secret.to_bytes());

		let public = self.public();
		pair[32..].copy_from_slice(&public.to_bytes());

		Keypair::from_bytes(&pair).expect("We expect the SecretKey to be good, so this must also be good.")
	}

	pub fn sign(self, msg: &[u8]) -> Signature {
		self.pair().sign(msg)
	}
}

impl From<Keyring> for AuthorityId {
	fn from(k: Keyring) -> Self {
		AuthorityId::from_slice(&k.public().to_bytes())
	}
}

/// Get a valid set of voters for a Grandpa round.
pub fn voter_set() -> VoterSet<AuthorityId> {
	VoterSet::new(authority_list()).unwrap()
}

/// Convenience function to get a list of Grandpa authorities.
pub fn authority_list() -> AuthorityList {
	keyring().iter().map(|(id, w)| (AuthorityId::from(*id), *w)).collect()
}

/// Get the corresponding identities from the keyring for the "standard" authority set.
pub fn keyring() -> Vec<(Keyring, u64)> {
	vec![(Keyring::Alice, 1), (Keyring::Bob, 1), (Keyring::Charlie, 1)]
}
