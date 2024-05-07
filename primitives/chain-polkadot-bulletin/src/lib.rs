// Copyright (C) Parity Technologies (UK) Ltd.
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

//! Polkadot Bulletin Chain primitives.

#![warn(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]

use bp_header_chain::ChainWithGrandpa;
use bp_messages::{ChainWithMessages, MessageNonce};
use bp_runtime::{
	decl_bridge_finality_runtime_apis, decl_bridge_messages_runtime_apis, Chain, ChainId,
};
use frame_support::{
	dispatch::DispatchClass,
	parameter_types,
	weights::{constants::WEIGHT_REF_TIME_PER_SECOND, Weight},
};
use frame_system::limits;
use sp_runtime::{Perbill, StateVersion};

// This chain reuses most of Polkadot primitives.
pub use bp_polkadot_core::{
	AccountId, Balance, BlockNumber, Hash, Hasher, Header, Nonce, Signature,
	AVERAGE_HEADER_SIZE_IN_JUSTIFICATION, MAX_HEADER_SIZE,
	REASONABLE_HEADERS_IN_JUSTIFICATON_ANCESTRY,
};

/// Maximal number of GRANDPA authorities at Polkadot Bulletin chain.
pub const MAX_AUTHORITIES_COUNT: u32 = 100;

/// Name of the With-Polkadot Bulletin chain GRANDPA pallet instance that is deployed at bridged
/// chains.
pub const WITH_POLKADOT_BULLETIN_GRANDPA_PALLET_NAME: &str = "BridgePolkadotBulletinGrandpa";
/// Name of the With-Polkadot Bulletin chain messages pallet instance that is deployed at bridged
/// chains.
pub const WITH_POLKADOT_BULLETIN_MESSAGES_PALLET_NAME: &str = "BridgePolkadotBulletinMessages";

// There are fewer system operations on this chain (e.g. staking, governance, etc.). Use a higher
// percentage of the block for data storage.
const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(90);

// Re following constants - we are using the same values at Cumulus parachains. They are limited
// by the maximal transaction weight/size. Since block limits at Bulletin Chain are larger than
// at the Cumulus Bridgeg Hubs, we could reuse the same values.

/// Maximal number of unrewarded relayer entries at inbound lane for Cumulus-based parachains.
pub const MAX_UNREWARDED_RELAYERS_IN_CONFIRMATION_TX: MessageNonce = 1024;

/// Maximal number of unconfirmed messages at inbound lane for Cumulus-based parachains.
pub const MAX_UNCONFIRMED_MESSAGES_IN_CONFIRMATION_TX: MessageNonce = 4096;

parameter_types! {
	/// We allow for 2 seconds of compute with a 6 second average block time.
	pub BlockWeights: limits::BlockWeights = limits::BlockWeights::with_sensible_defaults(
			Weight::from_parts(2u64 * WEIGHT_REF_TIME_PER_SECOND, u64::MAX),
			NORMAL_DISPATCH_RATIO,
		);
	// Note: Max transaction size is 8 MB. Set max block size to 10 MB to facilitate data storage.
	// This is double the "normal" Relay Chain block length limit.
	/// Maximal block length at Polkadot Bulletin chain.
	pub BlockLength: limits::BlockLength = limits::BlockLength::max_with_normal_ratio(
		10 * 1024 * 1024,
		NORMAL_DISPATCH_RATIO,
	);
}

/// Polkadot Bulletin Chain declaration.
pub struct PolkadotBulletin;

impl Chain for PolkadotBulletin {
	const ID: ChainId = *b"pdbc";

	type BlockNumber = BlockNumber;
	type Hash = Hash;
	type Hasher = Hasher;
	type Header = Header;

	type AccountId = AccountId;
	type Balance = Balance;
	type Nonce = Nonce;
	type Signature = Signature;

	const STATE_VERSION: StateVersion = StateVersion::V1;

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

impl ChainWithGrandpa for PolkadotBulletin {
	const WITH_CHAIN_GRANDPA_PALLET_NAME: &'static str = WITH_POLKADOT_BULLETIN_GRANDPA_PALLET_NAME;
	const MAX_AUTHORITIES_COUNT: u32 = MAX_AUTHORITIES_COUNT;
	const REASONABLE_HEADERS_IN_JUSTIFICATON_ANCESTRY: u32 =
		REASONABLE_HEADERS_IN_JUSTIFICATON_ANCESTRY;
	const MAX_HEADER_SIZE: u32 = MAX_HEADER_SIZE;
	const AVERAGE_HEADER_SIZE_IN_JUSTIFICATION: u32 = AVERAGE_HEADER_SIZE_IN_JUSTIFICATION;
}

impl ChainWithMessages for PolkadotBulletin {
	const WITH_CHAIN_MESSAGES_PALLET_NAME: &'static str =
		WITH_POLKADOT_BULLETIN_MESSAGES_PALLET_NAME;

	const MAX_UNREWARDED_RELAYERS_IN_CONFIRMATION_TX: MessageNonce =
		MAX_UNREWARDED_RELAYERS_IN_CONFIRMATION_TX;
	const MAX_UNCONFIRMED_MESSAGES_IN_CONFIRMATION_TX: MessageNonce =
		MAX_UNCONFIRMED_MESSAGES_IN_CONFIRMATION_TX;
}

decl_bridge_finality_runtime_apis!(polkadot_bulletin);
decl_bridge_messages_runtime_apis!(polkadot_bulletin);
