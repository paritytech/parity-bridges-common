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

use codec::{Compact, Decode, Encode};
use frame_support::weights::Weight;
use relay_substrate_client::{
	BalanceOf, Chain, ChainBase, ChainWithBalances, ChainWithGrandpa, Error as SubstrateError,
	IndexOf, RelayChain, SignParam, TransactionSignScheme, UnsignedTransaction,
};
use sp_core::{storage::StorageKey, Pair};
use sp_runtime::{generic::SignedPayload, traits::IdentifyAccount};
use std::time::Duration;

// // TODO: check add Account/Signature... all that stuff according to runtime
// mod bo_bridge_hub_wococo {
// 	pub use bp_polkadot_core::*;
// 	pub type BridgeHubWococo = PolkadotLike;
// 	pub type WeightToFee = frame_support::weights::IdentityFee<Balance>;
// }
//
// /// Re-export runtime
// /// Wococo BridgeHub parachain has the same runtime as Rococo
// pub use bridge_hub_rococo_runtime as runtime;
//
// /// Wococo header id.
// pub type HeaderId =
// 	relay_utils::HeaderId<bo_bridge_hub_wococo::Hash, bo_bridge_hub_wococo::BlockNumber>;
//
// /// Wococo header type used in headers sync.
// pub type SyncHeader = relay_substrate_client::SyncHeader<bo_bridge_hub_wococo::Header>;
//
// /// Wococo chain definition
// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// pub struct BridgeHubWococo;
//
// impl ChainBase for BridgeHubWococo {
// 	type BlockNumber = bo_bridge_hub_wococo::BlockNumber;
// 	type Hash = bo_bridge_hub_wococo::Hash;
// 	type Hasher = bo_bridge_hub_wococo::Hashing;
// 	type Header = bo_bridge_hub_wococo::Header;
//
// 	type AccountId = bo_bridge_hub_wococo::AccountId;
// 	type Balance = bo_bridge_hub_wococo::Balance;
// 	type Index = bo_bridge_hub_wococo::Nonce;
// 	type Signature = bo_bridge_hub_wococo::Signature;
//
// 	fn max_extrinsic_size() -> u32 {
// 		bo_bridge_hub_wococo::BridgeHubWococo::max_extrinsic_size()
// 	}
//
// 	fn max_extrinsic_weight() -> Weight {
// 		bo_bridge_hub_wococo::BridgeHubWococo::max_extrinsic_weight()
// 	}
// }
//
// impl Chain for BridgeHubWococo {
// 	const NAME: &'static str = "BridgeHubWococo";
// 	const TOKEN_ID: Option<&'static str> = None;
// 	const BEST_FINALIZED_HEADER_ID_METHOD: &'static str =
// 		"TODO: add best_finalized runtime api to bridge-hubs";
// 	const AVERAGE_BLOCK_INTERVAL: Duration = Duration::from_secs(6);
// 	const STORAGE_PROOF_OVERHEAD: u32 = bp_polkadot_core::EXTRA_STORAGE_PROOF_SIZE;
//
// 	type SignedBlock = bo_bridge_hub_wococo::SignedBlock;
// 	type Call = runtime::Call;
// 	type WeightToFee = bo_bridge_hub_wococo::WeightToFee;
// }
//
// impl RelayChain for BridgeHubWococo {
// 	const PARAS_PALLET_NAME: &'static str = "TODO:BridgeHubWococo:PARAS_PALLET_NAME";
// 	const PARACHAINS_FINALITY_PALLET_NAME: &'static str =
// 		"TODO:BridgeHubWococo:PARACHAINS_FINALITY_PALLET_NAME";
// }
//
// impl ChainWithGrandpa for BridgeHubWococo {
// 	const WITH_CHAIN_GRANDPA_PALLET_NAME: &'static str =
// 		"TODO:BridgeHubWococo:WITH_CHAIN_GRANDPA_PALLET_NAME";
// }
//
// impl ChainWithBalances for BridgeHubWococo {
// 	fn account_info_storage_key(account_id: &Self::AccountId) -> StorageKey {
// 		StorageKey(bo_bridge_hub_wococo::account_info_storage_key(account_id))
// 	}
// }
//
// impl TransactionSignScheme for BridgeHubWococo {
// 	type Chain = BridgeHubWococo;
// 	type AccountKeyPair = sp_core::sr25519::Pair;
// 	type SignedTransaction = runtime::UncheckedExtrinsic;
//
// 	fn sign_transaction(param: SignParam<Self>) -> Result<Self::SignedTransaction, SubstrateError> {
// 		let raw_payload = SignedPayload::from_raw(
// 			param.unsigned.call.clone(),
// 			(
// 				frame_system::CheckNonZeroSender::<runtime::Runtime>::new(),
// 				frame_system::CheckSpecVersion::<runtime::Runtime>::new(),
// 				frame_system::CheckTxVersion::<runtime::Runtime>::new(),
// 				frame_system::CheckGenesis::<runtime::Runtime>::new(),
// 				frame_system::CheckEra::<runtime::Runtime>::from(param.era.frame_era()),
// 				frame_system::CheckNonce::<runtime::Runtime>::from(param.unsigned.nonce),
// 				frame_system::CheckWeight::<runtime::Runtime>::new(),
// 				pallet_transaction_payment::ChargeTransactionPayment::<runtime::Runtime>::from(
// 					param.unsigned.tip,
// 				),
// 			),
// 			(
// 				(),
// 				param.spec_version,
// 				param.transaction_version,
// 				param.genesis_hash,
// 				param.era.signed_payload(param.genesis_hash),
// 				(),
// 				(),
// 				(),
// 			),
// 		);
// 		let signature = raw_payload.using_encoded(|payload| param.signer.sign(payload));
// 		let signer: sp_runtime::MultiSigner = param.signer.public().into();
// 		let (call, extra, _) = raw_payload.deconstruct();
//
// 		Ok(runtime::UncheckedExtrinsic::new_signed(
// 			call.into_decoded()?,
// 			signer.into_account().into(),
// 			signature.into(),
// 			extra,
// 		))
// 	}
//
// 	fn is_signed(tx: &Self::SignedTransaction) -> bool {
// 		tx.signature.is_some()
// 	}
//
// 	fn is_signed_by(signer: &Self::AccountKeyPair, tx: &Self::SignedTransaction) -> bool {
// 		tx.signature
// 			.as_ref()
// 			.map(|(address, _, _)| *address == runtime::Address::Id(signer.public().into()))
// 			.unwrap_or(false)
// 	}
//
// 	fn parse_transaction(tx: Self::SignedTransaction) -> Option<UnsignedTransaction<Self::Chain>> {
// 		let extra = &tx.signature.as_ref()?.2;
// 		Some(UnsignedTransaction {
// 			call: tx.function.into(),
// 			nonce: Compact::<IndexOf<Self::Chain>>::decode(&mut &extra.5.encode()[..]).ok()?.into(),
// 			tip: Compact::<BalanceOf<Self::Chain>>::decode(&mut &extra.7.encode()[..])
// 				.ok()?
// 				.into(),
// 		})
// 	}
// }
//
// #[cfg(test)]
// mod tests {
// 	use super::*;
// 	use relay_substrate_client::TransactionEra;
//
// 	#[test]
// 	fn parse_transaction_works() {
// 		let unsigned = UnsignedTransaction {
// 			call: bridge_hub_wococo_runtime::Call::System(
// 				bridge_hub_wococo_runtime::SystemCall::remark { remark: b"Hello world!".to_vec() },
// 			)
// 			.into(),
// 			nonce: 777,
// 			tip: 888,
// 		};
// 		let signed_transaction = BridgeHubWococo::sign_transaction(SignParam {
// 			spec_version: 42,
// 			transaction_version: 50000,
// 			genesis_hash: [42u8; 32].into(),
// 			signer: sp_core::sr25519::Pair::from_seed_slice(&[1u8; 32]).unwrap(),
// 			era: TransactionEra::immortal(),
// 			unsigned: unsigned.clone(),
// 		})
// 		.unwrap();
// 		let parsed_transaction = BridgeHubWococo::parse_transaction(signed_transaction).unwrap();
// 		assert_eq!(parsed_transaction, unsigned);
// 	}
// }
