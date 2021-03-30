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

use codec::Encode;
use relay_millau_client::{Millau, SigningParams as MillauSigningParams};
use relay_substrate_client::{
	finality_source::Justification, metrics::FloatJsonValueMetric, Chain, TransactionSignScheme,
};
use relay_westend_client::{SyncHeader as WestendSyncHeader, Westend};
use sp_core::{Bytes, Pair};

/// Westend-to-Millau finality sync pipeline.
pub(crate) type WestendFinalityToMillau = SubstrateFinalityToSubstrate<Westend, Millau, MillauSigningParams>;

impl SubstrateFinalitySyncPipeline for WestendFinalityToMillau {
	const BEST_FINALIZED_SOURCE_HEADER_ID_AT_TARGET: &'static str = bp_westend::BEST_FINALIZED_WESTEND_HEADER_METHOD;

	type TargetChain = Millau;

	fn transactions_author(&self) -> bp_millau::AccountId {
		self.target_sign.signer.public().as_array_ref().clone().into()
	}

	fn make_submit_finality_proof_transaction(
		&self,
		transaction_nonce: <Millau as Chain>::Index,
		header: WestendSyncHeader,
		proof: Justification<bp_westend::BlockNumber>,
	) -> Bytes {
		let call = millau_runtime::BridgeGrandpaWestendCall::<
			millau_runtime::Runtime,
			millau_runtime::WestendGrandpaInstance,
		>::submit_finality_proof(header.into_inner(), proof.into_inner())
		.into();

		let genesis_hash = *self.target_client.genesis_hash();
		let transaction = Millau::sign_transaction(genesis_hash, &self.target_sign.signer, transaction_nonce, call);

		Bytes(transaction.encode())
	}
}

/// Run Westend-to-Millau finality sync.
pub async fn run(
	westend_client: WestendClient,
	millau_client: MillauClient,
	millau_sign: MillauSigningParams,
	metrics_params: relay_utils::metrics::MetricsParams,
) -> Result<(), String> {
	crate::finality_pipeline::run(
		WestendFinalityToMillau::new(millau_client.clone(), millau_sign),
		westend_client,
		millau_client.clone(),
		relay_utils::relay_metrics(
			finality_relay::metrics_prefix::<WestendFinalityToMillau>(),
			metrics_params.address,
		)
		.standalone_metric(FloatJsonValueMetric::new(
			"https://api.coingecko.com/api/v3/simple/price?ids=Polkadot&vs_currencies=usd".into(),
			"$.polkadot.usd".into(),
			"polkadot_price".into(),
			"Polkadot price in USD".into(),
		))?
		.standalone_metric(FloatJsonValueMetric::new(
			"https://api.coingecko.com/api/v3/simple/price?ids=Kusama&vs_currencies=usd".into(),
			"$.kusama.usd".into(),
			"kusama_price".into(),
			"Kusama price in USD".into(),
		))?
		.into_params(),
	)
	.await
}
