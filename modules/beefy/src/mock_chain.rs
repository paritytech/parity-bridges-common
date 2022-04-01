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
	BridgedCommitment, BridgedCommitmentHasher, BridgedHeader, BridgedMmrHasher, BridgedMmrLeaf,
	BridgedMmrNode, BridgedRawMmrLeaf, BridgedValidatorSet,
};

use beefy_primitives::mmr::{BeefyNextAuthoritySet, MmrLeafVersion};
use bp_beefy::{
	BeefyMmrProof, BeefyPayload, Commitment, MmrProof, ValidatorSetId, MMR_ROOT_PAYLOAD_ID,
};
use codec::Encode;
use libsecp256k1::{sign, Message, SecretKey};
use pallet_mmr::NodeIndex;
use rand::Rng;
use sp_runtime::traits::{Hash as _, Header as _};
use std::collections::{BTreeSet, HashMap};

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

	/// Appends header, that has been finalized by BEEFY (so it has a linked signed commitment).
	pub fn append_finalized_header(mut self) -> Self {
		self = self.append_default_header();

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
		self = self.append_header(true, new_validator_set_id, new_validator_keys.clone());

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

		self.validator_set_id = self.validator_set_id + 1;
		self.validator_keys = self.next_validator_keys;
		self.next_validator_keys = new_validator_keys;

		self
	}

	pub fn append_default_header(mut self) -> Self {
		let next_validator_set_id = self.validator_set_id + 1;
		let next_validator_keys = self.next_validator_keys.clone();
		self.append_header(false, next_validator_set_id, next_validator_keys)
	}

	pub fn append_default_headers(mut self, count: usize) -> Self {
		for i in 0..count {
			self = self.append_default_header();
		}
		self
	}

	fn append_header(
		mut self,
		handoff: bool,
		next_validator_set_id: ValidatorSetId,
		next_validator_keys: Vec<SecretKey>,
	) -> Self {
		let header = HeaderAndCommitment::new(
			self.headers.len() as BridgedBlockNumber,
			self.headers.last().map(|h| h.header.hash()).unwrap_or_default(),
		);
		let raw_leaf = beefy_primitives::mmr::MmrLeaf {
			version: MmrLeafVersion::new(0, 0),
			parent_number_and_hash: (
				header.header.number().saturating_sub(1),
				*header.header.parent_hash(),
			),
			beefy_next_authority_set: BeefyNextAuthoritySet {
				id: next_validator_set_id,
				len: next_validator_keys.len() as _,
				root: Default::default(), // TODO
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
			BridgedMmrLeaf::Regular(raw_leaf)
		} else {
			BridgedMmrLeaf::Handoff(
				raw_leaf,
				next_validator_keys
					.into_iter()
					.map(|k| {
						sp_core::ecdsa::Public::from_raw(
							validator_key_to_public(k).serialize_compressed(),
						)
						.into()
					})
					.collect(),
			)
		});
		let leaf_index = *last_header.header.number();
		let leaf_count = *last_header.header.number() + 1;
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
