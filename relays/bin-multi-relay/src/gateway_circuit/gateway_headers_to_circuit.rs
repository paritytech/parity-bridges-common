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

//! Gateway-to-Circuit headers sync entrypoint.

use super::{CircuitClient, GatewayClient};
use crate::finality_pipeline::{SubstrateFinalitySyncPipeline, SubstrateFinalityToSubstrate};

use codec::Encode;
use relay_circuit_client::{Circuit, SigningParams as CircuitSigningParams};
use relay_gateway_client::{Gateway, SyncHeader as GatewaySyncHeader};
use relay_substrate_client::{finality_source::Justification, Chain, TransactionSignScheme};
use sp_core::{Bytes, Pair};

/// Gateway-to-Circuit finality sync pipeline.
pub(crate) type GatewayFinalityToCircuit = SubstrateFinalityToSubstrate<Gateway, Circuit, CircuitSigningParams>;

impl SubstrateFinalitySyncPipeline for GatewayFinalityToCircuit {
	const BEST_FINALIZED_SOURCE_HEADER_ID_AT_TARGET: &'static str = bp_gateway::BEST_FINALIZED_GATEWAY_HEADER_METHOD;

	type TargetChain = Circuit;

	fn transactions_author(&self) -> bp_circuit::AccountId {
		self.target_sign.signer.public().as_array_ref().clone().into()
	}

	fn make_submit_finality_proof_transaction(
		&self,
		transaction_nonce: <Circuit as Chain>::Index,
		header: GatewaySyncHeader,
		proof: Justification<bp_gateway::BlockNumber>,
	) -> Bytes {
		let call = circuit_runtime::BridgeGrandpaGatewayCall::<
			circuit_runtime::Runtime,
			circuit_runtime::GatewayGrandpaInstance,
		>::submit_finality_proof(header.into_inner(), proof.into_inner())
		.into();

		let genesis_hash = *self.target_client.genesis_hash();
		let transaction = Circuit::sign_transaction(genesis_hash, &self.target_sign.signer, transaction_nonce, call);

		Bytes(transaction.encode())
	}
}

/// Run Gateway-to-Circuit finality sync.
pub async fn run(
	gateway_client: GatewayClient,
	circuit_client: CircuitClient,
	circuit_sign: CircuitSigningParams,
	metrics_params: Option<relay_utils::metrics::MetricsParams>,
) -> Result<(), String> {
	crate::finality_pipeline::run(
		GatewayFinalityToCircuit::new(circuit_client.clone(), circuit_sign),
		gateway_client,
		circuit_client,
		metrics_params,
	)
	.await
}
