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

//! Gateway <> Circuit Bridge commands.

pub mod cli;
pub mod circuit_messages_to_gateway;
pub mod gateway_headers_to_circuit;
pub mod gateway_messages_to_circuit;

/// Circuit node client.
pub type CircuitClient = relay_substrate_client::Client<Circuit>;
/// Gateway node client.
pub type GatewayClient = relay_substrate_client::Client<Gateway>;

use crate::cli::{
	bridge::{CIRCUIT_TO_GATEWAY_INDEX, GATEWAY_TO_CIRCUIT_INDEX},
	encode_call::{self, Call, CliEncodeCall},
	CliChain, ExplicitOrMaximal, HexBytes, Origins,
};
use codec::{Decode, Encode};
use frame_support::weights::{GetDispatchInfo, Weight};
use pallet_bridge_dispatch::{CallOrigin, MessagePayload};
use relay_circuit_client::Circuit;
use relay_gateway_client::Gateway;
use relay_substrate_client::{Chain, TransactionSignScheme};
use relay_westend_client::Westend;
use sp_core::{Bytes, Pair};
use sp_runtime::{traits::IdentifyAccount, MultiSigner};
use sp_version::RuntimeVersion;
use std::fmt::Debug;

async fn run_send_message(command: cli::SendMessage) -> Result<(), String> {
	match command {
		cli::SendMessage::CircuitToGateway {
			source,
			source_sign,
			target_sign,
			lane,
			mut message,
			dispatch_weight,
			fee,
			origin,
			..
		} => {
			type Source = Circuit;
			type Target = Gateway;

			let account_ownership_digest = |target_call, source_account_id| {
				circuit_runtime::gateway_account_ownership_digest(
					&target_call,
					source_account_id,
					Target::RUNTIME_VERSION.spec_version,
				)
			};
			let estimate_message_fee_method = bp_gateway::TO_GATEWAY_ESTIMATE_MESSAGE_FEE_METHOD;
			let fee = fee.map(|x| x.cast());
			let send_message_call = |lane, payload, fee| {
				circuit_runtime::Call::BridgeGatewayMessages(circuit_runtime::MessagesCall::send_message(
					lane, payload, fee,
				))
			};

			let source_client = source.into_client::<Source>().await.map_err(format_err)?;
			let source_sign = source_sign.into_keypair::<Source>().map_err(format_err)?;
			let target_sign = target_sign.into_keypair::<Target>().map_err(format_err)?;

			encode_call::preprocess_call::<Source, Target>(&mut message, CIRCUIT_TO_GATEWAY_INDEX);
			let target_call = Target::encode_call(&message).map_err(|e| e.to_string())?;

			let payload = {
				let target_call_weight = prepare_call_dispatch_weight(
					dispatch_weight,
					ExplicitOrMaximal::Explicit(target_call.get_dispatch_info().weight),
					compute_maximal_message_dispatch_weight(Target::max_extrinsic_weight()),
				);
				let source_sender_public: MultiSigner = source_sign.public().into();
				let source_account_id = source_sender_public.into_account();

				message_payload(
					Target::RUNTIME_VERSION.spec_version,
					target_call_weight,
					match origin {
						Origins::Source => CallOrigin::SourceAccount(source_account_id),
						Origins::Target => {
							let digest = account_ownership_digest(&target_call, source_account_id.clone());
							let target_origin_public = target_sign.public();
							let digest_signature = target_sign.sign(&digest);
							CallOrigin::TargetAccount(
								source_account_id,
								target_origin_public.into(),
								digest_signature.into(),
							)
						}
					},
					&target_call,
				)
			};
			let dispatch_weight = payload.weight;

			let lane = lane.into();
			let fee = get_fee(fee, || {
				estimate_message_delivery_and_dispatch_fee(
					&source_client,
					estimate_message_fee_method,
					lane,
					payload.clone(),
				)
			})
			.await?;

			source_client
				.submit_signed_extrinsic(source_sign.public().into(), |transaction_nonce| {
					let send_message_call = send_message_call(lane, payload, fee);

					let signed_source_call = Source::sign_transaction(
						*source_client.genesis_hash(),
						&source_sign,
						transaction_nonce,
						send_message_call,
					)
					.encode();

					log::info!(
						target: "bridge",
						"Sending message to {}. Size: {}. Dispatch weight: {}. Fee: {}",
						Target::NAME,
						signed_source_call.len(),
						dispatch_weight,
						fee,
					);
					log::info!(
						target: "bridge",
						"Signed {} Call: {:?}",
						Source::NAME,
						HexBytes::encode(&signed_source_call)
					);

					Bytes(signed_source_call)
				})
				.await?;
		}
		cli::SendMessage::GatewayToCircuit {
			source,
			source_sign,
			target_sign,
			lane,
			mut message,
			dispatch_weight,
			fee,
			origin,
			..
		} => {
			type Source = Gateway;
			type Target = Circuit;

			let account_ownership_digest = |target_call, source_account_id| {
				gateway_runtime::circuit_account_ownership_digest(
					&target_call,
					source_account_id,
					Target::RUNTIME_VERSION.spec_version,
				)
			};
			let estimate_message_fee_method = bp_circuit::TO_CIRCUIT_ESTIMATE_MESSAGE_FEE_METHOD;
			let fee = fee.map(|x| x.0);
			let send_message_call = |lane, payload, fee| {
				gateway_runtime::Call::BridgeCircuitMessages(gateway_runtime::MessagesCall::send_message(
					lane, payload, fee,
				))
			};

			let source_client = source.into_client::<Source>().await.map_err(format_err)?;
			let source_sign = source_sign.into_keypair::<Source>().map_err(format_err)?;
			let target_sign = target_sign.into_keypair::<Target>().map_err(format_err)?;

			encode_call::preprocess_call::<Source, Target>(&mut message, GATEWAY_TO_CIRCUIT_INDEX);
			let target_call = Target::encode_call(&message).map_err(|e| e.to_string())?;

			let payload = {
				let target_call_weight = prepare_call_dispatch_weight(
					dispatch_weight,
					ExplicitOrMaximal::Explicit(target_call.get_dispatch_info().weight),
					compute_maximal_message_dispatch_weight(Target::max_extrinsic_weight()),
				);
				let source_sender_public: MultiSigner = source_sign.public().into();
				let source_account_id = source_sender_public.into_account();

				message_payload(
					Target::RUNTIME_VERSION.spec_version,
					target_call_weight,
					match origin {
						Origins::Source => CallOrigin::SourceAccount(source_account_id),
						Origins::Target => {
							let digest = account_ownership_digest(&target_call, source_account_id.clone());
							let target_origin_public = target_sign.public();
							let digest_signature = target_sign.sign(&digest);
							CallOrigin::TargetAccount(
								source_account_id,
								target_origin_public.into(),
								digest_signature.into(),
							)
						}
					},
					&target_call,
				)
			};
			let dispatch_weight = payload.weight;

			let lane = lane.into();
			let fee = get_fee(fee, || {
				estimate_message_delivery_and_dispatch_fee(
					&source_client,
					estimate_message_fee_method,
					lane,
					payload.clone(),
				)
			})
			.await?;

			source_client
				.submit_signed_extrinsic(source_sign.public().into(), |transaction_nonce| {
					let send_message_call = send_message_call(lane, payload, fee);

					let signed_source_call = Source::sign_transaction(
						*source_client.genesis_hash(),
						&source_sign,
						transaction_nonce,
						send_message_call,
					)
					.encode();

					log::info!(
						target: "bridge",
						"Sending message to {}. Size: {}. Dispatch weight: {}. Fee: {}",
						Target::NAME,
						signed_source_call.len(),
						dispatch_weight,
						fee,
					);
					log::info!(
						target: "bridge",
						"Signed {} Call: {:?}",
						Source::NAME,
						HexBytes::encode(&signed_source_call)
					);

					Bytes(signed_source_call)
				})
				.await?;
		}
	}
	Ok(())
}

