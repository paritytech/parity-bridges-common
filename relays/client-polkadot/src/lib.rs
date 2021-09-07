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

use codec::Encode;
use relay_substrate_client::{Chain, ChainBase, ChainWithBalances, TransactionSignScheme};
use sp_core::{storage::StorageKey, Pair};
use sp_runtime::{generic::SignedPayload, traits::IdentifyAccount};
use std::time::Duration;

pub mod runtime;

/// Polkadot header id.
pub type HeaderId = relay_utils::HeaderId<bp_polkadot::Hash, bp_polkadot::BlockNumber>;

/// Polkadot chain definition
#[derive(Debug, Clone, Copy)]
pub struct Polkadot;

impl ChainBase for Polkadot {
	type BlockNumber = bp_polkadot::BlockNumber;
	type Hash = bp_polkadot::Hash;
	type Hasher = bp_polkadot::Hasher;
	type Header = bp_polkadot::Header;
}

impl Chain for Polkadot {
	const NAME: &'static str = "Polkadot";
	const AVERAGE_BLOCK_INTERVAL: Duration = Duration::from_secs(6);
	const STORAGE_PROOF_OVERHEAD: u32 = bp_polkadot::EXTRA_STORAGE_PROOF_SIZE;
	const MAXIMAL_ENCODED_ACCOUNT_ID_SIZE: u32 = bp_polkadot::MAXIMAL_ENCODED_ACCOUNT_ID_SIZE;

	type AccountId = bp_polkadot::AccountId;
	type Index = bp_polkadot::Nonce;
	type SignedBlock = bp_polkadot::SignedBlock;
	type Call = crate::runtime::Call;
	type Balance = bp_polkadot::Balance;
	type WeightToFee = bp_polkadot::WeightToFee;
}

impl ChainWithBalances for Polkadot {
	fn account_info_storage_key(account_id: &Self::AccountId) -> StorageKey {
		StorageKey(bp_polkadot::account_info_storage_key(account_id))
	}
}

impl TransactionSignScheme for Polkadot {
	type Chain = Polkadot;
	type AccountKeyPair = sp_core::sr25519::Pair;
	type SignedTransaction = crate::runtime::UncheckedExtrinsic;

	fn sign_transaction(
		genesis_hash: <Self::Chain as ChainBase>::Hash,
		signer: &Self::AccountKeyPair,
		era: bp_runtime::TransactionEraOf<Self::Chain>,
		signer_nonce: <Self::Chain as Chain>::Index,
		call: <Self::Chain as Chain>::Call,
	) -> Self::SignedTransaction {
		let raw_payload = SignedPayload::new(
			call,
			bp_polkadot::SignedExtensions::new(bp_polkadot::VERSION, era, genesis_hash, signer_nonce, 0),
		)
		.expect("SignedExtension never fails.");

		let signature = raw_payload.using_encoded(|payload| signer.sign(payload));
		let signer: sp_runtime::MultiSigner = signer.public().into();
		let (call, extra, _) = raw_payload.deconstruct();

		bp_polkadot::UncheckedExtrinsic::new_signed(
			call,
			sp_runtime::MultiAddress::Id(signer.into_account()),
			signature.into(),
			extra,
		)
	}
}

/// Polkadot header type used in headers sync.
pub type SyncHeader = relay_substrate_client::SyncHeader<bp_polkadot::Header>;

/// Polkadot signing params.
pub type SigningParams = sp_core::sr25519::Pair;
