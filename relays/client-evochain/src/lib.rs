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

//! Types used to connect to the Evochain-Substrate chain.

use bp_evochain::EVOCHAIN_ACCEPTED_GRANDPA_FINALITY_PROOFS_METHOD;
use codec::{Compact, Decode, Encode};
use node_template_runtime as evochain_runtime;
use relay_substrate_client::{
	BalanceOf, Chain, ChainWithBalances, ChainWithGrandpa, ChainWithMessages,
	ChainWithTransactions, ChainWithUtilityPallet, Error as SubstrateError,
	FullRuntimeUtilityPallet, NonceOf, SignParam, UnderlyingChainProvider, UnsignedTransaction,
};
use sp_core::{storage::StorageKey, Pair};
use sp_runtime::{generic::SignedPayload, traits::IdentifyAccount};
use std::time::Duration;

/// Evochain header id.
pub type HeaderId = relay_utils::HeaderId<evochain_runtime::Hash, evochain_runtime::BlockNumber>;

/// Evochain chain definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Evochain;

impl UnderlyingChainProvider for Evochain {
	type Chain = bp_evochain::Evochain;
}

impl ChainWithMessages for Evochain {
	// TODO (https://github.com/paritytech/parity-bridges-common/issues/1692): change the name
	const WITH_CHAIN_RELAYERS_PALLET_NAME: Option<&'static str> = Some("BridgeRelayers");
	const TO_CHAIN_MESSAGE_DETAILS_METHOD: &'static str =
		bp_evochain::TO_EVOCHAIN_MESSAGE_DETAILS_METHOD;
	const FROM_CHAIN_MESSAGE_DETAILS_METHOD: &'static str =
		bp_evochain::FROM_EVOCHAIN_MESSAGE_DETAILS_METHOD;
}

impl Chain for Evochain {
	const NAME: &'static str = "Evochain";
	const BEST_FINALIZED_HEADER_ID_METHOD: &'static str =
		bp_evochain::BEST_FINALIZED_EVOCHAIN_HEADER_METHOD;
	const AVERAGE_BLOCK_INTERVAL: Duration = Duration::from_secs(5);

	type SignedBlock = evochain_runtime::SignedBlock;
	type Call = evochain_runtime::RuntimeCall;
}

impl ChainWithGrandpa for Evochain {
	const ACCEPTED_FINALITY_PROOFS_METHOD: &'static str =
		EVOCHAIN_ACCEPTED_GRANDPA_FINALITY_PROOFS_METHOD;
}

impl ChainWithBalances for Evochain {
	fn account_info_storage_key(account_id: &Self::AccountId) -> StorageKey {
		use frame_support::storage::generator::StorageMap;
		StorageKey(frame_system::Account::<evochain_runtime::Runtime>::storage_map_final_key(
			account_id,
		))
	}
}

impl ChainWithTransactions for Evochain {
	type AccountKeyPair = sp_core::sr25519::Pair;
	type SignedTransaction = evochain_runtime::UncheckedExtrinsic;

	fn sign_transaction(
		param: SignParam<Self>,
		unsigned: UnsignedTransaction<Self>,
	) -> Result<Self::SignedTransaction, SubstrateError> {
		let raw_payload = SignedPayload::from_raw(
			unsigned.call.clone(),
			(
				frame_system::CheckNonZeroSender::<evochain_runtime::Runtime>::new(),
				frame_system::CheckSpecVersion::<evochain_runtime::Runtime>::new(),
				frame_system::CheckTxVersion::<evochain_runtime::Runtime>::new(),
				frame_system::CheckGenesis::<evochain_runtime::Runtime>::new(),
				frame_system::CheckEra::<evochain_runtime::Runtime>::from(unsigned.era.frame_era()),
				frame_system::CheckNonce::<evochain_runtime::Runtime>::from(unsigned.nonce),
				frame_system::CheckWeight::<evochain_runtime::Runtime>::new(),
				pallet_transaction_payment::ChargeTransactionPayment::<evochain_runtime::Runtime>::from(unsigned.tip),
				evochain_runtime::BridgeRejectObsoleteHeadersAndMessages,
				evochain_runtime::BridgeRefundRialtoParachainMessages::default(),
			),
			(
				(),
				param.spec_version,
				param.transaction_version,
				param.genesis_hash,
				unsigned.era.signed_payload(param.genesis_hash),
				(),
				(),
				(),
				(),
				()
			),
		);
		let signature = raw_payload.using_encoded(|payload| param.signer.sign(payload));
		let signer: sp_runtime::MultiSigner = param.signer.public().into();
		let (call, extra, _) = raw_payload.deconstruct();

		Ok(evochain_runtime::UncheckedExtrinsic::new_signed(
			call.into_decoded()?,
			signer.into_account(),
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
				*address == evochain_runtime::Address::from(*signer.public().as_array_ref())
			})
			.unwrap_or(false)
	}

	fn parse_transaction(tx: Self::SignedTransaction) -> Option<UnsignedTransaction<Self>> {
		let extra = &tx.signature.as_ref()?.2;
		Some(
			UnsignedTransaction::new(
				tx.function.into(),
				Compact::<NonceOf<Self>>::decode(&mut &extra.5.encode()[..]).ok()?.into(),
			)
			.tip(Compact::<BalanceOf<Self>>::decode(&mut &extra.7.encode()[..]).ok()?.into()),
		)
	}
}

impl ChainWithUtilityPallet for Evochain {
	type UtilityPallet = FullRuntimeUtilityPallet<evochain_runtime::Runtime>;
}

/// Evochain signing params.
pub type SigningParams = sp_core::sr25519::Pair;

/// Evochain header type used in headers sync.
pub type SyncHeader = relay_substrate_client::SyncHeader<evochain_runtime::Header>;

#[cfg(test)]
mod tests {
	use super::*;
	use relay_substrate_client::TransactionEra;

	#[test]
	fn parse_transaction_works() {
		let unsigned = UnsignedTransaction {
			call: evochain_runtime::RuntimeCall::System(evochain_runtime::SystemCall::remark {
				remark: b"Hello world!".to_vec(),
			})
			.into(),
			nonce: 777,
			tip: 888,
			era: TransactionEra::immortal(),
		};
		let signed_transaction = Evochain::sign_transaction(
			SignParam {
				spec_version: 42,
				transaction_version: 50000,
				genesis_hash: [42u8; 64].into(),
				signer: sp_core::sr25519::Pair::from_seed_slice(&[1u8; 32]).unwrap(),
			},
			unsigned.clone(),
		)
		.unwrap();
		let parsed_transaction = Evochain::parse_transaction(signed_transaction).unwrap();
		assert_eq!(parsed_transaction, unsigned);
	}
}
