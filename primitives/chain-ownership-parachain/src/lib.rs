//! Primitives of the Ownership parachain.

#![warn(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]

use bp_messages::{
	ChainWithMessages, InboundMessageDetails, LaneId, MessageNonce, MessagePayload,
	OutboundMessageDetails,
};
use bp_runtime::{decl_bridge_runtime_apis, Chain, ChainId, Parachain};
use frame_support::{
	dispatch::DispatchClass,
	weights::{constants::WEIGHT_REF_TIME_PER_SECOND, IdentityFee, Weight},
	RuntimeDebug, StateVersion,
};
use frame_system::limits;
use sp_core::Hasher as HasherT;
use sp_runtime::{
	traits::{BlakeTwo256, IdentifyAccount, Verify},
	MultiSignature, MultiSigner, Perbill,
};
use sp_std::vec::Vec;

/// Identifier of OwnershipParachain in the Rococo/Polkadot relay chain.
///
/// This identifier is not something that is declared either by Ownership or OwnershipParachain. This
/// is an identifier of registration. So in theory it may be changed. But since bridge is going
/// to be deployed after parachain registration AND since parachain de-registration is highly
/// likely impossible, it is fine to declare this constant here.
pub const OWNERSHIP_PARACHAIN_ID: u32 = 1000;

/// Number of extra bytes (excluding size of storage value itself) of storage proof, built at
/// OwnershipParachain chain. This mostly depends on number of entries (and their density) in the
/// storage trie. Some reserve is reserved to account future chain growth.
pub const EXTRA_STORAGE_PROOF_SIZE: u32 = 1024;

/// Can be computed by subtracting encoded call size from raw transaction size.
pub const TX_EXTRA_BYTES: u32 = 104;

/// Maximal number of unrewarded relayer entries in Ownership chain confirmation transaction.
pub const MAX_UNREWARDED_RELAYERS_IN_CONFIRMATION_TX: MessageNonce = 1024;

/// Maximal number of unconfirmed messages in Ownership chain confirmation transaction.
pub const MAX_UNCONFIRMED_MESSAGES_IN_CONFIRMATION_TX: MessageNonce = 1024;

/// Ownership parachain.
#[derive(RuntimeDebug)]
pub struct OwnershipParachain;

impl Chain for OwnershipParachain {
	const ID: ChainId = *b"ownp";

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

impl Parachain for OwnershipParachain {
	const PARACHAIN_ID: u32 = OWNERSHIP_PARACHAIN_ID;
}

impl ChainWithMessages for OwnershipParachain {
	const WITH_CHAIN_MESSAGES_PALLET_NAME: &'static str =
		WITH_OWNERSHIP_PARACHAIN_MESSAGES_PALLET_NAME;
	const MAX_UNREWARDED_RELAYERS_IN_CONFIRMATION_TX: MessageNonce =
		MAX_UNREWARDED_RELAYERS_IN_CONFIRMATION_TX;
	const MAX_UNCONFIRMED_MESSAGES_IN_CONFIRMATION_TX: MessageNonce =
		MAX_UNCONFIRMED_MESSAGES_IN_CONFIRMATION_TX;
}

// Technically this is incorrect, because ownership-parachain isn't a bridge hub, but we're
// trying to keep it close to the bridge hubs code (at least in this aspect).
pub use bp_bridge_hub_cumulus::SignedExtension;

/// Name of the With-Ownership-Parachain messages pallet instance that is deployed at bridged chains.
pub const WITH_OWNERSHIP_PARACHAIN_MESSAGES_PALLET_NAME: &str = "BridgeOwnershipParachainMessages";
/// Name of the transaction payment pallet at the Ownership parachain runtime.
pub const TRANSACTION_PAYMENT_PALLET_NAME: &str = "TransactionPayment";

decl_bridge_runtime_apis!(ownership_parachain);
