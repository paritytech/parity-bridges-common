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

//! Types that are specific to the BridgeHubRococo runtime.

use codec::{Decode, Encode};
use scale_info::TypeInfo;

pub use bp_header_chain::BridgeGrandpaCallOf;
pub use bp_parachains::BridgeParachainCall;
pub use bp_runtime::calls::SystemCall;
pub use bridge_runtime_common::messages::BridgeMessagesCallOf;

// TODO:check-parameter - check SignedExtension
/// Unchecked BridgeHubRococo extrinsic.
pub type UncheckedExtrinsic = bp_bridge_hub_rococo::UncheckedExtrinsic<Call>;

pub type BridgeWococoGrandpaCall = BridgeGrandpaCallOf<bp_wococo::Wococo>;
pub type BridgeWococoMessagesCall = BridgeMessagesCallOf<bp_bridge_hub_wococo::BridgeHubWococo>;

/// `BridgeHubRococo` Runtime `Call` enum.
///
/// The enum represents a subset of possible `Call`s we can send to `BridgeHubRococo` chain.
/// Ideally this code would be auto-generated from metadata, because we want to
/// avoid depending directly on the ENTIRE runtime just to get the encoding of `Dispatchable`s.
///
/// All entries here (like pretty much in the entire file) must be kept in sync with
/// `BridgeHubRococo` `construct_runtime`, so that we maintain SCALE-compatibility.
///
/// // TODO:check-parameter -> change bridge-hub-rococo-wococo when merged to master in cumulus
/// See: [link](https://github.com/paritytech/cumulus/blob/bridge-hub-rococo-wococo/parachains/runtimes/bridge-hubs/bridge-hub-rococo/src/lib.rs)
#[allow(clippy::large_enum_variant)]
#[derive(Encode, Decode, Debug, PartialEq, Eq, Clone, TypeInfo)]
pub enum Call {
	#[codec(index = 0)]
	System(SystemCall),

	/// Wococo bridge pallet.
	#[codec(index = 41)]
	BridgeWococoGrandpa(BridgeWococoGrandpaCall),
	/// Wococo parachain bridge pallet.
	#[codec(index = 42)]
	BridgeWococoParachain(BridgeParachainCall),
	/// Wococo messages bridge pallet.
	#[codec(index = 46)]
	BridgeWococoMessages(BridgeWococoMessagesCall),
}
