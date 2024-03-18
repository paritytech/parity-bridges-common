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

//! Types used to connect to the Polkadot chain.

mod codegen_runtime;

use bp_polkadot::{AccountInfoStorageMapKeyProvider, POLKADOT_SYNCED_HEADERS_GRANDPA_INFO_METHOD};
use bp_polkadot_core::SuffixedCommonTransactionExtensionExt;
use codec::Encode;
use relay_substrate_client::{
	Chain, ChainWithBalances, ChainWithGrandpa, ChainWithTransactions, Error as SubstrateError,
	RelayChain, SignParam, UnderlyingChainProvider, UnsignedTransaction,
};
use sp_core::{storage::StorageKey, Pair};
use sp_runtime::{generic::SignedPayload, traits::IdentifyAccount, MultiAddress};
use sp_session::MembershipProof;
use std::time::Duration;

pub use codegen_runtime::api::runtime_types;

pub type RuntimeCall = runtime_types::polkadot_runtime::RuntimeCall;

pub type GrandpaCall = runtime_types::pallet_grandpa::pallet::Call;

/// Polkadot header id.
pub type HeaderId = relay_utils::HeaderId<bp_polkadot::Hash, bp_polkadot::BlockNumber>;

/// Polkadot header type used in headers sync.
pub type SyncHeader = relay_substrate_client::SyncHeader<bp_polkadot::Header>;

/// The address format for describing accounts.
pub type Address = MultiAddress<bp_polkadot::AccountId, ()>;

/// Polkadot chain definition
#[derive(Debug, Clone, Copy)]
pub struct Polkadot;

impl UnderlyingChainProvider for Polkadot {
	type Chain = bp_polkadot::Polkadot;
}

impl Chain for Polkadot {
	const NAME: &'static str = "Polkadot";
	const BEST_FINALIZED_HEADER_ID_METHOD: &'static str =
		bp_polkadot::BEST_FINALIZED_POLKADOT_HEADER_METHOD;
	const FREE_HEADERS_INTERVAL_METHOD: &'static str =
		bp_polkadot::FREE_HEADERS_INTERVAL_FOR_POLKADOT_METHOD;
	const AVERAGE_BLOCK_INTERVAL: Duration = Duration::from_secs(6);

	type SignedBlock = bp_polkadot::SignedBlock;
	type Call = RuntimeCall;
}

impl ChainWithGrandpa for Polkadot {
	const SYNCED_HEADERS_GRANDPA_INFO_METHOD: &'static str =
		POLKADOT_SYNCED_HEADERS_GRANDPA_INFO_METHOD;

	type KeyOwnerProof = MembershipProof;
}

impl ChainWithBalances for Polkadot {
	fn account_info_storage_key(account_id: &Self::AccountId) -> StorageKey {
		AccountInfoStorageMapKeyProvider::final_key(account_id)
	}
}

impl RelayChain for Polkadot {
	const PARAS_PALLET_NAME: &'static str = bp_polkadot::PARAS_PALLET_NAME;
	const WITH_CHAIN_BRIDGE_PARACHAINS_PALLET_NAME: &'static str =
		bp_polkadot::WITH_POLKADOT_BRIDGE_PARACHAINS_PALLET_NAME;
}

impl ChainWithTransactions for Polkadot {
	type AccountKeyPair = sp_core::sr25519::Pair;
	type SignedTransaction =
		bp_polkadot_core::UncheckedExtrinsic<Self::Call, bp_polkadot::TransactionExtension>;

	fn sign_transaction(
		param: SignParam<Self>,
		unsigned: UnsignedTransaction<Self>,
	) -> Result<Self::SignedTransaction, SubstrateError> {
		let raw_payload = SignedPayload::new(
			unsigned.call,
			bp_polkadot::TransactionExtension::from_params(
				param.spec_version,
				param.transaction_version,
				unsigned.era,
				param.genesis_hash,
				unsigned.nonce,
				unsigned.tip,
				((), ()),
			),
		)?;

		let signature = raw_payload.using_encoded(|payload| param.signer.sign(payload));
		let signer: sp_runtime::MultiSigner = param.signer.public().into();
		let (call, extra, _) = raw_payload.deconstruct();

		Ok(Self::SignedTransaction::new_signed(
			call,
			signer.into_account().into(),
			signature.into(),
			extra,
		))
	}
}
