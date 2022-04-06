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

//! Utitlites to build bridged chain and BEEFY+MMR structures.

use crate::mock::{
	sign_commitment, validator_key_to_public, validator_keys, BridgedBlockHash, BridgedBlockNumber,
	BridgedCommitment, BridgedHeader, BridgedMmrHasher, BridgedMmrLeaf, BridgedMmrNode,
	BridgedValidatorIdToMerkleLeaf, EXPECTED_MMR_LEAF_MAJOR_VERSION,
};

use beefy_primitives::mmr::{BeefyNextAuthoritySet, MmrLeafVersion};
use bp_beefy::{BeefyMmrProof, BeefyPayload, Commitment, ValidatorSetId, MMR_ROOT_PAYLOAD_ID};
use codec::Encode;
use libsecp256k1::SecretKey;
use pallet_mmr::NodeIndex;
use rand::Rng;
use sp_runtime::traits::{Convert, Header as HeaderT};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct HeaderAndCommitment {
	pub header: BridgedHeader,
	pub commitment: Option<BridgedCommitment>,
	pub leaf: Option<BridgedMmrLeaf>,
	pub leaf_proof: Option<BeefyMmrProof>,
}

impl HeaderAndCommitment {
	pub fn new(number: BridgedBlockNumber, parent_hash: BridgedBlockHash) -> Self {
		HeaderAndCommitment {
			header: BridgedHeader::new(
				number,
				Default::default(),
				Default::default(),
				parent_hash,
				Default::default(),
			),
			commitment: None,
			leaf: None,
			leaf_proof: None,
		}
	}
}

pub struct ChainBuilder {
	headers: Vec<HeaderAndCommitment>,
	validator_set_id: ValidatorSetId,
	validator_keys: Vec<SecretKey>,
	next_validator_keys: Vec<SecretKey>,
	mmr: mmr_lib::MMR<BridgedMmrNode, BridgedMmrHashMerge, BridgedMmrStorage>,
}

impl From<ChainBuilder> for HeaderAndCommitment {
	fn from(mut chain: ChainBuilder) -> HeaderAndCommitment {
		assert_eq!(chain.headers.len(), 1);
		chain.headers.remove(0)
	}
}

impl From<ChainBuilder> for Vec<HeaderAndCommitment> {
	fn from(chain: ChainBuilder) -> Vec<HeaderAndCommitment> {
		chain.headers
	}
}

struct BridgedMmrStorage {
	nodes: HashMap<NodeIndex, BridgedMmrNode>,
}

impl mmr_lib::MMRStore<BridgedMmrNode> for BridgedMmrStorage {
	fn get_elem(&self, pos: NodeIndex) -> mmr_lib::Result<Option<BridgedMmrNode>> {
		Ok(self.nodes.get(&pos).cloned())
	}

	fn append(&mut self, pos: NodeIndex, elems: Vec<BridgedMmrNode>) -> mmr_lib::Result<()> {
		for (i, elem) in elems.into_iter().enumerate() {
			self.nodes.insert(pos + i as NodeIndex, elem);
		}
		Ok(())
	}
}

impl ChainBuilder {
	/// Creates new chain builder with given validator set size.
	pub fn new(initial_validators_count: usize) -> Self {
		ChainBuilder {
			headers: Vec::new(),
			validator_set_id: 0,
			validator_keys: validator_keys(0, initial_validators_count),
			// validators for session 0 and 1 are always the same
			next_validator_keys: validator_keys(0, initial_validators_count),
			mmr: mmr_lib::MMR::new(0, BridgedMmrStorage { nodes: HashMap::new() }),
		}
	}

	/// Get header with given number.
	pub fn header(&self, number: BridgedBlockNumber) -> HeaderAndCommitment {
		self.headers[number as usize - 1].clone()
	}

	/// Append custom regular header using `HeaderBuilder`.
	pub fn custom_header(self) -> HeaderBuilder {
		let next_validator_set_id = self.validator_set_id + 1;
		let next_validator_keys = self.next_validator_keys.clone();
		HeaderBuilder::with_chain(self, false, next_validator_set_id, next_validator_keys)
	}

