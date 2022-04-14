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

//! Primitives that are used to interact with BEEFY bridge pallet.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

pub use beefy_merkle_tree::{
	merkle_root as beefy_merkle_root, Hasher as BeefyMmrHasher, Keccak256 as BeefyKeccak256,
};
pub use beefy_primitives::{
	crypto::AuthorityId as EcdsaValidatorId, known_payload_ids::MMR_ROOT_ID as MMR_ROOT_PAYLOAD_ID,
	mmr::MmrLeafVersion, Commitment, Payload as BeefyPayload, SignedCommitment, ValidatorSet,
	ValidatorSetId, BEEFY_ENGINE_ID,
};
pub use pallet_beefy_mmr::BeefyEcdsaToEthereum;
pub use pallet_mmr::verify_leaf_proof as verify_mmr_leaf_proof;
pub use pallet_mmr_primitives::{DataOrHash as MmrDataOrHash, Proof as MmrProof};

use bp_runtime::{BlockNumberOf, Chain, HashOf};
use codec::{Decode, Encode};
use frame_support::Parameter;
use scale_info::TypeInfo;
use sp_core::H256;
use sp_runtime::{
	app_crypto::RuntimeAppPublic,
	traits::{Convert, MaybeSerializeDeserialize},
	RuntimeDebug,
};
use sp_std::prelude::*;

pub mod storage_keys;

/// Substrate-based chain with BEEFY && MMR pallets deployed.
///
/// Both BEEFY and MMR pallets and their clients may be configured to use different
/// primitives. Some of types can be configured in low-level pallets, but are constrained
/// when BEEFY+MMR bundle is used.
pub trait ChainWithBeefy: Chain {
	/// Hash algorithm used to compute digest of the BEEFY commitment.
	///
	/// Corresponds to the hashing algorithm, used `beefy_gadget::BeefyKeystore`.
	type CommitmentHasher: sp_runtime::traits::Hash;

	/// Hash algorithm used to build MMR.
	///
	/// Corresponds to the `Hashing` field` of the `pallet-mmr` configuration. In BEEFY+MMR
	/// bundle, its output is hardcoded to be `H256` (see `beefy_merkle_tree::Hash` trait).
	///
	/// The same algorithm is also used to compute merkle roots in BEEFY - e.g. `parachain_heads`
	/// and validator addresses root in leaf data.
	type MmrHasher: beefy_merkle_tree::Hasher + Send + Sync;

	/// A way to identify BEEFY validator and verify its signature.
	///
	/// Corresponds to the `BeefyId` field of the `pallet-beefy` configuration.
	type ValidatorId: BeefyRuntimeAppPublic<<Self::CommitmentHasher as sp_runtime::traits::Hash>::Output>
		+ Parameter
		+ MaybeSerializeDeserialize
		+ Send
		+ Sync;

	/// A way to convert validator id to its raw representation in the BEEFY merkle tree.
	///
	/// Corresponds to the `BeefyAuthorityToMerkleLeaf` field of the `pallet-beefy-mmr`
	/// configuration.
	type ValidatorIdToMerkleLeaf: Convert<Self::ValidatorId, Vec<u8>>;
}

/// Extended version of `RuntimeAppPublic`, which is able to verify signature of pre-hashed
/// message. Regular `RuntimeAppPublic` is hashing message itself (using `blake2`), which
/// is not how things work in BEEFY.
pub trait BeefyRuntimeAppPublic<CommitmentHash>: RuntimeAppPublic {
	/// Verify a signature on a pre-hashed message. Return `true` if the signature is valid
	/// and thus matches the given `public` key.
	fn verify_prehashed(&self, sig: &Self::Signature, msg_hash: &CommitmentHash) -> bool;
}

