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

//! Everything required to serve Circuit <-> Gateway messages.

use crate::Runtime;

use bp_messages::{
	source_chain::TargetHeaderChain,
	target_chain::{ProvedMessages, SourceHeaderChain},
	InboundLaneData, LaneId, Message, MessageNonce, Parameter as MessagesParameter,
};
use bp_runtime::{InstanceId, CIRCUIT_BRIDGE_INSTANCE};
use bridge_runtime_common::messages::{self, ChainWithMessages, MessageBridge, MessageTransaction};
use codec::{Decode, Encode};
use frame_support::{
	parameter_types,
	weights::{DispatchClass, Weight},
	RuntimeDebug,
};
use sp_core::storage::StorageKey;
use sp_runtime::{FixedPointNumber, FixedU128};
use sp_std::{convert::TryFrom, ops::RangeInclusive};

/// Initial value of `RialtoToMillauConversionRate` parameter.
pub const INITIAL_CIRCUIT_TO_GATEWAY_CONVERSION_RATE: FixedU128 = FixedU128::from_inner(FixedU128::DIV);

parameter_types! {
	/// Circuit to Gateway conversion rate. Initially we treat both tokens as equal.
	pub storage CircuitToGatewayConversionRate: FixedU128 = INITIAL_CIRCUIT_TO_GATEWAY_CONVERSION_RATE;
}

/// Storage key of the Gateway -> Circuit message in the runtime storage.
pub fn message_key(lane: &LaneId, nonce: MessageNonce) -> StorageKey {
	pallet_bridge_messages::storage_keys::message_key::<Runtime, <Gateway as ChainWithMessages>::MessagesInstance>(
		lane, nonce,
	)
}

/// Storage key of the Gateway -> Circuit message lane state in the runtime storage.
pub fn outbound_lane_data_key(lane: &LaneId) -> StorageKey {
	pallet_bridge_messages::storage_keys::outbound_lane_data_key::<<Gateway as ChainWithMessages>::MessagesInstance>(
		lane,
	)
}

/// Storage key of the Circuit -> Gateway message lane state in the runtime storage.
pub fn inbound_lane_data_key(lane: &LaneId) -> StorageKey {
	pallet_bridge_messages::storage_keys::inbound_lane_data_key::<
		Runtime,
		<Gateway as ChainWithMessages>::MessagesInstance,
	>(lane)
}

/// Message payload for Gateway -> Circuit messages.
pub type ToCircuitMessagePayload = messages::source::FromThisChainMessagePayload<WithCircuitMessageBridge>;

/// Message verifier for Gateway -> Circuit messages.
pub type ToCircuitMessageVerifier = messages::source::FromThisChainMessageVerifier<WithCircuitMessageBridge>;

/// Message payload for Circuit -> Gateway messages.
pub type FromCircuitMessagePayload = messages::target::FromBridgedChainMessagePayload<WithCircuitMessageBridge>;

/// Encoded Gateway Call as it comes from Circuit.
pub type FromCircuitEncodedCall = messages::target::FromBridgedChainEncodedMessageCall<WithCircuitMessageBridge>;

/// Call-dispatch based message dispatch for Circuit -> Gateway messages.
pub type FromCircuitMessageDispatch = messages::target::FromBridgedChainMessageDispatch<
	WithCircuitMessageBridge,
	crate::Runtime,
	pallet_bridge_dispatch::DefaultInstance,
>;

/// Messages proof for Circuit -> Gateway messages.
pub type FromCircuitMessagesProof = messages::target::FromBridgedChainMessagesProof<bp_circuit::Hash>;

/// Messages delivery proof for Gateway -> Circuit messages.
pub type ToCircuitMessagesDeliveryProof = messages::source::FromBridgedChainMessagesDeliveryProof<bp_circuit::Hash>;

/// Circuit <-> Gateway message bridge.
#[derive(RuntimeDebug, Clone, Copy)]
pub struct WithCircuitMessageBridge;

impl MessageBridge for WithCircuitMessageBridge {
	const INSTANCE: InstanceId = CIRCUIT_BRIDGE_INSTANCE;

	const RELAYER_FEE_PERCENT: u32 = 10;

	type ThisChain = Gateway;
	type BridgedChain = Circuit;

	fn bridged_balance_to_this_balance(bridged_balance: bp_circuit::Balance) -> bp_gateway::Balance {
		bp_gateway::Balance::try_from(CircuitToGatewayConversionRate::get().saturating_mul_int(bridged_balance))
			.unwrap_or(bp_gateway::Balance::MAX)
	}
}

/// Gateway chain from message lane point of view.
#[derive(RuntimeDebug, Clone, Copy)]
pub struct Gateway;

impl messages::ChainWithMessages for Gateway {
	type Hash = bp_gateway::Hash;
	type AccountId = bp_gateway::AccountId;
	type Signer = bp_gateway::AccountSigner;
	type Signature = bp_gateway::Signature;
	type Weight = Weight;
	type Balance = bp_gateway::Balance;

	type MessagesInstance = crate::WithCircuitMessagesInstance;
}

impl messages::ThisChainWithMessages for Gateway {
	type Call = crate::Call;

	fn is_outbound_lane_enabled(lane: &LaneId) -> bool {
		*lane == LaneId::default()
	}

	fn maximal_pending_messages_at_outbound_lane() -> MessageNonce {
		MessageNonce::MAX
	}

