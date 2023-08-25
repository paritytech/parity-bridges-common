//! Bridge primitives of the Evochain.

#![warn(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]

// Re-export core Evochain primitives.
pub use evochain_primitives::*;

use bp_header_chain::ChainWithGrandpa;
use bp_messages::{
	ChainWithMessages, InboundMessageDetails, LaneId, MessageNonce, MessagePayload,
	OutboundMessageDetails,
};
use bp_runtime::{decl_bridge_finality_runtime_apis, decl_bridge_runtime_apis, Chain, ChainId};
use evochain_primitives::{
	AccountId, Balance, BlockLength, BlockNumber, BlockWeights, Hash, Hasher, Header, Nonce,
	Signature, MINUTES,
};
use frame_support::{dispatch::DispatchClass, weights::Weight, RuntimeDebug};
use sp_core::storage::StateVersion;
use sp_std::prelude::*;

/// Number of extra bytes (excluding size of storage value itself) of storage proof, built at
/// Evochain chain. This mostly depends on number of entries (and their density) in the storage trie.
/// Some reserve is reserved to account future chain growth.
pub const EXTRA_STORAGE_PROOF_SIZE: u32 = 1024;

/// Number of bytes, included in the signed Evochain transaction apart from the encoded call itself.
///
/// Can be computed by subtracting encoded call size from raw transaction size.
pub const TX_EXTRA_BYTES: u32 = 103;

/// Maximal number of unrewarded relayer entries in Evochain confirmation transaction.
pub const MAX_UNREWARDED_RELAYERS_IN_CONFIRMATION_TX: MessageNonce = 128;

/// Maximal number of unconfirmed messages in Evochain confirmation transaction.
pub const MAX_UNCONFIRMED_MESSAGES_IN_CONFIRMATION_TX: MessageNonce = 128;

/// The target length of a session (how often authorities change) on Evochain measured in of number of
/// blocks.
///
/// Note that since this is a target sessions may change before/after this time depending on network
/// conditions.
pub const SESSION_LENGTH: BlockNumber = 5 * MINUTES;

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

impl ChainWithMessages for Evochain {
	const WITH_CHAIN_MESSAGES_PALLET_NAME: &'static str = WITH_EVOCHAIN_MESSAGES_PALLET_NAME;

	const MAX_UNREWARDED_RELAYERS_IN_CONFIRMATION_TX: MessageNonce =
		MAX_UNREWARDED_RELAYERS_IN_CONFIRMATION_TX;
	const MAX_UNCONFIRMED_MESSAGES_IN_CONFIRMATION_TX: MessageNonce =
		MAX_UNCONFIRMED_MESSAGES_IN_CONFIRMATION_TX;
}

/// Name of the With-Evochain GRANDPA pallet instance that is deployed at bridged chains.
pub const WITH_EVOCHAIN_GRANDPA_PALLET_NAME: &str = "BridgeEvochainGrandpa";
/// Name of the With-Evochain messages pallet instance that is deployed at bridged chains.
pub const WITH_EVOCHAIN_MESSAGES_PALLET_NAME: &str = "BridgeEvochainMessages";
/// Name of the transaction payment pallet at the Evochain runtime.
pub const TRANSACTION_PAYMENT_PALLET_NAME: &str = "TransactionPayment";

decl_bridge_runtime_apis!(evochain, grandpa);