// this implementation allows to bridge with BEEFY chains, that are using default (eth-compatible)
// BEEFY configuration
impl BeefyRuntimeAppPublic<H256> for beefy_primitives::crypto::AuthorityId {
	fn verify_prehashed(&self, sig: &Self::Signature, msg_hash: &H256) -> bool {
		use sp_application_crypto::AppKey;
		static_assertions::assert_type_eq_all!(
			<<beefy_primitives::crypto::AuthorityId as RuntimeAppPublic>::Signature as AppKey>::UntypedGeneric,
			sp_core::ecdsa::Signature,
		);
		static_assertions::assert_type_eq_all!(
			<beefy_primitives::crypto::AuthorityId as AppKey>::UntypedGeneric,
			sp_core::ecdsa::Public,
		);

		// why it is here:
		//
		// 1) we need to call `sp_io::crypto::ecdsa_verify_prehashed` to be sure that the host
		// function is    used to verify signature;
		// 2) there's no explicit conversions from app-crypto sig+key types to matching underlying
		// types; 3) `ecdsa_verify_prehashed` works with underlying ECDSA types;
		// 4) hence this "convert".
		const PROOF: &'static str =
			"static assertion guarantees that both underlying types are equal; \
			conversion between same types can't fail; \
			qed";
		let ecdsa_signature = sp_core::ecdsa::Signature::try_from(sig.as_ref()).expect(PROOF);
		let ecdsa_public = sp_core::ecdsa::Public::try_from(self.as_ref()).expect(PROOF);
		sp_io::crypto::ecdsa_verify_prehashed(
			&ecdsa_signature,
			msg_hash.as_fixed_bytes(),
			&ecdsa_public,
		)
	}
}

/// BEEFY validator id used by given Substrate chain.
pub type BeefyValidatorIdOf<C> = <C as ChainWithBeefy>::ValidatorId;

/// BEEFY validator signature used by given Substrate chain.
pub type BeefyValidatorSignatureOf<C> =
	<<C as ChainWithBeefy>::ValidatorId as RuntimeAppPublic>::Signature;

/// Signed BEEFY commitment used by given Substrate chain.
pub type BeefySignedCommitmentOf<C> =
	SignedCommitment<BlockNumberOf<C>, BeefyValidatorSignatureOf<C>>;

/// BEEFY validator set, containing both validator identifiers and the numeric set id.
pub type BeefyValidatorSetOf<C> = ValidatorSet<BeefyValidatorIdOf<C>>;

/// Hash algorithm, used to compute digest of the BEEFY commitment before validators are signing the
/// commitment.
pub type BeefyCommitmentHasher<C> = <C as ChainWithBeefy>::CommitmentHasher;

/// unpacked BEEFY MMR leaf contents.
///
/// See `BeefyMmrLeafUnpacked` for details.
pub type BeefyMmrLeafUnpackedOf<C> = BeefyMmrLeafUnpacked<BeefyValidatorIdOf<C>>;

/// BEEFY version of MMR leaf proof.
///
/// Even though original struct supports different hash types, we're constraining it with the
/// hash type, used by BEEFY.
pub type BeefyMmrProof = MmrProof<BeefyMmrHash>;

/// Hash algorithm used in MMR construction by given Substrate chain.
pub type BeefyMmrHasherOf<C> = <C as ChainWithBeefy>::MmrHasher;

/// Hash type, used in MMR construction at the chain with BEEFY support.
pub type BeefyMmrHash = beefy_merkle_tree::Hash;

/// A way to convert validator id to its raw representation in the BEEFY merkle tree, used by given
/// Substrate chain.
pub type BeefyValidatorIdToMerkleLeafOf<C> = <C as ChainWithBeefy>::ValidatorIdToMerkleLeaf;

/// Actual type of leafs in the BEEFY MMR.
pub type BeefyMmrLeafOf<C> =
	beefy_primitives::mmr::MmrLeaf<BlockNumberOf<C>, HashOf<C>, BeefyMmrHash>;

