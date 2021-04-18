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

//! Gateway-to-Circuit messages sync entrypoint.

use crate::messages_lane::{
	select_delivery_transaction_limits, MessagesRelayParams, SubstrateMessageLane, SubstrateMessageLaneToSubstrate,
};
use crate::messages_source::SubstrateMessagesSource;
use crate::messages_target::SubstrateMessagesTarget;

use bp_messages::MessageNonce;
use bp_runtime::{CIRCUIT_BRIDGE_INSTANCE, GATEWAY_BRIDGE_INSTANCE};
use bridge_runtime_common::messages::target::FromBridgedChainMessagesProof;
use codec::Encode;
use frame_support::dispatch::GetDispatchInfo;
use messages_relay::message_lane::MessageLane;
use relay_circuit_client::{Circuit, HeaderId as CircuitHeaderId, SigningParams as CircuitSigningParams};
use relay_gateway_client::{Gateway, HeaderId as GatewayHeaderId, SigningParams as GatewaySigningParams};
use relay_substrate_client::{
	metrics::{FloatStorageValueMetric, StorageProofOverheadMetric},
	Chain, TransactionSignScheme,
};
use sp_core::{Bytes, Pair};
use std::{ops::RangeInclusive, time::Duration};

/// Gateway-to-Circuit message lane.
pub type GatewayMessagesToCircuit =
	SubstrateMessageLaneToSubstrate<Gateway, GatewaySigningParams, Circuit, CircuitSigningParams>;

impl SubstrateMessageLane for GatewayMessagesToCircuit {
	const OUTBOUND_LANE_MESSAGES_DISPATCH_WEIGHT_METHOD: &'static str =
		bp_circuit::TO_CIRCUIT_MESSAGES_DISPATCH_WEIGHT_METHOD;
	const OUTBOUND_LANE_LATEST_GENERATED_NONCE_METHOD: &'static str =
		bp_circuit::TO_CIRCUIT_LATEST_GENERATED_NONCE_METHOD;
	const OUTBOUND_LANE_LATEST_RECEIVED_NONCE_METHOD: &'static str =
		bp_circuit::TO_CIRCUIT_LATEST_RECEIVED_NONCE_METHOD;

	const INBOUND_LANE_LATEST_RECEIVED_NONCE_METHOD: &'static str =
		bp_gateway::FROM_GATEWAY_LATEST_RECEIVED_NONCE_METHOD;
	const INBOUND_LANE_LATEST_CONFIRMED_NONCE_METHOD: &'static str =
		bp_gateway::FROM_GATEWAY_LATEST_CONFIRMED_NONCE_METHOD;
	const INBOUND_LANE_UNREWARDED_RELAYERS_STATE: &'static str = bp_gateway::FROM_GATEWAY_UNREWARDED_RELAYERS_STATE;

	const BEST_FINALIZED_SOURCE_HEADER_ID_AT_TARGET: &'static str = bp_gateway::BEST_FINALIZED_GATEWAY_HEADER_METHOD;
	const BEST_FINALIZED_TARGET_HEADER_ID_AT_SOURCE: &'static str = bp_circuit::BEST_FINALIZED_CIRCUIT_HEADER_METHOD;

	type SourceChain = Gateway;
	type TargetChain = Circuit;

	fn source_transactions_author(&self) -> bp_gateway::AccountId {
		(*self.source_sign.public().as_array_ref()).into()
	}

	fn make_messages_receiving_proof_transaction(
		&self,
		transaction_nonce: <Gateway as Chain>::Index,
		_generated_at_block: CircuitHeaderId,
		proof: <Self as MessageLane>::MessagesReceivingProof,
	) -> Bytes {
		let (relayers_state, proof) = proof;
		let call: gateway_runtime::Call =
			gateway_runtime::MessagesCall::receive_messages_delivery_proof(proof, relayers_state).into();
		let call_weight = call.get_dispatch_info().weight;
		let genesis_hash = *self.source_client.genesis_hash();
		let transaction = Gateway::sign_transaction(genesis_hash, &self.source_sign, transaction_nonce, call);
		log::trace!(
			target: "bridge",
			"Prepared Circuit -> Gateway confirmation transaction. Weight: {}/{}, size: {}/{}",
			call_weight,
			bp_gateway::max_extrinsic_weight(),
			transaction.encode().len(),
			bp_gateway::max_extrinsic_size(),
		);
		Bytes(transaction.encode())
	}

	fn target_transactions_author(&self) -> bp_gateway::AccountId {
		(*self.target_sign.public().as_array_ref()).into()
	}

	fn make_messages_delivery_transaction(
		&self,
		transaction_nonce: <Circuit as Chain>::Index,
		_generated_at_header: GatewayHeaderId,
		_nonces: RangeInclusive<MessageNonce>,
		proof: <Self as MessageLane>::MessagesProof,
	) -> Bytes {
		let (dispatch_weight, proof) = proof;
		let FromBridgedChainMessagesProof {
			ref nonces_start,
			ref nonces_end,
			..
		} = proof;
		let messages_count = nonces_end - nonces_start + 1;
		let call: circuit_runtime::Call = circuit_runtime::MessagesCall::receive_messages_proof(
			self.relayer_id_at_source.clone(),
			proof,
			messages_count as _,
			dispatch_weight,
		)
		.into();
		let call_weight = call.get_dispatch_info().weight;
		let genesis_hash = *self.target_client.genesis_hash();
		let transaction = Circuit::sign_transaction(genesis_hash, &self.target_sign, transaction_nonce, call);
		log::trace!(
			target: "bridge",
			"Prepared Gateway -> Circuit delivery transaction. Weight: {}/{}, size: {}/{}",
			call_weight,
			bp_circuit::max_extrinsic_weight(),
			transaction.encode().len(),
			bp_circuit::max_extrinsic_size(),
		);
		Bytes(transaction.encode())
	}
}

