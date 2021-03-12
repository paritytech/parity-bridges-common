// Copyright 2019-2020 Parity Technologies (UK) Ltd.
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

//! Westend-to-Millau headers sync entrypoint.

use super::{MillauClient, WestendClient};
use crate::finality_pipeline::{SubstrateFinalitySyncPipeline, SubstrateFinalityToSubstrate};

use async_trait::async_trait;
use relay_millau_client::{Millau, SigningParams as MillauSigningParams};
use relay_substrate_client::{finality_source::Justification, Error as SubstrateError, TransactionSignScheme};
use relay_westend_client::{SyncHeader as WestendSyncHeader, Westend};
use sp_core::Pair;

/// Westend-to-Millau finality sync pipeline.
pub(crate) type WestendFinalityToMillau = SubstrateFinalityToSubstrate<Westend, Millau, MillauSigningParams>;

#[async_trait]
impl SubstrateFinalitySyncPipeline for WestendFinalityToMillau {
	const BEST_FINALIZED_SOURCE_HEADER_ID_AT_TARGET: &'static str = bp_westend::BEST_FINALIZED_WESTEND_HEADER_METHOD;

	type SignedTransaction = <Millau as TransactionSignScheme>::SignedTransaction;

	async fn make_submit_finality_proof_transaction(
		&self,
		header: WestendSyncHeader,
		proof: Justification<bp_rialto::BlockNumber>,
	) -> Result<Self::SignedTransaction, SubstrateError> {
		let account_id = self.target_sign.signer.public().as_array_ref().clone().into();
		let nonce = self.target_client.next_account_index(account_id).await?;

		let call = millau_runtime::FinalityBridgeRialtoCall::<
			millau_runtime::Runtime,
			millau_runtime::WestendFinalityVerifierInstance,
		>::submit_finality_proof(header.into_inner(), proof.into_inner())
		.into();

		let genesis_hash = *self.target_client.genesis_hash();
		let transaction = Millau::sign_transaction(genesis_hash, &self.target_sign.signer, nonce, call);

		Ok(transaction)
	}
}

/// Run Westend-to-Millau finality sync.
pub async fn run(
	westend_client: WestendClient,
	millau_client: MillauClient,
	millau_sign: MillauSigningParams,
	metrics_params: Option<relay_utils::metrics::MetricsParams>,
) {
	crate::finality_pipeline::run(
		WestendFinalityToMillau::new(millau_client.clone(), millau_sign),
		westend_client,
		millau_client,
		metrics_params,
	)
	.await;
}