	/// Append custom handoff header using `HeaderBuilder`.
	pub fn custom_handoff_header(self, next_validators_len: usize) -> HeaderBuilder {
		let new_validator_set_id = self.validator_set_id + 2;
		let new_validator_keys = validator_keys(
			rand::thread_rng().gen::<usize>() % (usize::MAX / 2),
			next_validators_len,
		);
		HeaderBuilder::with_chain(self, true, new_validator_set_id, new_validator_keys)
	}

	/// Appends header, that has been finalized by BEEFY (so it has a linked signed commitment).
	pub fn append_finalized_header(self) -> Self {
		let next_validator_set_id = self.validator_set_id + 1;
		let next_validator_keys = self.next_validator_keys.clone();
		HeaderBuilder::with_chain(self, false, next_validator_set_id, next_validator_keys)
			.finalize()
		/*		self = self.append_default_header();

		let last_header = self.headers.last_mut().expect("added by append_header; qed");
		last_header.commitment = Some(sign_commitment(
			Commitment {
				payload: BeefyPayload::new(
					MMR_ROOT_PAYLOAD_ID,
					self.mmr.get_root().expect("TODO").hash().encode(),
				),
				block_number: *last_header.header.number(),
				validator_set_id: self.validator_set_id,
			},
			&self.validator_keys,
		));

		self*/
	}

	/// Append multiple finalized headers at once.
	pub fn append_finalized_headers(mut self, count: usize) -> Self {
		for _ in 0..count {
			self = self.append_finalized_header();
		}
		self
	}

	/// Appends header, that enacts new validator set.
	///
	/// Such headers are explicitly finalized by BEEFY.
	pub fn append_handoff_header(mut self, next_validators_len: usize) -> Self {
		let new_validator_set_id = self.validator_set_id + 2;
		let new_validator_keys = validator_keys(
			rand::thread_rng().gen::<usize>() % (usize::MAX / 2),
			next_validators_len,
		);

		self =
			HeaderBuilder::with_chain(self, true, new_validator_set_id, new_validator_keys.clone())
				.finalize();
		/*
				self = self.append_header(
					true,
					new_validator_set_id,
					new_validator_keys.clone(),
				);

				let last_header = self.headers.last_mut().expect("added by append_header; qed");
				last_header.commitment = Some(sign_commitment(
					Commitment {
						payload: BeefyPayload::new(
							MMR_ROOT_PAYLOAD_ID,
							self.mmr.get_root().expect("TODO").hash().encode(),
						),
						block_number: *last_header.header.number(),
						validator_set_id: self.validator_set_id,
					},
					&self.validator_keys,
				));
		*/
		/*
				self.validator_set_id = self.validator_set_id + 1;
				self.validator_keys = self.next_validator_keys;
				self.next_validator_keys = new_validator_keys;
		*/
		self
	}

	pub fn append_default_header(self) -> Self {
		/*		let next_validator_set_id = self.validator_set_id + 1;
		let next_validator_keys = self.next_validator_keys.clone();
		self.append_header(
			false,
			next_validator_set_id,
			next_validator_keys,
		)*/
		let next_validator_set_id = self.validator_set_id + 1;
		let next_validator_keys = self.next_validator_keys.clone();
		HeaderBuilder::with_chain(self, false, next_validator_set_id, next_validator_keys).build()
	}

