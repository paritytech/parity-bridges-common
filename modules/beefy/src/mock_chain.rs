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

//! Utilities to build bridged chain and BEEFY+MMR structures.

use crate::mock::{
	parachain_heads, sign_commitment, validator_key_to_public, validator_keys, BridgedBlockNumber,
	BridgedCommitment, BridgedHeader, BridgedMmrHasher, BridgedMmrLeaf, BridgedMmrNode,
	BridgedValidatorIdToMerkleLeaf, EXPECTED_MMR_LEAF_MAJOR_VERSION,
};

use beefy_primitives::mmr::{BeefyNextAuthoritySet, MmrLeafVersion};
use bp_beefy::{
	BeefyMmrHash, BeefyMmrProof, BeefyPayload, Commitment, ValidatorSetId, MMR_ROOT_PAYLOAD_ID,
};
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
	pub leaf: BridgedMmrLeaf,
	pub leaf_proof: BeefyMmrProof,
	pub mmr_root: BeefyMmrHash,
}

pub struct ChainBuilder {
	headers: Vec<HeaderAndCommitment>,
	validator_set_id: ValidatorSetId,
	validator_keys: Vec<SecretKey>,
	next_validator_keys: Vec<SecretKey>,
	mmr: mmr_lib::MMR<BridgedMmrNode, BridgedMmrHashMerge, BridgedMmrStorage>,
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

	/// Returns single built header.
	pub fn to_header(&self) -> HeaderAndCommitment {
		assert_eq!(self.headers.len(), 1);
		self.headers[0].clone()
	}

	/// Returns built chain.
	pub fn to_chain(&self) -> Vec<HeaderAndCommitment> {
		self.headers.clone()
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
	pub fn append_handoff_header(self, next_validators_len: usize) -> Self {
		let new_validator_set_id = self.validator_set_id + 2;
		let new_validator_keys = validator_keys(
			rand::thread_rng().gen::<usize>() % (usize::MAX / 2),
			next_validators_len,
		);

		HeaderBuilder::with_chain(self, true, new_validator_set_id, new_validator_keys.clone())
			.finalize()
	}

	/// Append single default header without commitment.
	pub fn append_default_header(self) -> Self {
		let next_validator_set_id = self.validator_set_id + 1;
		let next_validator_keys = self.next_validator_keys.clone();
		HeaderBuilder::with_chain(self, false, next_validator_set_id, next_validator_keys).build()
	}

	/// Append several default header without commitment.
	pub fn append_default_headers(mut self, count: usize) -> Self {
		for _ in 0..count {
			self = self.append_default_header();
		}
		self
	}
}

/// Custom header builder.
pub struct HeaderBuilder {
	chain: ChainBuilder,
	header: BridgedHeader,
	leaf: BridgedMmrLeaf,
	leaf_proof: Option<BeefyMmrProof>,
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
		let header = BridgedHeader::new(
			header_number,
			Default::default(),
			Default::default(),
			chain.headers.last().map(|h| h.header.hash()).unwrap_or_default(),
			Default::default(),
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
			parent_number_and_hash: (header.number().saturating_sub(1), *header.parent_hash()),
			beefy_next_authority_set: BeefyNextAuthoritySet {
				id: next_validator_set_id,
				len: next_validator_publics.len() as _,
				root: bp_beefy::beefy_merkle_root::<BridgedMmrHasher, _, _>(
					next_validator_addresses,
				),
			},
			leaf_extra: parachain_heads(&header),
		};

		HeaderBuilder {
			chain,
			header,
			leaf: if !handoff {
				BridgedMmrLeaf::Regular(raw_leaf.encode())
			} else {
				BridgedMmrLeaf::Handoff(raw_leaf.encode(), next_validator_publics)
			},
			leaf_proof: None,
			new_validator_keys: if handoff { Some(next_validator_keys) } else { None },
		}
	}

	/// Customize header MMR leaf.
	pub fn customize_leaf(mut self, f: impl FnOnce(BridgedMmrLeaf) -> BridgedMmrLeaf) -> Self {
		self.leaf = f(self.leaf);
		self
	}

	/// Customize generated proof of header MMR leaf.
	///
	/// Can only be called once.
	pub fn customize_proof(mut self, f: impl FnOnce(BeefyMmrProof) -> BeefyMmrProof) -> Self {
		assert!(self.leaf_proof.is_none());

		let raw_leaf_hash = BridgedMmrHasher::hash(self.leaf.leaf());
		let node = BridgedMmrNode::Hash(raw_leaf_hash.into());
		log::trace!(
			target: "runtime::bridge-beefy",
			"Inserting MMR leaf with hash {} for header {}",
			node.hash(),
			self.header.number(),
		);
		let leaf_position = self.chain.mmr.push(node).unwrap();

		let proof = self.chain.mmr.gen_proof(vec![leaf_position]).unwrap();
		// genesis has no leaf => leaf index is header number minus 1
		let leaf_index = *self.header.number() - 1;
		let leaf_count = *self.header.number();
		let proof_size = proof.proof_items().len();
		self.leaf_proof = Some(f(BeefyMmrProof {
			leaf_index,
			leaf_count,
			items: proof.proof_items().iter().map(|i| i.hash().to_fixed_bytes()).collect(),
		}));
		log::trace!(
			target: "runtime::bridge-beefy",
			"Proof of leaf {}/{} (for header {}) has {} items. Root: {}",
			leaf_index,
			leaf_count,
			self.header.number(),
			proof_size,
			self.chain.mmr.get_root().unwrap().hash(),
		);

		self
	}

	/// Build header without commitment.
	pub fn build(mut self) -> ChainBuilder {
		if self.leaf_proof.is_none() {
			self = self.customize_proof(|proof| proof);
		}

		if let Some(new_validator_keys) = self.new_validator_keys {
			self.chain.validator_set_id = self.chain.validator_set_id + 1;
			self.chain.validator_keys = self.chain.next_validator_keys;
			self.chain.next_validator_keys = new_validator_keys;
		}

		self.chain.headers.push(HeaderAndCommitment {
			header: self.header,
			commitment: None,
			leaf: self.leaf,
			leaf_proof: self.leaf_proof.expect("guaranteed by the customize_proof call above; qed"),
			mmr_root: self.chain.mmr.get_root().unwrap().hash().into(),
		});

		self.chain
	}

	/// Build header with commitment.
	pub fn finalize(self) -> ChainBuilder {
		let current_validator_set_id = self.chain.validator_set_id;
		let current_validator_set_keys = self.chain.validator_keys.clone();
		let mut chain = self.build();

		let last_header = chain.headers.last_mut().expect("added by append_header; qed");
		last_header.commitment = Some(sign_commitment(
			Commitment {
				payload: BeefyPayload::new(
					MMR_ROOT_PAYLOAD_ID,
					chain.mmr.get_root().unwrap().hash().encode(),
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