async fn run_encode_message_payload(call: cli::EncodeMessagePayload) -> Result<(), String> {
	match call {
		cli::EncodeMessagePayload::GatewayToCircuit { payload } => {
			type Source = Gateway;

			let payload = Source::encode_message(payload)?;
			println!("{:?}", HexBytes::encode(&payload));
		}
		cli::EncodeMessagePayload::CircuitToGateway { payload } => {
			type Source = Circuit;

			let payload = Source::encode_message(payload)?;
			println!("{:?}", HexBytes::encode(&payload));
		}
	}
	Ok(())
}

async fn run_estimate_fee(cmd: cli::EstimateFee) -> Result<(), String> {
	match cmd {
		cli::EstimateFee::GatewayToCircuit { source, lane, payload } => {
			type Source = Gateway;
			type SourceBalance = bp_gateway::Balance;

			let estimate_message_fee_method = bp_circuit::TO_CIRCUIT_ESTIMATE_MESSAGE_FEE_METHOD;

			let source_client = source.into_client::<Source>().await.map_err(format_err)?;
			let lane = lane.into();
			let payload = Source::encode_message(payload)?;

			let fee: Option<SourceBalance> =
				estimate_message_delivery_and_dispatch_fee(&source_client, estimate_message_fee_method, lane, payload)
					.await?;

			println!("Fee: {:?}", fee);
		}
		cli::EstimateFee::CircuitToGateway { source, lane, payload } => {
			type Source = Circuit;
			type SourceBalance = bp_circuit::Balance;

			let estimate_message_fee_method = bp_gateway::TO_GATEWAY_ESTIMATE_MESSAGE_FEE_METHOD;

			let source_client = source.into_client::<Source>().await.map_err(format_err)?;
			let lane = lane.into();
			let payload = Source::encode_message(payload)?;

			let fee: Option<SourceBalance> =
				estimate_message_delivery_and_dispatch_fee(&source_client, estimate_message_fee_method, lane, payload)
					.await?;

			println!("Fee: {:?}", fee);
		}
	}

	Ok(())
}