/// MMR leaf with unpacked validators set when they're changed.
///
/// There are two options on how to deal with validator set in the BEEFY client. The first one is
/// when instead of storing public keys of all validators, the commitment is submitted with public
/// validator keys and proof-of-membership for every such key. Another one is when we're actually
/// receiving public keys of all validators when validator set changes and are immediately verifying
/// all these keys against validators merkle root. This makes the handoff procedure more heavy,
/// but all subsequent operations on the same set are cheaper.
#[derive(Encode, Decode, RuntimeDebug, PartialEq, Eq, Clone, TypeInfo)]
pub enum BeefyMmrLeafUnpacked<BeefyValidatorId> {
	/// This variant shall be used when containing MMR leaf is not signalling BEEFY authorities
	/// change.
	///
	/// The vector is encoded MMR leaf contents (`beefy_primitives::mmr::MmrLeaf`). We can't
	/// use it here directly, because leaf structure may change in the future.
	Regular(Vec<u8>),
	/// This variant shall be used when containing MMR leaf is signalling BEEFY authorities change.
	///
	/// The vector is encoded MMR leaf contents (`beefy_primitives::mmr::MmrLeaf`). We can't
	/// use it here directly, because leaf structure may change in the future.
	///
	/// The pallet will reject this variant if MMR leaf is not changing authorities.
	Handoff(Vec<u8>, Vec<BeefyValidatorId>),
}

impl<BeefyValidatorId> BeefyMmrLeafUnpacked<BeefyValidatorId> {
	/// Returns reference to the actual MMR leaf contents.
	pub fn leaf(&self) -> &[u8] {
		match *self {
			BeefyMmrLeafUnpacked::Regular(ref leaf) => leaf,
			BeefyMmrLeafUnpacked::Handoff(ref leaf, _) => leaf,
		}
	}

	/// Returns reference to the next validator set, if available.
	pub fn next_validators(&self) -> Option<&Vec<BeefyValidatorId>> {
		match *self {
			BeefyMmrLeafUnpacked::Regular(_) => None,
			BeefyMmrLeafUnpacked::Handoff(_, ref next_validators) => Some(next_validators),
		}
	}

	/// Converts self to unpacked next validator set, if available.
	pub fn into_next_validators(self) -> Option<Vec<BeefyValidatorId>> {
		match self {
			BeefyMmrLeafUnpacked::Regular(_) => None,
			BeefyMmrLeafUnpacked::Handoff(_, next_validators) => Some(next_validators),
		}
	}

	/// Set actual MMR leaf contents.
	pub fn set_leaf(self, new_raw_leaf: Vec<u8>) -> Self {
		match self {
			BeefyMmrLeafUnpacked::Regular(_) => BeefyMmrLeafUnpacked::Regular(new_raw_leaf),
			BeefyMmrLeafUnpacked::Handoff(_, next_validators) =>
				BeefyMmrLeafUnpacked::Handoff(new_raw_leaf, next_validators),
		}
	}

	/// Set next validator set.
	pub fn set_next_validators(self, next_validators: Option<Vec<BeefyValidatorId>>) -> Self {
		let raw_leaf = match self {
			BeefyMmrLeafUnpacked::Regular(raw_leaf) => raw_leaf,
			BeefyMmrLeafUnpacked::Handoff(raw_leaf, _) => raw_leaf,
		};
		match next_validators {
			Some(next_validators) => BeefyMmrLeafUnpacked::Handoff(raw_leaf, next_validators),
			None => BeefyMmrLeafUnpacked::Regular(raw_leaf),
		}
	}
}

/// Data required for initializing the BEEFY pallet.
#[derive(Encode, Decode, RuntimeDebug, PartialEq, Eq, Clone, TypeInfo)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
pub struct InitializationData<BlockNumber, ValidatorId> {
	/// Should the pallet block transaction immediately after initialization.
	pub is_halted: bool,
	/// Number of the best block, finalized by BEEFY.
	pub best_beefy_block_number: BlockNumber,
	/// BEEFY validator set that will be finalizing descendants of the `best_beefy_block_number`
	/// block.
	pub current_validator_set: (ValidatorSetId, Vec<ValidatorId>),
	/// Next BEEFY validator set, that we'll switch to, once we see the handoff header.
	pub next_validator_set: (ValidatorSetId, Vec<ValidatorId>),
}

/// Basic data, stored by the pallet for every imported commitment.
#[derive(Encode, Decode, RuntimeDebug, PartialEq, Eq, Clone, TypeInfo)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
pub struct ImportedCommitment<BlockNumber, BlockHash> {
	/// Block number and hash of the finalized block parent.
	pub parent_number_and_hash: (BlockNumber, BlockHash),
	/// MMR root at the imported block.
	pub mmr_root: BeefyMmrHash,
	/// Parachain heads merkle root at the imported block.
	pub parachain_heads: BeefyMmrHash,
}