	pub fn append_default_headers(mut self, count: usize) -> Self {
		for _ in 0..count {
			self = self.append_default_header();
		}
		self
	}
	/*
	fn append_header(
		mut self,
		handoff: bool,
		next_validator_set_id: ValidatorSetId,
		next_validator_keys: Vec<SecretKey>,
	) -> Self {
		let next_validator_publics = next_validator_keys
			.into_iter()
			.map(|k| {
				sp_core::ecdsa::Public::from_raw(validator_key_to_public(k).serialize_compressed())
					.into()
			})
			.collect::<Vec<_>>();
		let next_validator_addresses = next_validator_publics
			.iter()
			.cloned()
			.map(BridgedValidatorIdToMerkleLeaf::convert)
			.collect::<Vec<_>>();
		// we're starting with header#1, since header#0 is always finalized
		let header_number = self.headers.len() as BridgedBlockNumber + 1;
		let header = HeaderAndCommitment::new(
			header_number,
			self.headers.last().map(|h| h.header.hash()).unwrap_or_default(),
		);
		let raw_leaf = BridgedRawMmrLeaf {
			version: MmrLeafVersion::new(EXPECTED_MMR_LEAF_MAJOR_VERSION, 0),
			parent_number_and_hash: (
				header.header.number().saturating_sub(1),
				*header.header.parent_hash(),
			),
			beefy_next_authority_set: BeefyNextAuthoritySet {
				id: next_validator_set_id,
				len: next_validator_publics.len() as _,
				root: bp_beefy::beefy_merkle_root::<BridgedMmrHasher, _, _>(
					next_validator_addresses,
				),
			},
			parachain_heads: Default::default(), // TODO
		};
		let node = BridgedMmrNode::Data(raw_leaf.clone());
		log::trace!(
			target: "runtime::bridge-beefy",
			"Inserting MMR leaf with hash {} for header {}",
			node.hash(),
			header.header.number(),
		);
		let leaf_position = self.mmr.push(node).expect("TODO");
		self.headers.push(header);

		let last_header = self.headers.last_mut().expect("added one line above; qed");
		let proof = self.mmr.gen_proof(vec![leaf_position]).expect("TODO");
		last_header.leaf = Some(if !handoff {
			BridgedMmrLeaf::Regular(raw_leaf.encode())
		} else {
			BridgedMmrLeaf::Handoff(raw_leaf.encode(), next_validator_publics)
		});
		// genesis has no leaf => leaf index is header number minus 1
		let leaf_index = *last_header.header.number() - 1;
		let leaf_count = *last_header.header.number();
		let proof_size = proof.proof_items().len();
		last_header.leaf_proof = Some(MmrProof {
			leaf_index,
			leaf_count,
			items: proof.proof_items().iter().map(|i| i.hash().to_fixed_bytes()).collect(),
		});
		log::trace!(
			target: "runtime::bridge-beefy",
			"Proof of leaf {}/{} (for header {}) has {} items. Root: {}",
			leaf_index,
			leaf_count,
			last_header.header.number(),
			proof_size,
			self.mmr.get_root().expect("TODO").hash(),
		);

		self
	}*/
}

/// Custom header builder.
pub struct HeaderBuilder {
	chain: ChainBuilder,
	header: HeaderAndCommitment,
	new_validator_keys: Option<Vec<SecretKey>>,
}

impl HeaderBuilder {
	fn with_chain(
		chain: ChainBuilder,
		handoff: bool,
		next_validator_set_id: ValidatorSetId,
		next_validator_keys: Vec<SecretKey>,
	) -> Self {
		// we're starting with header#1, since header#0 is always finalized
		let header_number = chain.headers.len() as BridgedBlockNumber + 1;
		let mut header = HeaderAndCommitment::new(
			header_number,
			chain.headers.last().map(|h| h.header.hash()).unwrap_or_default(),
		);

		let next_validator_publics = next_validator_keys
			.iter()
			.cloned()
			.map(|k| {
				sp_core::ecdsa::Public::from_raw(validator_key_to_public(k).serialize_compressed())
					.into()
			})
			.collect::<Vec<_>>();
		let next_validator_addresses = next_validator_publics
			.iter()
			.cloned()
			.map(BridgedValidatorIdToMerkleLeaf::convert)
			.collect::<Vec<_>>();
		let raw_leaf = beefy_primitives::mmr::MmrLeaf {
			version: MmrLeafVersion::new(EXPECTED_MMR_LEAF_MAJOR_VERSION, 0),
			parent_number_and_hash: (
				header.header.number().saturating_sub(1),
				*header.header.parent_hash(),
			),
			beefy_next_authority_set: BeefyNextAuthoritySet {
				id: next_validator_set_id,
				len: next_validator_publics.len() as _,
				root: bp_beefy::beefy_merkle_root::<BridgedMmrHasher, _, _>(
					next_validator_addresses,
				),
			},
			parachain_heads: Default::default(), // TODO
		};
		header.leaf = Some(if !handoff {
			BridgedMmrLeaf::Regular(raw_leaf.encode())
		} else {
			BridgedMmrLeaf::Handoff(raw_leaf.encode(), next_validator_publics)
		});

		HeaderBuilder {
			chain,
			header,
			new_validator_keys: if handoff { Some(next_validator_keys) } else { None },
		}
	}