async fn estimate_message_delivery_and_dispatch_fee<Fee: Decode, C: Chain, P: Encode>(
	client: &relay_substrate_client::Client<C>,
	estimate_fee_method: &str,
	lane: bp_messages::LaneId,
	payload: P,
) -> Result<Option<Fee>, relay_substrate_client::Error> {
	let encoded_response = client
		.state_call(estimate_fee_method.into(), (lane, payload).encode().into(), None)
		.await?;
	let decoded_response: Option<Fee> =
		Decode::decode(&mut &encoded_response.0[..]).map_err(relay_substrate_client::Error::ResponseParseFailed)?;
	Ok(decoded_response)
}

fn message_payload<SAccountId, TPublic, TSignature>(
	spec_version: u32,
	weight: Weight,
	origin: CallOrigin<SAccountId, TPublic, TSignature>,
	call: &impl Encode,
) -> MessagePayload<SAccountId, TPublic, TSignature, Vec<u8>>
where
	SAccountId: Encode + Debug,
	TPublic: Encode + Debug,
	TSignature: Encode + Debug,
{
	// Display nicely formatted call.
	let payload = MessagePayload {
		spec_version,
		weight,
		origin,
		call: HexBytes::encode(call),
	};

	log::info!(target: "bridge", "Created Message Payload: {:#?}", payload);
	log::info!(target: "bridge", "Encoded Message Payload: {:?}", HexBytes::encode(&payload));

	// re-pack to return `Vec<u8>`
	let MessagePayload {
		spec_version,
		weight,
		origin,
		call,
	} = payload;
	MessagePayload {
		spec_version,
		weight,
		origin,
		call: call.0,
	}
}

fn prepare_call_dispatch_weight(
	user_specified_dispatch_weight: Option<ExplicitOrMaximal<Weight>>,
	weight_from_pre_dispatch_call: ExplicitOrMaximal<Weight>,
	maximal_allowed_weight: Weight,
) -> Weight {
	match user_specified_dispatch_weight.unwrap_or(weight_from_pre_dispatch_call) {
		ExplicitOrMaximal::Explicit(weight) => weight,
		ExplicitOrMaximal::Maximal => maximal_allowed_weight,
	}
}

async fn get_fee<Fee, F, R, E>(fee: Option<Fee>, f: F) -> Result<Fee, String>
where
	Fee: Decode,
	F: FnOnce() -> R,
	R: std::future::Future<Output = Result<Option<Fee>, E>>,
	E: Debug,
{
	match fee {
		Some(fee) => Ok(fee),
		None => match f().await {
			Ok(Some(fee)) => Ok(fee),
			Ok(None) => Err("Failed to estimate message fee. Message is too heavy?".into()),
			Err(error) => Err(format!("Failed to estimate message fee: {:?}", error)),
		},
	}
}

fn compute_maximal_message_dispatch_weight(maximal_extrinsic_weight: Weight) -> Weight {
	bridge_runtime_common::messages::target::maximal_incoming_message_dispatch_weight(maximal_extrinsic_weight)
}

