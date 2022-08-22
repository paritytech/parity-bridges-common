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

// TODO: join with primitives do we need this here or move to the primitives?

//! Types that are specific to the BridgeHubWococo runtime.

use bp_polkadot_core::PolkadotLike;
use codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_version::RuntimeVersion;

pub use bp_bridge_hub_wococo::SS58Prefix;
use bp_runtime::Chain;

pub const VERSION: RuntimeVersion = relay_bridge_hub_rococo_client::runtime::VERSION;
pub type UncheckedExtrinsic = bp_bridge_hub_wococo::UncheckedExtrinsic<Call>;

/// Wococo Runtime `Call` enum.
///
/// The enum represents a subset of possible `Call`s we can send to Wococo chain.
/// Ideally this code would be auto-generated from metadata, because we want to
/// avoid depending directly on the ENTIRE runtime just to get the encoding of `Dispatchable`s.
///
/// All entries here (like pretty much in the entire file) must be kept in sync with Rococo
/// `construct_runtime`, so that we maintain SCALE-compatibility.
///
/// See: [link](https://github.com/paritytech/polkadot/blob/master/runtime/rococo/src/lib.rs)
#[allow(clippy::large_enum_variant)]
#[derive(Encode, Decode, Debug, PartialEq, Eq, Clone, TypeInfo)]
pub enum Call {
	// TODO:check-parameter - indexes
	/// System pallet.
	#[codec(index = 0)]
	System(SystemCall),
	/// Rococo bridge pallet.
	#[codec(index = 41)]
	BridgeGrandpaRococo(BridgeGrandpaRococoCall),
}

#[derive(Encode, Decode, Debug, PartialEq, Eq, Clone, TypeInfo)]
#[allow(non_camel_case_types)]
pub enum SystemCall {
	#[codec(index = 1)]
	remark(Vec<u8>),
}
#[derive(Encode, Decode, Debug, PartialEq, Eq, Clone, TypeInfo)]
#[allow(non_camel_case_types)]
pub enum BridgeGrandpaRococoCall {
	#[codec(index = 0)]
	submit_finality_proof(
		Box<<PolkadotLike as Chain>::Header>,
		bp_header_chain::justification::GrandpaJustification<<PolkadotLike as Chain>::Header>,
	),
	#[codec(index = 1)]
	initialize(bp_header_chain::InitializationData<<PolkadotLike as Chain>::Header>),
}

impl sp_runtime::traits::Dispatchable for Call {
	type Origin = ();
	type Config = ();
	type Info = ();
	type PostInfo = ();

	fn dispatch(self, _origin: Self::Origin) -> sp_runtime::DispatchResultWithInfo<Self::PostInfo> {
		unimplemented!("The Call is not expected to be dispatched.")
	}
}