	pub fn customize_leaf(mut self, f: impl FnOnce(BridgedMmrLeaf) -> BridgedMmrLeaf) -> Self {
		let mut leaf = self.header.leaf.take().expect("set in constructor; qed");
		leaf = f(leaf);
		self.header.leaf = Some(leaf);
		self
	}

	pub fn customize_proof(mut self, f: impl FnOnce(BeefyMmrProof) -> BeefyMmrProof) -> Self {
		let raw_leaf = self.header.leaf.as_ref().expect("set in constructor; qed").leaf();
		let raw_leaf_hash = BridgedMmrHasher::hash(raw_leaf);
		let node = BridgedMmrNode::Hash(raw_leaf_hash.into());
		log::trace!(
			target: "runtime::bridge-beefy",
			"Inserting MMR leaf with hash {} for header {}",
			node.hash(),
			self.header.header.number(),
		);
		let leaf_position = self.chain.mmr.push(node).expect("TODO");

		let proof = self.chain.mmr.gen_proof(vec![leaf_position]).expect("TODO");
		// genesis has no leaf => leaf index is header number minus 1
		let leaf_index = *self.header.header.number() - 1;
		let leaf_count = *self.header.header.number();
		let proof_size = proof.proof_items().len();
		self.header.leaf_proof = Some(f(BeefyMmrProof {
			leaf_index,
			leaf_count,
			items: proof.proof_items().iter().map(|i| i.hash().to_fixed_bytes()).collect(),
		}));
		log::trace!(
			target: "runtime::bridge-beefy",
			"Proof of leaf {}/{} (for header {}) has {} items. Root: {}",
			leaf_index,
			leaf_count,
			self.header.header.number(),
			proof_size,
			self.chain.mmr.get_root().expect("TODO").hash(),
		);

		self
	}

	pub fn build(mut self) -> ChainBuilder {
		if self.header.leaf_proof.is_none() {
			self = self.customize_proof(|proof| proof);
		}

		if let Some(new_validator_keys) = self.new_validator_keys {
			self.chain.validator_set_id = self.chain.validator_set_id + 1;
			self.chain.validator_keys = self.chain.next_validator_keys;
			self.chain.next_validator_keys = new_validator_keys;
		}

		self.chain.headers.push(self.header);

		self.chain
	}

	pub fn finalize(self) -> ChainBuilder {
		let current_validator_set_id = self.chain.validator_set_id;
		let current_validator_set_keys = self.chain.validator_keys.clone();
		let mut chain = self.build();

		let last_header = chain.headers.last_mut().expect("added by append_header; qed");
		last_header.commitment = Some(sign_commitment(
			Commitment {
				payload: BeefyPayload::new(
					MMR_ROOT_PAYLOAD_ID,
					chain.mmr.get_root().expect("TODO").hash().encode(),
				),
				block_number: *last_header.header.number(),
				validator_set_id: current_validator_set_id,
			},
			&current_validator_set_keys,
		));

		chain
	}
}

/// Default Merging & Hashing behavior for MMR.
pub struct BridgedMmrHashMerge;

impl mmr_lib::Merge for BridgedMmrHashMerge {
	type Item = BridgedMmrNode;

	fn merge(left: &Self::Item, right: &Self::Item) -> Self::Item {
		let mut concat = left.hash().as_ref().to_vec();
		concat.extend_from_slice(right.hash().as_ref());

		BridgedMmrNode::Hash(BridgedMmrHasher::hash(&concat).into())
	}
}
