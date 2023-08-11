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

//! Primitives of the Evochain.

#![warn(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]

mod evo_hash;

use bp_beefy::ChainWithBeefy;
use bp_header_chain::ChainWithGrandpa;
use bp_messages::{
	ChainWithMessages, InboundMessageDetails, LaneId, MessageNonce, MessagePayload,
	OutboundMessageDetails,
};
use bp_runtime::{decl_bridge_finality_runtime_apis, decl_bridge_runtime_apis, Chain, ChainId};
use frame_support::{
	dispatch::DispatchClass,
	weights::{constants::WEIGHT_REF_TIME_PER_SECOND, IdentityFee, Weight},
	RuntimeDebug,
};
use frame_system::limits;
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use sp_core::{storage::StateVersion, Hasher as HasherT};
use sp_runtime::{
	traits::{BlakeTwo256, Keccak256},
	MultiSignature,
};
use sp_runtime::{
	traits::{IdentifyAccount, Verify},
	MultiSigner, Perbill,
};
use sp_std::prelude::*;
use sp_trie::{LayoutV0, LayoutV1, TrieConfiguration};

pub use evo_hash::EvoHash;

/// Number of extra bytes (excluding size of storage value itself) of storage proof, built at
/// Evochain chain. This mostly depends on number of entries (and their density) in the storage trie.
/// Some reserve is reserved to account future chain growth.
pub const EXTRA_STORAGE_PROOF_SIZE: u32 = 1024;

/// Number of bytes, included in the signed Evochain transaction apart from the encoded call itself.
///
/// Can be computed by subtracting encoded call size from raw transaction size.
pub const TX_EXTRA_BYTES: u32 = 103;

/// Maximum weight of single Evochain block.
///
/// This represents 0.5 seconds of compute assuming a target block time of six seconds.
///
/// Max PoV size is set to max value, since it isn't important for relay/standalone chains.
pub const MAXIMUM_BLOCK_WEIGHT: Weight =
	Weight::from_parts(WEIGHT_REF_TIME_PER_SECOND.saturating_div(2), u64::MAX);

/// Represents the portion of a block that will be used by Normal extrinsics.
pub const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

/// Maximal number of unrewarded relayer entries in Evochain confirmation transaction.
pub const MAX_UNREWARDED_RELAYERS_IN_CONFIRMATION_TX: MessageNonce = 128;

/// Maximal number of unconfirmed messages in Evochain confirmation transaction.
pub const MAX_UNCONFIRMED_MESSAGES_IN_CONFIRMATION_TX: MessageNonce = 128;

/// The target length of a session (how often authorities change) on Evochain measured in of number of
/// blocks.
///
/// Note that since this is a target sessions may change before/after this time depending on network
/// conditions.
pub const SESSION_LENGTH: BlockNumber = 5 * time_units::MINUTES;

/// Maximal number of GRANDPA authorities at Evochain.
pub const MAX_AUTHORITIES_COUNT: u32 = 5;

/// Reasonable number of headers in the `votes_ancestries` on Evochain chain.
///
/// See [`bp-header-chain::ChainWithGrandpa`] for more details.
pub const REASONABLE_HEADERS_IN_JUSTIFICATON_ANCESTRY: u32 = 8;

/// Approximate average header size in `votes_ancestries` field of justification on Evochain chain.
///
/// See [`bp-header-chain::ChainWithGrandpa`] for more details.
pub const AVERAGE_HEADER_SIZE_IN_JUSTIFICATION: u32 = 256;

/// Approximate maximal header size on Evochain chain.
///
/// We expect maximal header to have digest item with the new authorities set for every consensus
/// engine (GRANDPA, Babe, BEEFY, ...) - so we multiply it by 3. And also
/// `AVERAGE_HEADER_SIZE_IN_JUSTIFICATION` bytes for other stuff.
///
/// See [`bp-header-chain::ChainWithGrandpa`] for more details.
pub const MAX_HEADER_SIZE: u32 = MAX_AUTHORITIES_COUNT
	.saturating_mul(3)
	.saturating_add(AVERAGE_HEADER_SIZE_IN_JUSTIFICATION);

/// Re-export `time_units` to make usage easier.
pub use time_units::*;

/// Human readable time units defined in terms of number of blocks.
pub mod time_units {
	use super::BlockNumber;

	/// Milliseconds between Evochain chain blocks.
	pub const MILLISECS_PER_BLOCK: u64 = 6000;
	/// Slot duration in Evochain chain consensus algorithms.
	pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;

