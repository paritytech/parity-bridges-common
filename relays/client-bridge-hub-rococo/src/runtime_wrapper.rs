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

//! Types that are specific to the BridgeHubRococo runtime.

use codec::{Decode, Encode};
use scale_info::TypeInfo;

pub use bp_header_chain::BridgeGrandpaCallOf;
pub use bp_parachains::BridgeParachainCall;
pub use bridge_runtime_common::messages::BridgeMessagesCallOf;
pub use relay_substrate_client::calls::{SystemCall, UtilityCall};

/// Unchecked BridgeHubRococo extrinsic.
pub type UncheckedExtrinsic = bp_bridge_hub_rococo::UncheckedExtrinsic<
	Call,
	rewarding_bridge_signed_extension::RewardingBridgeSignedExtension,
>;

// The indirect pallet call used to sync `Wococo` GRANDPA finality to `BHRococo`.
pub type BridgeWococoGrandpaCall = BridgeGrandpaCallOf<bp_wococo::Wococo>;
// The indirect pallet call used to sync `BridgeHubWococo` messages to `BHRococo`.
pub type BridgeWococoMessagesCall = BridgeMessagesCallOf<bp_bridge_hub_wococo::BridgeHubWococo>;

/// `BridgeHubRococo` Runtime `Call` enum.
///
/// The enum represents a subset of possible `Call`s we can send to `BridgeHubRococo` chain.
/// Ideally this code would be auto-generated from metadata, because we want to
/// avoid depending directly on the ENTIRE runtime just to get the encoding of `Dispatchable`s.
///
/// All entries here (like pretty much in the entire file) must be kept in sync with
/// `BridgeHubRococo` `construct_runtime`, so that we maintain SCALE-compatibility.
#[allow(clippy::large_enum_variant)]
#[derive(Encode, Decode, Debug, PartialEq, Eq, Clone, TypeInfo)]
pub enum Call {
	#[cfg(test)]
	#[codec(index = 0)]
	System(SystemCall),
	/// Utility pallet.
	#[codec(index = 40)]
	Utility(UtilityCall<Call>),

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

impl From<UtilityCall<Call>> for Call {
	fn from(call: UtilityCall<Call>) -> Call {
		Call::Utility(call)
	}
}

// TODO: remove this and use common from cumulus-like once fixed (https://github.com/paritytech/parity-bridges-common/issues/1598)
/// Module with rewarding bridge signed extension support
pub mod rewarding_bridge_signed_extension {
	use bp_polkadot_core::{Balance, Hash, Nonce, PolkadotLike};
	use bp_runtime::extensions::*;

	type RewardingBridgeSignedExtra = (
		CheckNonZeroSender,
		CheckSpecVersion,
		CheckTxVersion,
		CheckGenesis<PolkadotLike>,
		CheckEra<PolkadotLike>,
		CheckNonce<Nonce>,
		CheckWeight,
		ChargeTransactionPayment<PolkadotLike>,
		BridgeRejectObsoleteHeadersAndMessages,
		RefundBridgedParachainMessages,
		RefundBridgedParachainMessages,
	);

	/// The signed extension used by Cumulus and Cumulus-like parachain with bridging and rewarding.
	pub type RewardingBridgeSignedExtension = GenericSignedExtension<RewardingBridgeSignedExtra>;

	pub fn from_params(
		spec_version: u32,
		transaction_version: u32,
		era: bp_runtime::TransactionEraOf<PolkadotLike>,
		genesis_hash: Hash,
		nonce: Nonce,
		tip: Balance,
	) -> RewardingBridgeSignedExtension {
		GenericSignedExtension::<RewardingBridgeSignedExtra>::new(
			(
				(),              // non-zero sender
				(),              // spec version
				(),              // tx version
				(),              // genesis
				era.frame_era(), // era
				nonce.into(),    // nonce (compact encoding)
				(),              // Check weight
				tip.into(),      // transaction payment / tip (compact encoding)
				(),              // bridge reject obsolete headers and msgs
				(),              // bridge-hub-rococo instance1 register reward for message passing
				(),              // bridge-hub-rococo instance2 register reward for message passing
			),
			Some((
				(),
				spec_version,
				transaction_version,
				genesis_hash,
				era.signed_payload(genesis_hash),
				(),
				(),
				(),
				(),
				(),
				(),
			)),
		)
	}

	/// Return signer nonce, used to craft transaction.
	pub fn nonce(sign_ext: &RewardingBridgeSignedExtension) -> Nonce {
		sign_ext.payload.5.into()
	}

	/// Return transaction tip.
	pub fn tip(sign_ext: &RewardingBridgeSignedExtension) -> Balance {
		sign_ext.payload.7.into()
	}
}