/// Gateway node as messages source.
type GatewaySourceClient = SubstrateMessagesSource<
	Gateway,
	GatewayMessagesToCircuit,
	gateway_runtime::Runtime,
	gateway_runtime::WithCircuitMessagesInstance,
>;

/// Circuit node as messages target.
type CircuitTargetClient = SubstrateMessagesTarget<
	Circuit,
	GatewayMessagesToCircuit,
	circuit_runtime::Runtime,
	circuit_runtime::WithGatewayMessagesInstance,
>;

/// Run Gateway-to-Circuit messages sync.
pub async fn run(
	params: MessagesRelayParams<Gateway, GatewaySigningParams, Circuit, CircuitSigningParams>,
) -> Result<(), String> {
	let stall_timeout = Duration::from_secs(5 * 60);
	let relayer_id_at_gateway = (*params.source_sign.public().as_array_ref()).into();

	let lane_id = params.lane_id;
	let source_client = params.source_client;
	let lane = GatewayMessagesToCircuit {
		source_client: source_client.clone(),
		source_sign: params.source_sign,
		target_client: params.target_client.clone(),
		target_sign: params.target_sign,
		relayer_id_at_source: relayer_id_at_gateway,
	};

	// 2/3 is reserved for proofs and tx overhead
	let max_messages_size_in_single_batch = bp_circuit::max_extrinsic_size() as usize / 3;
	let (max_messages_in_single_batch, max_messages_weight_in_single_batch) =
		select_delivery_transaction_limits::<pallet_bridge_messages::weights::RialtoWeight<gateway_runtime::Runtime>>(
			bp_circuit::max_extrinsic_weight(),
			bp_circuit::MAX_UNREWARDED_RELAYER_ENTRIES_AT_INBOUND_LANE,
		);

	log::info!(
		target: "bridge",
		"Starting Gateway -> Circuit messages relay.\n\t\
			Gateway relayer account id: {:?}\n\t\
			Max messages in single transaction: {}\n\t\
			Max messages size in single transaction: {}\n\t\
			Max messages weight in single transaction: {}",
		lane.relayer_id_at_source,
		max_messages_in_single_batch,
		max_messages_size_in_single_batch,
		max_messages_weight_in_single_batch,
	);

	messages_relay::message_lane_loop::run(
		messages_relay::message_lane_loop::Params {
			lane: lane_id,
			source_tick: Gateway::AVERAGE_BLOCK_INTERVAL,
			target_tick: Circuit::AVERAGE_BLOCK_INTERVAL,
			reconnect_delay: relay_utils::relay_loop::RECONNECT_DELAY,
			stall_timeout,
			delivery_params: messages_relay::message_lane_loop::MessageDeliveryParams {
				max_unrewarded_relayer_entries_at_target: bp_circuit::MAX_UNREWARDED_RELAYER_ENTRIES_AT_INBOUND_LANE,
				max_unconfirmed_nonces_at_target: bp_circuit::MAX_UNCONFIRMED_MESSAGES_AT_INBOUND_LANE,
				max_messages_in_single_batch,
				max_messages_weight_in_single_batch,
				max_messages_size_in_single_batch,
			},
		},
		GatewaySourceClient::new(
			source_client.clone(),
			lane.clone(),
			lane_id,
			CIRCUIT_BRIDGE_INSTANCE,
			params.target_to_source_headers_relay,
		),
		CircuitTargetClient::new(
			params.target_client,
			lane,
			lane_id,
			GATEWAY_BRIDGE_INSTANCE,
			params.source_to_target_headers_relay,
		),
		relay_utils::relay_metrics(
			Some(messages_relay::message_lane_loop::metrics_prefix::<
				GatewayMessagesToCircuit,
			>(&lane_id)),
			params.metrics_params,
		)
		.standalone_metric(|registry, prefix| {
			StorageProofOverheadMetric::new(
				registry,
				prefix,
				source_client.clone(),
				"gateway_storage_proof_overhead".into(),
				"Gateway storage proof overhead".into(),
			)
		})?
		.standalone_metric(|registry, prefix| {
			FloatStorageValueMetric::<_, sp_runtime::FixedU128>::new(
				registry,
				prefix,
				source_client,
				sp_core::storage::StorageKey(
					gateway_runtime::circuit_messages::CircuitToGatewayConversionRate::key().to_vec(),
				),
				Some(gateway_runtime::circuit_messages::INITIAL_CIRCUIT_TO_GATEWAY_CONVERSION_RATE),
				"gateway_circuit_to_gateway_conversion_rate".into(),
				"Circuit to Gateway tokens conversion rate (used by Circuit)".into(),
			)
		})?
		.into_params(),
		futures::future::pending(),
	)
	.await
}
