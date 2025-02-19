// Copyright 2022 Parity Technologies (UK) Ltd.
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

//! Types used to connect to the BridgeHub-Kusama-Substrate parachain.

pub mod codegen_runtime;

use bp_bridge_hub_kusama::AVERAGE_BLOCK_INTERVAL;
use bp_polkadot_core::{SuffixedCommonSignedExtension, SuffixedCommonSignedExtensionExt};
use codec::Encode;
use relay_substrate_client::{
	calls::UtilityCall as MockUtilityCall, Chain, ChainWithBalances, ChainWithMessages,
	ChainWithRuntimeVersion, ChainWithTransactions, ChainWithUtilityPallet,
	Error as SubstrateError, MockedRuntimeUtilityPallet, SignParam, SimpleRuntimeVersion,
	UnderlyingChainProvider, UnsignedTransaction,
};
use sp_core::{storage::StorageKey, Pair};
use sp_runtime::{generic::SignedPayload, traits::IdentifyAccount};
use std::time::Duration;

pub use codegen_runtime::api::runtime_types;
use runtime_types::frame_metadata_hash_extension::Mode;

use bp_runtime::extensions::{
	BridgeRejectObsoleteHeadersAndMessages, GenericSignedExtensionSchema,
	RefundBridgedParachainMessagesSchema,
};

pub type CheckMetadataHash = GenericSignedExtensionSchema<Mode, Option<[u8; 32]>>;

pub type SignedExtension = SuffixedCommonSignedExtension<(
	BridgeRejectObsoleteHeadersAndMessages,
	RefundBridgedParachainMessagesSchema,
	CheckMetadataHash,
)>;

pub type RuntimeCall = runtime_types::bridge_hub_kusama_runtime::RuntimeCall;
pub type BridgeMessagesCall = runtime_types::pallet_bridge_messages::pallet::Call;
pub type BridgeGrandpaCall = runtime_types::pallet_bridge_grandpa::pallet::Call;
pub type BridgeParachainCall = runtime_types::pallet_bridge_parachains::pallet::Call;
type UncheckedExtrinsic = bp_bridge_hub_kusama::UncheckedExtrinsic<RuntimeCall, SignedExtension>;
type UtilityCall = runtime_types::pallet_utility::pallet::Call;

/// Kusama chain definition
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BridgeHubKusama;

impl UnderlyingChainProvider for BridgeHubKusama {
	type Chain = bp_bridge_hub_kusama::BridgeHubKusama;
}

impl Chain for BridgeHubKusama {
	const NAME: &'static str = "BridgeHubKusama";
	const BEST_FINALIZED_HEADER_ID_METHOD: &'static str =
		bp_bridge_hub_kusama::BEST_FINALIZED_BRIDGE_HUB_KUSAMA_HEADER_METHOD;
	const FREE_HEADERS_INTERVAL_METHOD: &'static str =
		bp_bridge_hub_kusama::FREE_HEADERS_INTERVAL_FOR_BRIDGE_HUB_KUSAMA_METHOD;
	const AVERAGE_BLOCK_INTERVAL: Duration = AVERAGE_BLOCK_INTERVAL;

	type SignedBlock = bp_bridge_hub_kusama::SignedBlock;
	type Call = RuntimeCall;
}

impl ChainWithBalances for BridgeHubKusama {
	fn account_info_storage_key(account_id: &Self::AccountId) -> StorageKey {
		bp_bridge_hub_kusama::AccountInfoStorageMapKeyProvider::final_key(account_id)
	}
}

impl From<MockUtilityCall<RuntimeCall>> for RuntimeCall {
	fn from(value: MockUtilityCall<RuntimeCall>) -> RuntimeCall {
		match value {
			MockUtilityCall::batch_all(calls) =>
				RuntimeCall::Utility(UtilityCall::batch_all { calls }),
		}
	}
}

impl ChainWithUtilityPallet for BridgeHubKusama {
	type UtilityPallet = MockedRuntimeUtilityPallet<RuntimeCall>;
}

impl ChainWithTransactions for BridgeHubKusama {
	type AccountKeyPair = sp_core::sr25519::Pair;
	type SignedTransaction = UncheckedExtrinsic;

	fn sign_transaction(
		param: SignParam<Self>,
		unsigned: UnsignedTransaction<Self>,
	) -> Result<Self::SignedTransaction, SubstrateError> {
		let raw_payload = SignedPayload::new(
			unsigned.call,
			SignedExtension::from_params(
				param.spec_version,
				param.transaction_version,
				unsigned.era,
				param.genesis_hash,
				unsigned.nonce,
				unsigned.tip,
				(((), (), Mode::Disabled), ((), (), None)),
			),
		)?;

		let signature = raw_payload.using_encoded(|payload| param.signer.sign(payload));
		let signer: sp_runtime::MultiSigner = param.signer.public().into();
		let (call, extra, _) = raw_payload.deconstruct();

		Ok(UncheckedExtrinsic::new_signed(
			call,
			signer.into_account().into(),
			signature.into(),
			extra,
		))
	}
}

impl ChainWithMessages for BridgeHubKusama {
	const WITH_CHAIN_RELAYERS_PALLET_NAME: Option<&'static str> =
		Some(bp_bridge_hub_kusama::WITH_BRIDGE_HUB_KUSAMA_RELAYERS_PALLET_NAME);

	const TO_CHAIN_MESSAGE_DETAILS_METHOD: &'static str =
		bp_bridge_hub_kusama::TO_BRIDGE_HUB_KUSAMA_MESSAGE_DETAILS_METHOD;
	const FROM_CHAIN_MESSAGE_DETAILS_METHOD: &'static str =
		bp_bridge_hub_kusama::FROM_BRIDGE_HUB_KUSAMA_MESSAGE_DETAILS_METHOD;
}

impl ChainWithRuntimeVersion for BridgeHubKusama {
	const RUNTIME_VERSION: Option<SimpleRuntimeVersion> =
		Some(SimpleRuntimeVersion { spec_version: 1_004_000, transaction_version: 5 });
}
