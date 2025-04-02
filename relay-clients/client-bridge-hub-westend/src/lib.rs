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

//! Types used to connect to the BridgeHub-Westend-Substrate parachain.

pub mod codegen_runtime;

use bp_bridge_hub_westend::AVERAGE_BLOCK_INTERVAL;
use bp_polkadot_core::{SuffixedCommonTransactionExtension, SuffixedCommonTransactionExtensionExt};
use codec::Encode;
use relay_substrate_client::{
	calls::UtilityCall as MockUtilityCall, Chain, ChainWithBalances, ChainWithMessages,
	ChainWithRuntimeVersion, ChainWithTransactions, ChainWithUtilityPallet,
	Error as SubstrateError, MockedRuntimeUtilityPallet, SignParam, SimpleRuntimeVersion,
	UnderlyingChainProvider, UnsignedTransaction,
};
use sp_core::{storage::StorageKey, Pair};
use sp_runtime::{
	generic::SignedPayload,
	traits::{FakeDispatchable, IdentifyAccount},
};
use std::time::Duration;

pub use codegen_runtime::api::runtime_types;
use runtime_types::frame_metadata_hash_extension::Mode;

use bp_runtime::extensions::{
	BridgeRejectObsoleteHeadersAndMessages, GenericTransactionExtensionSchema,
	RefundBridgedParachainMessagesSchema,
};

pub type CheckMetadataHash = GenericTransactionExtensionSchema<Mode, Option<[u8; 32]>>;

pub type TransactionExtension = SuffixedCommonTransactionExtension<(
	BridgeRejectObsoleteHeadersAndMessages,
	RefundBridgedParachainMessagesSchema,
	CheckMetadataHash,
)>;

pub type RuntimeCall = runtime_types::bridge_hub_westend_runtime::RuntimeCall;
pub type BridgeMessagesCall = runtime_types::pallet_bridge_messages::pallet::Call;
pub type BridgeGrandpaCall = runtime_types::pallet_bridge_grandpa::pallet::Call;
pub type BridgeParachainCall = runtime_types::pallet_bridge_parachains::pallet::Call;
type UncheckedExtrinsic =
	bp_bridge_hub_westend::UncheckedExtrinsic<RuntimeCall, TransactionExtension>;
type UtilityCall = runtime_types::pallet_utility::pallet::Call;

/// Westend chain definition
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BridgeHubWestend;

impl UnderlyingChainProvider for BridgeHubWestend {
	type Chain = bp_bridge_hub_westend::BridgeHubWestend;
}

impl Chain for BridgeHubWestend {
	const NAME: &'static str = "BridgeHubWestend";
	const BEST_FINALIZED_HEADER_ID_METHOD: &'static str =
		bp_bridge_hub_westend::BEST_FINALIZED_BRIDGE_HUB_WESTEND_HEADER_METHOD;
	const FREE_HEADERS_INTERVAL_METHOD: &'static str =
		bp_bridge_hub_westend::FREE_HEADERS_INTERVAL_FOR_BRIDGE_HUB_WESTEND_METHOD;
	const AVERAGE_BLOCK_INTERVAL: Duration = AVERAGE_BLOCK_INTERVAL;

	type SignedBlock = bp_bridge_hub_westend::SignedBlock;
	type Call = RuntimeCall;
}

impl ChainWithBalances for BridgeHubWestend {
	fn account_info_storage_key(account_id: &Self::AccountId) -> StorageKey {
		bp_bridge_hub_westend::AccountInfoStorageMapKeyProvider::final_key(account_id)
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

impl ChainWithUtilityPallet for BridgeHubWestend {
	type UtilityPallet = MockedRuntimeUtilityPallet<RuntimeCall>;
}

impl ChainWithTransactions for BridgeHubWestend {
	type AccountKeyPair = sp_core::sr25519::Pair;
	type SignedTransaction = UncheckedExtrinsic;

	fn sign_transaction(
		param: SignParam<Self>,
		unsigned: UnsignedTransaction<Self>,
	) -> Result<Self::SignedTransaction, SubstrateError> {
		let raw_payload = SignedPayload::new(
			FakeDispatchable::from(unsigned.call),
			TransactionExtension::from_params(
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
			call.deconstruct(),
			signer.into_account().into(),
			signature.into(),
			extra,
		))
	}
}

impl ChainWithMessages for BridgeHubWestend {
	const TO_CHAIN_MESSAGE_DETAILS_METHOD: &'static str =
		bp_bridge_hub_westend::TO_BRIDGE_HUB_WESTEND_MESSAGE_DETAILS_METHOD;
	const FROM_CHAIN_MESSAGE_DETAILS_METHOD: &'static str =
		bp_bridge_hub_westend::FROM_BRIDGE_HUB_WESTEND_MESSAGE_DETAILS_METHOD;
}

impl ChainWithRuntimeVersion for BridgeHubWestend {
	const RUNTIME_VERSION: Option<SimpleRuntimeVersion> =
		Some(SimpleRuntimeVersion { spec_version: 1_016_001, transaction_version: 6 });
}
