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

//! Polkadot-to-Kusama headers sync entrypoint.

use codec::Encode;
use sp_core::{Bytes, Pair};

use bp_header_chain::justification::GrandpaJustification;
use relay_kusama_client::{Kusama, SigningParams as KusamaSigningParams};
use relay_polkadot_client::{Polkadot, SyncHeader as PolkadotSyncHeader};
use relay_substrate_client::{Chain, Client, TransactionSignScheme};
use relay_utils::metrics::MetricsParams;
use substrate_relay_helper::finality_pipeline::{SubstrateFinalitySyncPipeline, SubstrateFinalityToSubstrate};

/// Maximal saturating difference between `balance(now)` and `balance(now-24h)` to treat
/// relay as gone wild.
pub(crate) const MAXIMAL_BALANCE_DECREASE_PER_DAY: bp_kusama::Balance = 0; // TODO

/// Polkadot-to-Kusama finality sync pipeline.
pub(crate) type FinalityPipelinePolkadotFinalityToKusama =
	SubstrateFinalityToSubstrate<Polkadot, Kusama, KusamaSigningParams>;

#[derive(Clone, Debug)]
pub(crate) struct PolkadotFinalityToKusama {
	finality_pipeline: FinalityPipelinePolkadotFinalityToKusama,
}

impl PolkadotFinalityToKusama {
	pub fn new(target_client: Client<Kusama>, target_sign: KusamaSigningParams) -> Self {
		Self {
			finality_pipeline: FinalityPipelinePolkadotFinalityToKusama::new(target_client, target_sign),
		}
	}
}

impl SubstrateFinalitySyncPipeline for PolkadotFinalityToKusama {
	type FinalitySyncPipeline = FinalityPipelinePolkadotFinalityToKusama;

	const BEST_FINALIZED_SOURCE_HEADER_ID_AT_TARGET: &'static str = bp_polkadot::BEST_FINALIZED_POLKADOT_HEADER_METHOD;

	type TargetChain = Kusama;

	fn customize_metrics(params: MetricsParams) -> anyhow::Result<MetricsParams> {
		crate::chains::add_polkadot_kusama_price_metrics::<Self::FinalitySyncPipeline>(
			Some(finality_relay::metrics_prefix::<Self::FinalitySyncPipeline>()),
			params,
		)
	}

	fn start_relay_guards(&self) {
		relay_substrate_client::guard::abort_on_spec_version_change(
			self.finality_pipeline.target_client.clone(),
			bp_kusama::VERSION.spec_version,
		);
		relay_substrate_client::guard::abort_when_account_balance_decreased(
			self.finality_pipeline.target_client.clone(),
			self.transactions_author(),
			MAXIMAL_BALANCE_DECREASE_PER_DAY,
		);
	}

	fn transactions_author(&self) -> bp_kusama::AccountId {
		(*self.finality_pipeline.target_sign.public().as_array_ref()).into()
	}

	fn make_submit_finality_proof_transaction(
		&self,
		era: bp_runtime::TransactionEraOf<Kusama>,
		transaction_nonce: <Kusama as Chain>::Index,
		header: PolkadotSyncHeader,
		proof: GrandpaJustification<bp_polkadot::Header>,
	) -> Bytes {
		let call = relay_kusama_client::runtime::Call::BridgePolkadotGrandpa(
			relay_kusama_client::runtime::BridgePolkadotGrandpaCall::submit_finality_proof(header.into_inner(), proof),
		);
		let genesis_hash = *self.finality_pipeline.target_client.genesis_hash();
		let transaction = Kusama::sign_transaction(
			genesis_hash,
			&self.finality_pipeline.target_sign,
			era,
			transaction_nonce,
			call,
		);

		Bytes(transaction.encode())
	}
}
