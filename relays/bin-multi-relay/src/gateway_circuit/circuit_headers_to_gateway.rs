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

//! Circuit-to-Gateway headers sync entrypoint.

use super::{CircuitClient, GatewayClient};
use crate::finality_pipeline::{SubstrateFinalitySyncPipeline, SubstrateFinalityToSubstrate};

use codec::Encode;
use relay_circuit_client::{Circuit, SyncHeader as CircuitSyncHeader};
use relay_gateway_client::{Gateway, SigningParams as GatewaySigningParams};
use relay_substrate_client::{finality_source::Justification, Chain, TransactionSignScheme};
use sp_core::{Bytes, Pair};

/// Circuit-to-Gateway finality sync pipeline.
pub(crate) type CircuitFinalityToGateway = SubstrateFinalityToSubstrate<Circuit, Gateway, GatewaySigningParams>;

impl SubstrateFinalitySyncPipeline for CircuitFinalityToGateway {
	const BEST_FINALIZED_SOURCE_HEADER_ID_AT_TARGET: &'static str = bp_circuit::BEST_FINALIZED_CIRCUIT_HEADER_METHOD;

	type TargetChain = Gateway;

	fn transactions_author(&self) -> bp_gateway::AccountId {
		self.target_sign.signer.public().as_array_ref().clone().into()
	}

	fn make_submit_finality_proof_transaction(
		&self,
		transaction_nonce: <Gateway as Chain>::Index,
		header: CircuitSyncHeader,
		proof: Justification<bp_circuit::BlockNumber>,
	) -> Bytes {
		let call =
			gateway_runtime::BridgeGrandpaCircuitCall::submit_finality_proof(header.into_inner(), proof.into_inner())
				.into();

		let genesis_hash = *self.target_client.genesis_hash();
		let transaction = Gateway::sign_transaction(genesis_hash, &self.target_sign.signer, transaction_nonce, call);

		Bytes(transaction.encode())
	}
}

/// Run Circuit-to-Gateway finality sync.
pub async fn run(
	circuit_client: CircuitClient,
	gateway_client: GatewayClient,
	gateway_sign: GatewaySigningParams,
	metrics_params: Option<relay_utils::metrics::MetricsParams>,
) -> Result<(), String> {
	crate::finality_pipeline::run(
		CircuitFinalityToGateway::new(gateway_client.clone(), gateway_sign),
		circuit_client,
		gateway_client,
		metrics_params,
	)
	.await
}
