// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

//! Module with configuration which reflects PeoplePolkadot runtime setup
//! (AccountId, Headers, Hashes...)

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use bp_bridge_hub_cumulus::*;
use bp_messages::*;
use bp_runtime::{
	decl_bridge_finality_runtime_apis, decl_bridge_messages_runtime_apis, Chain, ChainId, Parachain,
};
use frame_support::dispatch::DispatchClass;
use sp_runtime::{RuntimeDebug, StateVersion};

/// PeoplePolkadot parachain.
#[derive(RuntimeDebug)]
pub struct PeoplePolkadot;

impl Chain for PeoplePolkadot {
	const ID: ChainId = *b"phpd";
	const STATE_VERSION: StateVersion = StateVersion::V1;

	type BlockNumber = BlockNumber;
	type Hash = Hash;
	type Hasher = Hasher;
	type Header = Header;

	type AccountId = AccountId;
	type Balance = Balance;
	type Nonce = Nonce;
	type Signature = Signature;

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

impl Parachain for PeoplePolkadot {
	const PARACHAIN_ID: u32 = PEOPLE_POLKADOT_PARACHAIN_ID;
	const MAX_HEADER_SIZE: u32 = MAX_BRIDGE_HUB_HEADER_SIZE;
}

impl ChainWithMessages for PeoplePolkadot {
	const WITH_CHAIN_MESSAGES_PALLET_NAME: &'static str =
		WITH_PEOPLE_POLKADOT_MESSAGES_PALLET_NAME;
	const MAX_UNREWARDED_RELAYERS_IN_CONFIRMATION_TX: MessageNonce =
		MAX_UNREWARDED_RELAYERS_IN_CONFIRMATION_TX;
	/// This constant limits the maximum number of messages in `receive_messages_proof`.
	/// We need to adjust it from 4096 to 2024 due to the actual weights identified by
	/// `check_message_lane_weights`. A higher value can be set once we switch
	/// `max_extrinsic_weight` to `BlockWeightsForAsyncBacking`.
	const MAX_UNCONFIRMED_MESSAGES_IN_CONFIRMATION_TX: MessageNonce = 2024;
}

/// Identifier of PeoplePolkadot in the Polkadot relay chain.
pub const PEOPLE_POLKADOT_PARACHAIN_ID: u32 = 1004;

/// Name of the With-PeoplePolkadot messages pallet instance that is deployed at bridged chains.
pub const WITH_PEOPLE_POLKADOT_MESSAGES_PALLET_NAME: &str = "BridgePolkadotMessages";

/// Pallet index of `BridgePolkadotBulletinMessages: pallet_bridge_messages::<Instance1>`.
pub const WITH_PEOPLE_POLKADOT_TO_BULLETIN_MESSAGES_PALLET_INDEX: u8 = 61;

decl_bridge_finality_runtime_apis!(people_polkadot);
decl_bridge_messages_runtime_apis!(people_polkadot, LegacyLaneId);
