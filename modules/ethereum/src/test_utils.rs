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

use crate::validators::step_validator;
use crate::verification::calculate_score;
use primitives::{rlp_encode, Address, Bloom, Header, SealedEmptyStep, H256, H520, U256};
use secp256k1::{Message, PublicKey, SecretKey};

/// Gas limit valid in test environment.
pub const GAS_LIMIT: u64 = 0x2000;

/// Test header builder.
pub struct HeaderBuilder {
	header: Header,
	parent_header: Header,
}

impl HeaderBuilder {
	/// Creates default genesis header.
	pub fn genesis() -> Self {
		let current_step = 0u64;
		Self {
			header: Header {
				gas_limit: GAS_LIMIT.into(),
				seal: vec![primitives::rlp_encode(&current_step), vec![]],
				..Default::default()
			},
			parent_header: Default::default(),
		}
	}

	/// Creates default header on top of parent with given hash.
	#[cfg(test)]
	pub fn with_parent_hash(parent_hash: H256) -> Self {
		use crate::mock::TestRuntime;
		use crate::Headers;
		use frame_support::StorageMap;

		let parent_header = Headers::<TestRuntime>::get(&parent_hash).unwrap().header;
		Self::with_parent(&parent_header)
	}

	/// Creates default header on top of parent with given number. First parent is selected.
	#[cfg(test)]
	pub fn with_parent_number(parent_number: u64) -> Self {
		use crate::HeadersByNumber;
		use frame_support::StorageMap;

		let parent_hash = HeadersByNumber::get(parent_number).unwrap()[0].clone();
		Self::with_parent_hash(parent_hash)
	}

	/// Creates default header on top of non-existent parent.
	#[cfg(test)]
	pub fn with_number(number: u64) -> Self {
		Self::with_parent(&Header {
			number: number - 1,
			seal: vec![primitives::rlp_encode(&(number - 1)), vec![]],
			..Default::default()
		})
	}

	/// Creates default header on top of given parent.
	pub fn with_parent(parent_header: &Header) -> Self {
		let parent_step = parent_header.step().unwrap();
		let current_step = parent_step + 1;
		Self {
			header: Header {
				parent_hash: parent_header.compute_hash(),
				number: parent_header.number + 1,
				gas_limit: GAS_LIMIT.into(),
				seal: vec![primitives::rlp_encode(&current_step), vec![]],
				difficulty: calculate_score(parent_step, current_step, 0),
				..Default::default()
			},
			parent_header: parent_header.clone(),
		}
	}

	/// Update step of this header.
	pub fn step(mut self, step: u64) -> Self {
		let parent_step = self.parent_header.step();
		self.header.seal[0] = rlp_encode(&step);
		self.header.difficulty = parent_step
			.map(|parent_step| calculate_score(parent_step, step, 0))
			.unwrap_or_default();
		self
	}

	/// Adds empty steps to this header.
	pub fn empty_steps(mut self, empty_steps: &[(&SecretKey, u64)]) -> Self {
		let sealed_empty_steps = empty_steps
			.into_iter()
			.map(|(author, step)| {
				let mut empty_step = SealedEmptyStep {
					step: *step,
					signature: Default::default(),
				};
				let message = empty_step.message(&self.header.parent_hash);
				let signature: [u8; 65] = sign(author, message).into();
				empty_step.signature = signature.into();
				empty_step
			})
			.collect::<Vec<_>>();

		// by default in test configuration headers are generated without empty steps seal
		if self.header.seal.len() < 3 {
			self.header.seal.push(Vec::new());
		}

		self.header.seal[2] = SealedEmptyStep::rlp_of(&sealed_empty_steps);
		self
	}

	/// Update difficulty field of this header.
	pub fn difficulty(mut self, difficulty: U256) -> Self {
		self.header.difficulty = difficulty;
		self
	}

	/// Update extra data field of this header.
	pub fn extra_data(mut self, extra_data: Vec<u8>) -> Self {
		self.header.extra_data = extra_data;
		self
	}

	/// Update gas limit field of this header.
	pub fn gas_limit(mut self, gas_limit: U256) -> Self {
		self.header.gas_limit = gas_limit;
		self
	}

	/// Update gas used field of this header.
	pub fn gas_used(mut self, gas_used: U256) -> Self {
		self.header.gas_used = gas_used;
		self
	}

	/// Update log bloom field of this header.
	pub fn log_bloom(mut self, log_bloom: Bloom) -> Self {
		self.header.log_bloom = log_bloom;
		self
	}

	/// Update receipts root field of this header.
	pub fn receipts_root(mut self, receipts_root: H256) -> Self {
		self.header.receipts_root = receipts_root;
		self
	}

	/// Update timestamp field of this header.
	pub fn timestamp(mut self, timestamp: u64) -> Self {
		self.header.timestamp = timestamp;
		self
	}

	/// Signs header by given author.
	pub fn sign_by(mut self, author: &SecretKey) -> Header {
		self.header.author = secret_to_address(author);

		let message = self.header.seal_hash(false).unwrap();
		let signature = sign(author, message);
		self.header.seal[1] = rlp_encode(&signature);
		self.header
	}

	/// Signs header by given authors set.
	pub fn sign_by_set(self, authors: &[SecretKey]) -> Header {
		let step = self.header.step().unwrap();
		let author = step_validator(authors, step);
		self.sign_by(author)
	}
}

/// Returns address correspnding to given secret key.
pub fn secret_to_address(secret: &SecretKey) -> Address {
	let public = PublicKey::from_secret_key(secret);
	let mut raw_public = [0u8; 64];
	raw_public.copy_from_slice(&public.serialize()[1..]);
	primitives::public_to_address(&raw_public)
}

/// Return author's signature over given message.
pub fn sign(author: &SecretKey, message: H256) -> H520 {
	let (signature, recovery_id) = secp256k1::sign(&Message::parse(message.as_fixed_bytes()), author);
	let mut raw_signature = [0u8; 65];
	raw_signature[..64].copy_from_slice(&signature.serialize());
	raw_signature[64] = recovery_id.serialize();
	raw_signature.into()
}
