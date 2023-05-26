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

//! Defines traits which represent a common interface for Substrate pallets which want to
//! incorporate bridge functionality.

#![cfg_attr(not(feature = "std"), no_std)]

use bp_runtime::{BasicOperatingMode, Chain, HeaderOf};
use codec::{Decode, Encode};
use core::{clone::Clone, cmp::Eq, default::Default, fmt::Debug};
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{traits::Header as HeaderT, ConsensusEngineId, KeyTypeId, RuntimeDebug};
use sp_std::boxed::Box;

pub mod aleph_justification;
pub mod storage_keys;

pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"alp0");
pub const ALEPH_ENGINE_ID: ConsensusEngineId = *b"FRNK";

mod app {
	use sp_application_crypto::{app_crypto, ed25519};
	app_crypto!(ed25519, crate::KEY_TYPE);
}

pub type AuthorityId = app::Public;
pub type AuthoritySignature = app::Signature;
pub type AuthoritySet = Vec<AuthorityId>;

/// Data required for initializing the Aleph bridge pallet.
///
/// The bridge needs to know where to start its sync from, and this provides that initial context.
#[derive(Default, Encode, Decode, RuntimeDebug, PartialEq, Eq, Clone, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct InitializationData<H: HeaderT> {
	/// The header from which we should start syncing.
	pub header: Box<H>,
	/// The initial authorities of the pallet.
	pub authority_list: AuthoritySet,
	/// Pallet operating mode.
	pub operating_mode: BasicOperatingMode,
}

/// A minimized version of `pallet-bridge-aleph::Call` that can be used without a runtime.
#[derive(Encode, Decode, Debug, PartialEq, Eq, Clone, TypeInfo)]
#[allow(non_camel_case_types)]
pub enum BridgeAlephCall<Header: HeaderT> {
	/// `pallet-bridge-aleph::Call::initialize`
	#[codec(index = 1)]
	initialize { init_data: InitializationData<Header> },
}

/// The `BridgeAlephCall` used by a chain.
pub type BridgeAlephCallOf<C> = BridgeAlephCall<HeaderOf<C>>;

pub trait ChainWithAleph: Chain {
	const WITH_CHAIN_ALEPH_PALLET_NAME: &'static str;
	const MAX_HEADER_SIZE: u32;
	const MAX_AUTHORITIES_COUNT: u32;
	const AVERAGE_HEADER_SIZE_IN_JUSTIFICATION: u32;
}
