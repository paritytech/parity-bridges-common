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

//! Utilities for working with test accounts.

use ed25519_dalek::{Keypair, PublicKey, SecretKey, Signature};
use finality_grandpa::voter_set::VoterSet;
use sp_application_crypto::Public;
use sp_finality_grandpa::{AuthorityId, AuthorityList};
use sp_runtime::RuntimeDebug;

/// Used to indicate if a type is able to cryptographically sign messages.
pub trait Signer {
	fn public(&self) -> PublicKey {
		(&self.secret()).into()
	}

	fn secret(&self) -> SecretKey;

	fn pair(&self) -> Keypair {
		let mut pair: [u8; 64] = [0; 64];

		let secret = self.secret();
		pair[..32].copy_from_slice(&secret.to_bytes());

		let public = self.public();
		pair[32..].copy_from_slice(&public.to_bytes());

		Keypair::from_bytes(&pair).expect("We expect the SecretKey to be good, so this must also be good.")
	}

	fn sign(&self, msg: &[u8]) -> Signature {
		use ed25519_dalek::Signer;
		self.pair().sign(msg)
	}
}

/// Set of test accounts with friendly names.
#[derive(RuntimeDebug, Clone, Copy)]
pub enum TestKeyring {
	Alice,
	Bob,
	Charlie,
	Dave,
	Eve,
	Ferdie,
}

impl Signer for TestKeyring {
	fn secret(&self) -> SecretKey {
		SecretKey::from_bytes(&[*self as u8; 32]).expect("A static array of the correct length is a known good.")
	}
}

/// A test account which can be used to sign messages.
#[derive(RuntimeDebug, Clone, Copy)]
pub struct Account(pub u8);

impl Signer for Account {
	fn secret(&self) -> SecretKey {
		SecretKey::from_bytes(&[self.0; 32]).expect("A static array of the correct length is a known good.")
	}
}

impl From<TestKeyring> for AuthorityId {
	fn from(k: TestKeyring) -> Self {
		AuthorityId::from_slice(&k.public().to_bytes())
	}
}

impl From<Account> for AuthorityId {
	fn from(p: Account) -> Self {
		AuthorityId::from_slice(&p.public().to_bytes())
	}
}

/// Get a valid set of voters for a Grandpa round.
pub fn voter_set() -> VoterSet<AuthorityId> {
	VoterSet::new(authority_list()).unwrap()
}

/// Convenience function to get a list of Grandpa authorities.
pub fn authority_list() -> AuthorityList {
	test_keyring()
		.iter()
		.map(|(id, w)| (AuthorityId::from(*id), *w))
		.collect()
}

/// Get the corresponding identities from the keyring for the "standard" authority set.
pub fn test_keyring() -> Vec<(TestKeyring, u64)> {
	vec![
		(TestKeyring::Alice, 1),
		(TestKeyring::Bob, 1),
		(TestKeyring::Charlie, 1),
	]
}

/// Get a list of "unique" accounts.
pub fn accounts(len: u8) -> Vec<Account> {
	let mut v = vec![];
	for i in 0..len {
		v.push(Account(i));
	}

	v
}