impl CliEncodeCall for Circuit {
	fn max_extrinsic_size() -> u32 {
		bp_circuit::max_extrinsic_size()
	}

	fn encode_call(call: &Call) -> anyhow::Result<Self::Call> {
		Ok(match call {
			Call::Raw { data } => Decode::decode(&mut &*data.0)?,
			Call::Remark { remark_payload, .. } => circuit_runtime::Call::System(circuit_runtime::SystemCall::remark(
				remark_payload.as_ref().map(|x| x.0.clone()).unwrap_or_default(),
			)),
			Call::Transfer { recipient, amount } => circuit_runtime::Call::Balances(
				circuit_runtime::BalancesCall::transfer(recipient.raw_id(), amount.cast()),
			),
			Call::BridgeSendMessage {
				lane,
				payload,
				fee,
				bridge_instance_index,
			} => match *bridge_instance_index {
				CIRCUIT_TO_GATEWAY_INDEX => {
					let payload = Decode::decode(&mut &*payload.0)?;
					circuit_runtime::Call::BridgeGatewayMessages(circuit_runtime::MessagesCall::send_message(
						lane.0,
						payload,
						fee.cast(),
					))
				}
				_ => anyhow::bail!(
					"Unsupported target bridge pallet with instance index: {}",
					bridge_instance_index
				),
			},
		})
	}
}

impl CliChain for Circuit {
	const RUNTIME_VERSION: RuntimeVersion = circuit_runtime::VERSION;

	type KeyPair = sp_core::sr25519::Pair;
	type MessagePayload = MessagePayload<bp_circuit::AccountId, bp_gateway::AccountSigner, bp_gateway::Signature, Vec<u8>>;

	fn ss58_format() -> u16 {
		circuit_runtime::SS58Prefix::get() as u16
	}

	fn max_extrinsic_weight() -> Weight {
		bp_circuit::max_extrinsic_weight()
	}

	// TODO [#854|#843] support multiple bridges?
	fn encode_message(message: cli::MessagePayload) -> Result<Self::MessagePayload, String> {
		match message {
			cli::MessagePayload::Raw { data } => MessagePayload::decode(&mut &*data.0)
				.map_err(|e| format!("Failed to decode Circuit's MessagePayload: {:?}", e)),
			cli::MessagePayload::Call { mut call, mut sender } => {
				type Source = Circuit;
				type Target = Gateway;

				sender.enforce_chain::<Source>();
				let spec_version = Target::RUNTIME_VERSION.spec_version;
				let origin = CallOrigin::SourceAccount(sender.raw_id());
				encode_call::preprocess_call::<Source, Target>(&mut call, CIRCUIT_TO_GATEWAY_INDEX);
				let call = Target::encode_call(&call).map_err(|e| e.to_string())?;
				let weight = call.get_dispatch_info().weight;

				Ok(message_payload(spec_version, weight, origin, &call))
			}
		}
	}
}

impl CliEncodeCall for Gateway {
	fn max_extrinsic_size() -> u32 {
		bp_gateway::max_extrinsic_size()
	}

	fn encode_call(call: &Call) -> anyhow::Result<Self::Call> {
		Ok(match call {
			Call::Raw { data } => Decode::decode(&mut &*data.0)?,
			Call::Remark { remark_payload, .. } => gateway_runtime::Call::System(gateway_runtime::SystemCall::remark(
				remark_payload.as_ref().map(|x| x.0.clone()).unwrap_or_default(),
			)),
			Call::Transfer { recipient, amount } => {
				gateway_runtime::Call::Balances(gateway_runtime::BalancesCall::transfer(recipient.raw_id(), amount.0))
			}
			Call::BridgeSendMessage {
				lane,
				payload,
				fee,
				bridge_instance_index,
			} => match *bridge_instance_index {
				GATEWAY_TO_CIRCUIT_INDEX => {
					let payload = Decode::decode(&mut &*payload.0)?;
					gateway_runtime::Call::BridgeCircuitMessages(gateway_runtime::MessagesCall::send_message(
						lane.0, payload, fee.0,
					))
				}
				_ => anyhow::bail!(
					"Unsupported target bridge pallet with instance index: {}",
					bridge_instance_index
				),
			},
		})
	}
}

impl CliChain for Gateway {
	const RUNTIME_VERSION: RuntimeVersion = gateway_runtime::VERSION;

