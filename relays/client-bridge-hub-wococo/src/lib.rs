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

//! Types used to connect to the BridgeHub-Wococo-Substrate parachain.

use bp_messages::MessageNonce;
use codec::Encode;
use relay_substrate_client::{
	Chain, ChainBaseShadow, ChainWithBalances, ChainWithMessages, ChainWithTransactions,
	Error as SubstrateError, SignParam, UnsignedTransaction,
};
use sp_core::{storage::StorageKey, Pair};
use sp_runtime::{generic::SignedPayload, traits::IdentifyAccount};
use std::time::Duration;

/// Re-export runtime wrapper
pub mod runtime_wrapper;
pub use runtime_wrapper as runtime;

/// Wococo chain definition
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BridgeHubWococo;

impl ChainBaseShadow for BridgeHubWococo {
	type Chain = bp_bridge_hub_wococo::BridgeHubWococo;
}

impl Chain for BridgeHubWococo {
	const NAME: &'static str = "BridgeHubWococo";
	const TOKEN_ID: Option<&'static str> = None;
	const BEST_FINALIZED_HEADER_ID_METHOD: &'static str =
		bp_bridge_hub_wococo::BEST_FINALIZED_BRIDGE_HUB_WOCOCO_HEADER_METHOD;
	const AVERAGE_BLOCK_INTERVAL: Duration = Duration::from_secs(6);

	type SignedBlock = bp_bridge_hub_wococo::SignedBlock;
	type Call = runtime::Call;
}

impl ChainWithBalances for BridgeHubWococo {
	fn account_info_storage_key(account_id: &Self::AccountId) -> StorageKey {
		bp_bridge_hub_wococo::AccountInfoStorageMapKeyProvider::final_key(account_id)
	}
}

impl ChainWithTransactions for BridgeHubWococo {
	type AccountKeyPair = sp_core::sr25519::Pair;
	type SignedTransaction = runtime::UncheckedExtrinsic;

	fn sign_transaction(
		param: SignParam<Self>,
		unsigned: UnsignedTransaction<Self>,
	) -> Result<Self::SignedTransaction, SubstrateError> {
		let raw_payload = SignedPayload::new(
			unsigned.call,
			bp_bridge_hub_wococo::SignedExtensions::new(
				param.spec_version,
				param.transaction_version,
				unsigned.era,
				param.genesis_hash,
				unsigned.nonce,
				unsigned.tip,
			),
		)?;

		let signature = raw_payload.using_encoded(|payload| param.signer.sign(payload));
		let signer: sp_runtime::MultiSigner = param.signer.public().into();
		let (call, extra, _) = raw_payload.deconstruct();

		Ok(bp_bridge_hub_wococo::UncheckedExtrinsic::new_signed(
			call,
			signer.into_account().into(),
			signature.into(),
			extra,
		))
	}

	fn is_signed(tx: &Self::SignedTransaction) -> bool {
		tx.signature.is_some()
	}

	fn is_signed_by(signer: &Self::AccountKeyPair, tx: &Self::SignedTransaction) -> bool {
		tx.signature
			.as_ref()
			.map(|(address, _, _)| {
				*address == bp_bridge_hub_wococo::Address::Id(signer.public().into())
			})
			.unwrap_or(false)
	}

	fn parse_transaction(tx: Self::SignedTransaction) -> Option<UnsignedTransaction<Self>> {
		let extra = &tx.signature.as_ref()?.2;
		Some(UnsignedTransaction::new(tx.function, extra.nonce()).tip(extra.tip()))
	}
}

impl ChainWithMessages for BridgeHubWococo {
	const WITH_CHAIN_MESSAGES_PALLET_NAME: &'static str =
		bp_bridge_hub_wococo::WITH_BRIDGE_HUB_WOCOCO_MESSAGES_PALLET_NAME;

	const TO_CHAIN_MESSAGE_DETAILS_METHOD: &'static str =
		bp_bridge_hub_wococo::TO_BRIDGE_HUB_WOCOCO_MESSAGE_DETAILS_METHOD;
	const FROM_CHAIN_MESSAGE_DETAILS_METHOD: &'static str =
		bp_bridge_hub_wococo::FROM_BRIDGE_HUB_WOCOCO_MESSAGE_DETAILS_METHOD;

	const MAX_UNREWARDED_RELAYERS_IN_CONFIRMATION_TX: MessageNonce =
		bp_bridge_hub_wococo::MAX_UNREWARDED_RELAYERS_IN_CONFIRMATION_TX;
	const MAX_UNCONFIRMED_MESSAGES_IN_CONFIRMATION_TX: MessageNonce =
		bp_bridge_hub_wococo::MAX_UNCONFIRMED_MESSAGES_IN_CONFIRMATION_TX;

	// TODO: fix (https://github.com/paritytech/parity-bridges-common/issues/1640)
	type WeightInfo = ();
}

#[cfg(test)]
mod tests {
	use super::*;
	use relay_substrate_client::TransactionEra;

	#[test]
	fn parse_transaction_works() {
		let unsigned = UnsignedTransaction {
			call: runtime::Call::System(runtime::SystemCall::remark(b"Hello world!".to_vec()))
				.into(),
			nonce: 777,
			tip: 888,
			era: TransactionEra::immortal(),
		};
		let signed_transaction = BridgeHubWococo::sign_transaction(
			SignParam {
				spec_version: 42,
				transaction_version: 50000,
				genesis_hash: [42u8; 32].into(),
				signer: sp_core::sr25519::Pair::from_seed_slice(&[1u8; 32]).unwrap(),
			},
			unsigned.clone(),
		)
		.unwrap();
		let parsed_transaction = BridgeHubWococo::parse_transaction(signed_transaction).unwrap();
		assert_eq!(parsed_transaction, unsigned);
	}
}