	/// A minute, expressed in Evochain chain blocks.
	pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
	/// A hour, expressed in Evochain chain blocks.
	pub const HOURS: BlockNumber = MINUTES * 60;
	/// A day, expressed in Evochain chain blocks.
	pub const DAYS: BlockNumber = HOURS * 24;
}

/// Block number type used in Evochain.
pub type BlockNumber = u64;

/// Hash type used in Evochain.
pub type Hash = sp_core::H256;

/// Type of object that can produce hashes on Evochain.
pub type Hasher = BlakeTwo256;

/// The header type used by Evochain.
pub type Header = sp_runtime::generic::Header<BlockNumber, Hasher>;

/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it equivalent
/// to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// Public key of the chain account that may be used to verify signatures.
pub type AccountSigner = MultiSigner;

/// Balance of an account.
pub type Balance = u64;

/// Nonce of a transaction in the chain.
pub type Nonce = u32;

/// Weight-to-Fee type used by Evochain.
pub type WeightToFee = IdentityFee<Balance>;

/// Evochain chain.
#[derive(RuntimeDebug)]
pub struct Evochain;

impl Chain for Evochain {
	const ID: ChainId = *b"evol";

	type BlockNumber = BlockNumber;
	type Hash = Hash;
	type Hasher = Hasher;
	type Header = Header;

	type AccountId = AccountId;
	type Balance = Balance;
	type Nonce = Nonce;
	type Signature = Signature;

	const STATE_VERSION: StateVersion = StateVersion::V0;

	fn max_extrinsic_size() -> u32 {
		*BlockLength::get().max.get(DispatchClass::Normal)
	}

	fn max_extrinsic_weight() -> Weight {
		BlockWeights::get()
			.get(DispatchClass::Normal)
			.max_extrinsic
			.unwrap_or(Weight::MAX)
	}
}

impl ChainWithGrandpa for Evochain {
	const WITH_CHAIN_GRANDPA_PALLET_NAME: &'static str = WITH_EVOCHAIN_GRANDPA_PALLET_NAME;
	const MAX_AUTHORITIES_COUNT: u32 = MAX_AUTHORITIES_COUNT;
	const REASONABLE_HEADERS_IN_JUSTIFICATON_ANCESTRY: u32 =
		REASONABLE_HEADERS_IN_JUSTIFICATON_ANCESTRY;
	const MAX_HEADER_SIZE: u32 = MAX_HEADER_SIZE;
	const AVERAGE_HEADER_SIZE_IN_JUSTIFICATION: u32 = AVERAGE_HEADER_SIZE_IN_JUSTIFICATION;
}

impl ChainWithBeefy for Evochain {
	type CommitmentHasher = Keccak256;
	type MmrHashing = Keccak256;
	type MmrHash = <Keccak256 as sp_runtime::traits::Hash>::Output;
	type BeefyMmrLeafExtra = ();
	type AuthorityId = bp_beefy::EcdsaValidatorId;
	type AuthorityIdToMerkleLeaf = bp_beefy::BeefyEcdsaToEthereum;
}

impl ChainWithMessages for Evochain {
	const WITH_CHAIN_MESSAGES_PALLET_NAME: &'static str = WITH_EVOCHAIN_MESSAGES_PALLET_NAME;

	const MAX_UNREWARDED_RELAYERS_IN_CONFIRMATION_TX: MessageNonce =
		MAX_UNREWARDED_RELAYERS_IN_CONFIRMATION_TX;
	const MAX_UNCONFIRMED_MESSAGES_IN_CONFIRMATION_TX: MessageNonce =
		MAX_UNCONFIRMED_MESSAGES_IN_CONFIRMATION_TX;
}

frame_support::parameter_types! {
	/// Size limit of the Evochain blocks.
	pub BlockLength: limits::BlockLength =
		limits::BlockLength::max_with_normal_ratio(2 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
	/// Weight limit of the Evochain blocks.
	pub BlockWeights: limits::BlockWeights =
		limits::BlockWeights::with_sensible_defaults(MAXIMUM_BLOCK_WEIGHT, NORMAL_DISPATCH_RATIO);
}

/// Name of the With-Evochain GRANDPA pallet instance that is deployed at bridged chains.
pub const WITH_EVOCHAIN_GRANDPA_PALLET_NAME: &str = "BridgeEvochainGrandpa";
/// Name of the With-Evochain messages pallet instance that is deployed at bridged chains.
pub const WITH_EVOCHAIN_MESSAGES_PALLET_NAME: &str = "BridgeEvochainMessages";
/// Name of the transaction payment pallet at the Evochain runtime.
pub const TRANSACTION_PAYMENT_PALLET_NAME: &str = "TransactionPayment";

decl_bridge_runtime_apis!(evochain, grandpa);