	type KeyPair = sp_core::sr25519::Pair;
	type MessagePayload = MessagePayload<bp_gateway::AccountId, bp_circuit::AccountSigner, bp_circuit::Signature, Vec<u8>>;

	fn ss58_format() -> u16 {
		gateway_runtime::SS58Prefix::get() as u16
	}

	fn max_extrinsic_weight() -> Weight {
		bp_gateway::max_extrinsic_weight()
	}

	fn encode_message(message: cli::MessagePayload) -> Result<Self::MessagePayload, String> {
		match message {
			cli::MessagePayload::Raw { data } => MessagePayload::decode(&mut &*data.0)
				.map_err(|e| format!("Failed to decode Gateway's MessagePayload: {:?}", e)),
			cli::MessagePayload::Call { mut call, mut sender } => {
				type Source = Gateway;
				type Target = Circuit;

				sender.enforce_chain::<Source>();
				let spec_version = Target::RUNTIME_VERSION.spec_version;
				let origin = CallOrigin::SourceAccount(sender.raw_id());
				encode_call::preprocess_call::<Source, Target>(&mut call, GATEWAY_TO_CIRCUIT_INDEX);
				let call = Target::encode_call(&call).map_err(|e| e.to_string())?;
				let weight = call.get_dispatch_info().weight;

				Ok(message_payload(spec_version, weight, origin, &call))
			}
		}
	}
}

impl CliChain for Westend {
	const RUNTIME_VERSION: RuntimeVersion = bp_westend::VERSION;

	type KeyPair = sp_core::sr25519::Pair;
	type MessagePayload = ();

	fn ss58_format() -> u16 {
		42
	}

	fn max_extrinsic_weight() -> Weight {
		0
	}

	fn encode_message(_message: cli::MessagePayload) -> Result<Self::MessagePayload, String> {
		Err("Sending messages from Westend is not yet supported.".into())
	}
}

fn format_err(e: anyhow::Error) -> String {
	e.to_string()
}

#[cfg(test)]
mod tests {
	use super::*;
	use bp_messages::source_chain::TargetHeaderChain;
	use sp_core::Pair;
	use sp_runtime::traits::{IdentifyAccount, Verify};

	#[test]
	fn circuit_signature_is_valid_on_gateway() {
		let circuit_sign = relay_circuit_client::SigningParams::from_string("//Dave", None).unwrap();

		let call = gateway_runtime::Call::System(gateway_runtime::SystemCall::remark(vec![]));

		let circuit_public: bp_circuit::AccountSigner = circuit_sign.public().into();
		let circuit_account_id: bp_circuit::AccountId = circuit_public.into_account();

		let digest = circuit_runtime::gateway_account_ownership_digest(
			&call,
			circuit_account_id,
			gateway_runtime::VERSION.spec_version,
		);

		let gateway_signer = relay_gateway_client::SigningParams::from_string("//Dave", None).unwrap();
		let signature = gateway_signer.sign(&digest);

		assert!(signature.verify(&digest[..], &gateway_signer.public()));
	}

	#[test]
	fn gateway_signature_is_valid_on_circuit() {
		let gateway_sign = relay_gateway_client::SigningParams::from_string("//Dave", None).unwrap();

		let call = circuit_runtime::Call::System(circuit_runtime::SystemCall::remark(vec![]));

		let gateway_public: bp_gateway::AccountSigner = gateway_sign.public().into();
		let gateway_account_id: bp_gateway::AccountId = gateway_public.into_account();

		let digest = gateway_runtime::circuit_account_ownership_digest(
			&call,
			gateway_account_id,
			circuit_runtime::VERSION.spec_version,
		);

		let circuit_signer = relay_circuit_client::SigningParams::from_string("//Dave", None).unwrap();
		let signature = circuit_signer.sign(&digest);

		assert!(signature.verify(&digest[..], &circuit_signer.public()));
	}