	fn estimate_delivery_confirmation_transaction() -> MessageTransaction<Weight> {
		let inbound_data_size =
			InboundLaneData::<bp_gateway::AccountId>::encoded_size_hint(bp_gateway::MAXIMAL_ENCODED_ACCOUNT_ID_SIZE, 1)
				.unwrap_or(u32::MAX);

		MessageTransaction {
			dispatch_weight: bp_gateway::MAX_SINGLE_MESSAGE_DELIVERY_CONFIRMATION_TX_WEIGHT,
			size: inbound_data_size
				.saturating_add(bp_circuit::EXTRA_STORAGE_PROOF_SIZE)
				.saturating_add(bp_gateway::TX_EXTRA_BYTES),
		}
	}

	fn transaction_payment(transaction: MessageTransaction<Weight>) -> bp_gateway::Balance {
		// in our testnets, both per-byte fee and weight-to-fee are 1:1
		messages::transaction_payment(
			bp_gateway::BlockWeights::get()
				.get(DispatchClass::Normal)
				.base_extrinsic,
			1,
			FixedU128::zero(),
			|weight| weight as _,
			transaction,
		)
	}
}

/// Circuit chain from message lane point of view.
#[derive(RuntimeDebug, Clone, Copy)]
pub struct Circuit;

impl messages::ChainWithMessages for Circuit {
	type Hash = bp_circuit::Hash;
	type AccountId = bp_circuit::AccountId;
	type Signer = bp_circuit::AccountSigner;
	type Signature = bp_circuit::Signature;
	type Weight = Weight;
	type Balance = bp_circuit::Balance;

	type MessagesInstance = pallet_bridge_messages::DefaultInstance;
}

impl messages::BridgedChainWithMessages for Circuit {
	fn maximal_extrinsic_size() -> u32 {
		bp_circuit::max_extrinsic_size()
	}

	fn message_weight_limits(_message_payload: &[u8]) -> RangeInclusive<Weight> {
		// we don't want to relay too large messages + keep reserve for future upgrades
		let upper_limit =
			messages::target::maximal_incoming_message_dispatch_weight(bp_circuit::max_extrinsic_weight());

		// we're charging for payload bytes in `WithCircuitMessageBridge::transaction_payment` function
		//
		// this bridge may be used to deliver all kind of messages, so we're not making any assumptions about
		// minimal dispatch weight here

		0..=upper_limit
	}

	fn estimate_delivery_transaction(
		message_payload: &[u8],
		message_dispatch_weight: Weight,
	) -> MessageTransaction<Weight> {
		let message_payload_len = u32::try_from(message_payload.len()).unwrap_or(u32::MAX);
		let extra_bytes_in_payload = Weight::from(message_payload_len)
			.saturating_sub(pallet_bridge_messages::EXPECTED_DEFAULT_MESSAGE_LENGTH.into());

		MessageTransaction {
			dispatch_weight: extra_bytes_in_payload
				.saturating_mul(bp_circuit::ADDITIONAL_MESSAGE_BYTE_DELIVERY_WEIGHT)
				.saturating_add(bp_circuit::DEFAULT_MESSAGE_DELIVERY_TX_WEIGHT)
				.saturating_add(message_dispatch_weight),
			size: message_payload_len
				.saturating_add(bp_gateway::EXTRA_STORAGE_PROOF_SIZE)
				.saturating_add(bp_circuit::TX_EXTRA_BYTES),
		}
	}

	fn transaction_payment(transaction: MessageTransaction<Weight>) -> bp_circuit::Balance {
		// in our testnets, both per-byte fee and weight-to-fee are 1:1
		messages::transaction_payment(
			bp_circuit::BlockWeights::get()
				.get(DispatchClass::Normal)
				.base_extrinsic,
			1,
			FixedU128::zero(),
			|weight| weight as _,
			transaction,
		)
	}
}

impl TargetHeaderChain<ToCircuitMessagePayload, bp_circuit::AccountId> for Circuit {
	type Error = &'static str;
	// The proof is:
	// - hash of the header this proof has been created with;
	// - the storage proof of one or several keys;
	// - id of the lane we prove state of.
	type MessagesDeliveryProof = ToCircuitMessagesDeliveryProof;

	fn verify_message(payload: &ToCircuitMessagePayload) -> Result<(), Self::Error> {
		messages::source::verify_chain_message::<WithCircuitMessageBridge>(payload)
	}

	fn verify_messages_delivery_proof(
		proof: Self::MessagesDeliveryProof,
	) -> Result<(LaneId, InboundLaneData<bp_gateway::AccountId>), Self::Error> {
		messages::source::verify_messages_delivery_proof::<WithCircuitMessageBridge, Runtime>(proof)
	}
}

impl SourceHeaderChain<bp_circuit::Balance> for Circuit {
	type Error = &'static str;
	// The proof is:
	// - hash of the header this proof has been created with;
	// - the storage proof of one or several keys;
	// - id of the lane we prove messages for;
	// - inclusive range of messages nonces that are proved.
	type MessagesProof = FromCircuitMessagesProof;

	fn verify_messages_proof(
		proof: Self::MessagesProof,
		messages_count: u32,
	) -> Result<ProvedMessages<Message<bp_circuit::Balance>>, Self::Error> {
		messages::target::verify_messages_proof::<WithCircuitMessageBridge, Runtime>(proof, messages_count)
	}
}

/// Gateway -> Circuit message lane pallet parameters.
#[derive(RuntimeDebug, Clone, Encode, Decode, PartialEq, Eq)]
pub enum GatewayToCircuitMessagesParameter {
	/// The conversion formula we use is: `GatewayTokens = CircuitTokens * conversion_rate`.
	CircuitToGatewayConversionRate(FixedU128),
}

impl MessagesParameter for GatewayToCircuitMessagesParameter {
	fn save(&self) {
		match *self {
			GatewayToCircuitMessagesParameter::CircuitToGatewayConversionRate(ref conversion_rate) => {
				CircuitToGatewayConversionRate::set(conversion_rate)
			}
		}
	}
}
