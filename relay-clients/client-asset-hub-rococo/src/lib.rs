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

//! Types used to connect to the AssetHub-Rococo-Substrate parachain.

pub mod codegen_runtime;

use bp_asset_hub_rococo::AVERAGE_BLOCK_INTERVAL;
use bp_polkadot_core::{SuffixedCommonTransactionExtension, SuffixedCommonTransactionExtensionExt};
use codec::Encode;
use relay_substrate_client::{
	Chain, ChainWithBalances, ChainWithMessages, ChainWithRuntimeVersion, ChainWithTransactions,
	Error as SubstrateError, SignParam, SimpleRuntimeVersion, UnderlyingChainProvider,
	UnsignedTransaction,
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
	CheckMetadataHash,
	BridgeRejectObsoleteHeadersAndMessages,
	RefundBridgedParachainMessagesSchema,
)>;

pub type RuntimeCall = runtime_types::asset_hub_rococo_runtime::RuntimeCall;
pub type BridgeMessagesCall = runtime_types::pallet_bridge_messages::pallet::Call;

type UncheckedExtrinsic =
	bp_asset_hub_rococo::UncheckedExtrinsic<RuntimeCall, TransactionExtension>;

/// Rococo chain definition
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssetHubRococo;

impl UnderlyingChainProvider for AssetHubRococo {
	type Chain = bp_asset_hub_rococo::AssetHubRococo;
}

impl Chain for AssetHubRococo {
	const NAME: &'static str = "AssetHubRococo";
	const BEST_FINALIZED_HEADER_ID_METHOD: &'static str =
		bp_asset_hub_rococo::BEST_FINALIZED_ASSET_HUB_ROCOCO_HEADER_METHOD;
	const FREE_HEADERS_INTERVAL_METHOD: &'static str =
		bp_asset_hub_rococo::FREE_HEADERS_INTERVAL_FOR_ASSET_HUB_ROCOCO_METHOD;
	const AVERAGE_BLOCK_INTERVAL: Duration = AVERAGE_BLOCK_INTERVAL;

	type SignedBlock = bp_asset_hub_rococo::SignedBlock;
	type Call = RuntimeCall;
}

impl ChainWithBalances for AssetHubRococo {
	fn account_info_storage_key(account_id: &Self::AccountId) -> StorageKey {
		bp_asset_hub_rococo::AccountInfoStorageMapKeyProvider::final_key(account_id)
	}
}

impl ChainWithTransactions for AssetHubRococo {
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
				((Mode::Disabled, (), ()), (None, (), ())),
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

impl ChainWithMessages for AssetHubRococo {
	const WITH_CHAIN_RELAYERS_PALLET_NAME: Option<&'static str> =
		Some(bp_asset_hub_rococo::WITH_ASSET_HUB_ROCOCO_RELAYERS_PALLET_NAME);

	const TO_CHAIN_MESSAGE_DETAILS_METHOD: &'static str =
		bp_asset_hub_rococo::TO_ASSET_HUB_ROCOCO_MESSAGE_DETAILS_METHOD;
	const FROM_CHAIN_MESSAGE_DETAILS_METHOD: &'static str =
		bp_asset_hub_rococo::FROM_ASSET_HUB_ROCOCO_MESSAGE_DETAILS_METHOD;
}

impl ChainWithRuntimeVersion for AssetHubRococo {
	const RUNTIME_VERSION: Option<SimpleRuntimeVersion> =
		Some(SimpleRuntimeVersion { spec_version: 1_016_001, transaction_version: 6 });
}