	#[test]
	fn maximal_gateway_to_circuit_message_arguments_size_is_computed_correctly() {
		use gateway_runtime::circuit_messages::Circuit;

		let maximal_remark_size = encode_call::compute_maximal_message_arguments_size(
			bp_gateway::max_extrinsic_size(),
			bp_circuit::max_extrinsic_size(),
		);

		let call: circuit_runtime::Call = circuit_runtime::SystemCall::remark(vec![42; maximal_remark_size as _]).into();
		let payload = message_payload(
			Default::default(),
			call.get_dispatch_info().weight,
			pallet_bridge_dispatch::CallOrigin::SourceRoot,
			&call,
		);
		assert_eq!(Circuit::verify_message(&payload), Ok(()));

		let call: circuit_runtime::Call =
			circuit_runtime::SystemCall::remark(vec![42; (maximal_remark_size + 1) as _]).into();
		let payload = message_payload(
			Default::default(),
			call.get_dispatch_info().weight,
			pallet_bridge_dispatch::CallOrigin::SourceRoot,
			&call,
		);
		assert!(Circuit::verify_message(&payload).is_err());
	}

	#[test]
	fn maximal_size_remark_to_gateway_is_generated_correctly() {
		assert!(
			bridge_runtime_common::messages::target::maximal_incoming_message_size(bp_gateway::max_extrinsic_size())
				> bp_circuit::max_extrinsic_size(),
			"We can't actually send maximal messages to Gateway from Circuit, because Circuit extrinsics can't be that large",
		)
	}

	#[test]
	fn maximal_gateway_to_circuit_message_dispatch_weight_is_computed_correctly() {
		use gateway_runtime::circuit_messages::Circuit;

		let maximal_dispatch_weight = compute_maximal_message_dispatch_weight(bp_circuit::max_extrinsic_weight());
		let call: circuit_runtime::Call = gateway_runtime::SystemCall::remark(vec![]).into();

		let payload = message_payload(
			Default::default(),
			maximal_dispatch_weight,
			pallet_bridge_dispatch::CallOrigin::SourceRoot,
			&call,
		);
		assert_eq!(Circuit::verify_message(&payload), Ok(()));

		let payload = message_payload(
			Default::default(),
			maximal_dispatch_weight + 1,
			pallet_bridge_dispatch::CallOrigin::SourceRoot,
			&call,
		);
		assert!(Circuit::verify_message(&payload).is_err());
	}

	#[test]
	fn maximal_weight_fill_block_to_gateway_is_generated_correctly() {
		use circuit_runtime::gateway_messages::Gateway;

		let maximal_dispatch_weight = compute_maximal_message_dispatch_weight(bp_gateway::max_extrinsic_weight());
		let call: gateway_runtime::Call = circuit_runtime::SystemCall::remark(vec![]).into();

		let payload = message_payload(
			Default::default(),
			maximal_dispatch_weight,
			pallet_bridge_dispatch::CallOrigin::SourceRoot,
			&call,
		);
		assert_eq!(Gateway::verify_message(&payload), Ok(()));

		let payload = message_payload(
			Default::default(),
			maximal_dispatch_weight + 1,
			pallet_bridge_dispatch::CallOrigin::SourceRoot,
			&call,
		);
		assert!(Gateway::verify_message(&payload).is_err());
	}

	#[test]
	fn gateway_tx_extra_bytes_constant_is_correct() {
		let gateway_call = gateway_runtime::Call::System(gateway_runtime::SystemCall::remark(vec![]));
		let gateway_tx = Gateway::sign_transaction(
			Default::default(),
			&sp_keyring::AccountKeyring::Alice.pair(),
			0,
			gateway_call.clone(),
		);
		let extra_bytes_in_transaction = gateway_tx.encode().len() - gateway_call.encode().len();
		assert!(
			bp_gateway::TX_EXTRA_BYTES as usize >= extra_bytes_in_transaction,
			"Hardcoded number of extra bytes in Gateway transaction {} is lower than actual value: {}",
			bp_gateway::TX_EXTRA_BYTES,
			extra_bytes_in_transaction,
		);
	}

	#[test]
	fn circuit_tx_extra_bytes_constant_is_correct() {
		let circuit_call = circuit_runtime::Call::System(circuit_runtime::SystemCall::remark(vec![]));
		let circuit_tx = Circuit::sign_transaction(
			Default::default(),
			&sp_keyring::AccountKeyring::Alice.pair(),
			0,
			circuit_call.clone(),
		);
		let extra_bytes_in_transaction = circuit_tx.encode().len() - circuit_call.encode().len();
		assert!(
			bp_circuit::TX_EXTRA_BYTES as usize >= extra_bytes_in_transaction,
			"Hardcoded number of extra bytes in Circuit transaction {} is lower than actual value: {}",
			bp_circuit::TX_EXTRA_BYTES,
			extra_bytes_in_transaction,
		);
	}
}
