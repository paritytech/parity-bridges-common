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

//! Gateway <> Circuit Bridge commands.

pub mod cli;
pub mod circuit_headers_to_gateway;
pub mod circuit_messages_to_gateway;
pub mod gateway_headers_to_circuit;
pub mod gateway_messages_to_circuit;
pub mod westend_headers_to_circuit;

/// Circuit node client.
pub type CircuitClient = relay_substrate_client::Client<Circuit>;
/// Gateway node client.
pub type GatewayClient = relay_substrate_client::Client<Gateway>;
/// Westend node client.
pub type WestendClient = relay_substrate_client::Client<Westend>;

use crate::cli::{ExplicitOrMaximal, HexBytes, Origins};
use codec::{Decode, Encode};
use frame_support::weights::{GetDispatchInfo, Weight};
use pallet_bridge_dispatch::{CallOrigin, MessagePayload};
use relay_circuit_client::{Circuit, SigningParams as CircuitSigningParams};
use relay_gateway_client::{Gateway, SigningParams as GatewaySigningParams};
use relay_substrate_client::{Chain, ConnectionParams, TransactionSignScheme};
use relay_westend_client::Westend;
use sp_core::{Bytes, Pair};
use sp_runtime::traits::IdentifyAccount;
use std::fmt::Debug;

async fn run_init_bridge(command: cli::InitBridge) -> Result<(), String> {
	match command {
		cli::InitBridge::CircuitToGateway {
			circuit,
			gateway,
			gateway_sign,
		} => {
			let circuit_client = circuit.into_client().await?;
			let gateway_client = gateway.into_client().await?;
			let gateway_sign = gateway_sign.parse()?;

			crate::headers_initialize::initialize(
				circuit_client,
				gateway_client.clone(),
				gateway_sign.signer.public().into(),
				move |transaction_nonce, initialization_data| {
					Bytes(
						Gateway::sign_transaction(
							*gateway_client.genesis_hash(),
							&gateway_sign.signer,
							transaction_nonce,
							gateway_runtime::SudoCall::sudo(Box::new(
								gateway_runtime::BridgeGrandpaCircuitCall::initialize(initialization_data).into(),
							))
							.into(),
						)
						.encode(),
					)
				},
			)
			.await;
		}
		cli::InitBridge::GatewayToCircuit {
			gateway,
			circuit,
			circuit_sign,
		} => {
			let gateway_client = gateway.into_client().await?;
			let circuit_client = circuit.into_client().await?;
			let circuit_sign = circuit_sign.parse()?;

			crate::headers_initialize::initialize(
				gateway_client,
				circuit_client.clone(),
				circuit_sign.signer.public().into(),
				move |transaction_nonce, initialization_data| {
					let initialize_call = circuit_runtime::BridgeGrandpaGatewayCall::<
						circuit_runtime::Runtime,
						circuit_runtime::GatewayGrandpaInstance,
					>::initialize(initialization_data);

					Bytes(
						Circuit::sign_transaction(
							*circuit_client.genesis_hash(),
							&circuit_sign.signer,
							transaction_nonce,
							circuit_runtime::SudoCall::sudo(Box::new(initialize_call.into())).into(),
						)
						.encode(),
					)
				},
			)
			.await;
		}
		cli::InitBridge::WestendToCircuit {
			westend,
			circuit,
			circuit_sign,
		} => {
			let westend_client = westend.into_client().await?;
			let circuit_client = circuit.into_client().await?;
			let circuit_sign = circuit_sign.parse()?;

			// at Westend -> Circuit initialization we're not using sudo, because otherwise our deployments
			// may fail, because we need to initialize both Gateway -> Circuit and Westend -> Circuit bridge.
			// => since there's single possible sudo account, one of transaction may fail with duplicate nonce error
			crate::headers_initialize::initialize(
				westend_client,
				circuit_client.clone(),
				circuit_sign.signer.public().into(),
				move |transaction_nonce, initialization_data| {
					let initialize_call = circuit_runtime::BridgeGrandpaWestendCall::<
						circuit_runtime::Runtime,
						circuit_runtime::WestendGrandpaInstance,
					>::initialize(initialization_data);

					Bytes(
						Circuit::sign_transaction(
							*circuit_client.genesis_hash(),
							&circuit_sign.signer,
							transaction_nonce,
							initialize_call.into(),
						)
						.encode(),
					)
				},
			)
			.await;
		}
	}
	Ok(())
}

async fn run_relay_headers(command: cli::RelayHeaders) -> Result<(), String> {
	match command {
		cli::RelayHeaders::CircuitToGateway {
			circuit,
			gateway,
			gateway_sign,
			prometheus_params,
		} => {
			let circuit_client = circuit.into_client().await?;
			let gateway_client = gateway.into_client().await?;
			let gateway_sign = gateway_sign.parse()?;
			circuit_headers_to_gateway::run(circuit_client, gateway_client, gateway_sign, prometheus_params.into()).await
		}
		cli::RelayHeaders::GatewayToCircuit {
			gateway,
			circuit,
			circuit_sign,
			prometheus_params,
		} => {
			let gateway_client = gateway.into_client().await?;
			let circuit_client = circuit.into_client().await?;
			let circuit_sign = circuit_sign.parse()?;
			gateway_headers_to_circuit::run(gateway_client, circuit_client, circuit_sign, prometheus_params.into()).await
		}
		cli::RelayHeaders::WestendToCircuit {
			westend,
			circuit,
			circuit_sign,
			prometheus_params,
		} => {
			let westend_client = westend.into_client().await?;
			let circuit_client = circuit.into_client().await?;
			let circuit_sign = circuit_sign.parse()?;
			westend_headers_to_circuit::run(westend_client, circuit_client, circuit_sign, prometheus_params.into()).await
		}
	}
}

async fn run_relay_messages(command: cli::RelayMessages) -> Result<(), String> {
	match command {
		cli::RelayMessages::CircuitToGateway {
			circuit,
			circuit_sign,
			gateway,
			gateway_sign,
			prometheus_params,
			lane,
		} => {
			let circuit_client = circuit.into_client().await?;
			let circuit_sign = circuit_sign.parse()?;
			let gateway_client = gateway.into_client().await?;
			let gateway_sign = gateway_sign.parse()?;

			circuit_messages_to_gateway::run(
				circuit_client,
				circuit_sign,
				gateway_client,
				gateway_sign,
				lane.into(),
				prometheus_params.into(),
			)
			.await
		}
		cli::RelayMessages::GatewayToCircuit {
			gateway,
			gateway_sign,
			circuit,
			circuit_sign,
			prometheus_params,
			lane,
		} => {
			let gateway_client = gateway.into_client().await?;
			let gateway_sign = gateway_sign.parse()?;
			let circuit_client = circuit.into_client().await?;
			let circuit_sign = circuit_sign.parse()?;

			gateway_messages_to_circuit::run(
				gateway_client,
				gateway_sign,
				circuit_client,
				circuit_sign,
				lane.into(),
				prometheus_params.into(),
			)
			.await
		}
	}
}

async fn run_send_message(command: cli::SendMessage) -> Result<(), String> {
	match command {
		cli::SendMessage::CircuitToGateway {
			circuit,
			circuit_sign,
			gateway_sign,
			lane,
			message,
			dispatch_weight,
			fee,
			origin,
			..
		} => {
			let circuit_client = circuit.into_client().await?;
			let circuit_sign = circuit_sign.parse()?;
			let gateway_sign = gateway_sign.parse()?;
			let gateway_call = message.into_call()?;

			let payload =
				circuit_to_gateway_message_payload(&circuit_sign, &gateway_sign, &gateway_call, origin, dispatch_weight);
			let dispatch_weight = payload.weight;

			let lane = lane.into();
			let fee = get_fee(fee, || {
				estimate_message_delivery_and_dispatch_fee(
					&circuit_client,
					bp_gateway::TO_GATEWAY_ESTIMATE_MESSAGE_FEE_METHOD,
					lane,
					payload.clone(),
				)
			})
			.await?;

			circuit_client
				.submit_signed_extrinsic(circuit_sign.signer.public().clone().into(), |transaction_nonce| {
					let circuit_call = circuit_runtime::Call::BridgeGatewayMessages(
						circuit_runtime::MessagesCall::send_message(lane, payload, fee),
					);

					let signed_circuit_call = Circuit::sign_transaction(
						*circuit_client.genesis_hash(),
						&circuit_sign.signer,
						transaction_nonce,
						circuit_call,
					)
					.encode();

					log::info!(
						target: "bridge",
						"Sending message to Gateway. Size: {}. Dispatch weight: {}. Fee: {}",
						signed_circuit_call.len(),
						dispatch_weight,
						fee,
					);
					log::info!(target: "bridge", "Signed Circuit Call: {:?}", HexBytes::encode(&signed_circuit_call));

					Bytes(signed_circuit_call)
				})
				.await?;
		}
		cli::SendMessage::GatewayToCircuit {
			gateway,
			gateway_sign,
			circuit_sign,
			lane,
			message,
			dispatch_weight,
			fee,
			origin,
			..
		} => {
			let gateway_client = gateway.into_client().await?;
			let gateway_sign = gateway_sign.parse()?;
			let circuit_sign = circuit_sign.parse()?;
			let circuit_call = message.into_call()?;

			let payload =
				gateway_to_circuit_message_payload(&gateway_sign, &circuit_sign, &circuit_call, origin, dispatch_weight);
			let dispatch_weight = payload.weight;

			let lane = lane.into();
			let fee = get_fee(fee, || {
				estimate_message_delivery_and_dispatch_fee(
					&gateway_client,
					bp_circuit::TO_CIRCUIT_ESTIMATE_MESSAGE_FEE_METHOD,
					lane,
					payload.clone(),
				)
			})
			.await?;

			gateway_client
				.submit_signed_extrinsic(gateway_sign.signer.public().clone().into(), |transaction_nonce| {
					let gateway_call = gateway_runtime::Call::BridgeCircuitMessages(
						gateway_runtime::MessagesCall::send_message(lane, payload, fee),
					);

					let signed_gateway_call = Gateway::sign_transaction(
						*gateway_client.genesis_hash(),
						&gateway_sign.signer,
						transaction_nonce,
						gateway_call,
					)
					.encode();

					log::info!(
						target: "bridge",
						"Sending message to Circuit. Size: {}. Dispatch weight: {}. Fee: {}",
						signed_gateway_call.len(),
						dispatch_weight,
						fee,
					);
					log::info!(target: "bridge", "Signed Gateway Call: {:?}", HexBytes::encode(&signed_gateway_call));

					Bytes(signed_gateway_call)
				})
				.await?;
		}
	}
	Ok(())
}

async fn run_encode_call(call: cli::EncodeCall) -> Result<(), String> {
	match call {
		cli::EncodeCall::Gateway { call } => {
			let call = call.into_call()?;

			println!("{:?}", HexBytes::encode(&call));
		}
		cli::EncodeCall::Circuit { call } => {
			let call = call.into_call()?;
			println!("{:?}", HexBytes::encode(&call));
		}
	}
	Ok(())
}

async fn run_encode_message_payload(call: cli::EncodeMessagePayload) -> Result<(), String> {
	match call {
		cli::EncodeMessagePayload::GatewayToCircuit { payload } => {
			let payload = payload.into_payload()?;

			println!("{:?}", HexBytes::encode(&payload));
		}
		cli::EncodeMessagePayload::CircuitToGateway { payload } => {
			let payload = payload.into_payload()?;

			println!("{:?}", HexBytes::encode(&payload));
		}
	}
	Ok(())
}

async fn run_estimate_fee(cmd: cli::EstimateFee) -> Result<(), String> {
	match cmd {
		cli::EstimateFee::GatewayToCircuit { gateway, lane, payload } => {
			let client = gateway.into_client().await?;
			let lane = lane.into();
			let payload = payload.into_payload()?;

			let fee: Option<bp_gateway::Balance> = estimate_message_delivery_and_dispatch_fee(
				&client,
				bp_circuit::TO_CIRCUIT_ESTIMATE_MESSAGE_FEE_METHOD,
				lane,
				payload,
			)
			.await?;

			println!("Fee: {:?}", fee);
		}
		cli::EstimateFee::CircuitToGateway { circuit, lane, payload } => {
			let client = circuit.into_client().await?;
			let lane = lane.into();
			let payload = payload.into_payload()?;

			let fee: Option<bp_circuit::Balance> = estimate_message_delivery_and_dispatch_fee(
				&client,
				bp_gateway::TO_GATEWAY_ESTIMATE_MESSAGE_FEE_METHOD,
				lane,
				payload,
			)
			.await?;

			println!("Fee: {:?}", fee);
		}
	}

	Ok(())
}

async fn run_derive_account(cmd: cli::DeriveAccount) -> Result<(), String> {
	match cmd {
		cli::DeriveAccount::GatewayToCircuit { account } => {
			let account = account.into_gateway();
			let acc = bp_runtime::SourceAccount::Account(account.clone());
			let id = bp_circuit::derive_account_from_gateway_id(acc);
			println!(
				"{} (Gateway)\n\nCorresponding (derived) account id:\n-> {} (Circuit)",
				account, id
			)
		}
		cli::DeriveAccount::CircuitToGateway { account } => {
			let account = account.into_circuit();
			let acc = bp_runtime::SourceAccount::Account(account.clone());
			let id = bp_gateway::derive_account_from_circuit_id(acc);
			println!(
				"{} (Circuit)\n\nCorresponding (derived) account id:\n-> {} (Gateway)",
				account, id
			)
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

fn remark_payload(remark_size: Option<ExplicitOrMaximal<usize>>, maximal_allowed_size: u32) -> Vec<u8> {
	match remark_size {
		Some(ExplicitOrMaximal::Explicit(remark_size)) => vec![0; remark_size],
		Some(ExplicitOrMaximal::Maximal) => vec![0; maximal_allowed_size as _],
		None => format!(
			"Unix time: {}",
			std::time::SystemTime::now()
				.duration_since(std::time::SystemTime::UNIX_EPOCH)
				.unwrap_or_default()
				.as_secs(),
		)
		.as_bytes()
		.to_vec(),
	}
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

fn gateway_to_circuit_message_payload(
	gateway_sign: &GatewaySigningParams,
	circuit_sign: &CircuitSigningParams,
	circuit_call: &circuit_runtime::Call,
	origin: Origins,
	user_specified_dispatch_weight: Option<ExplicitOrMaximal<Weight>>,
) -> gateway_runtime::circuit_messages::ToCircuitMessagePayload {
	let circuit_call_weight = prepare_call_dispatch_weight(
		user_specified_dispatch_weight,
		ExplicitOrMaximal::Explicit(circuit_call.get_dispatch_info().weight),
		compute_maximal_message_dispatch_weight(bp_circuit::max_extrinsic_weight()),
	);
	let gateway_sender_public: bp_gateway::AccountSigner = gateway_sign.signer.public().clone().into();
	let gateway_account_id: bp_gateway::AccountId = gateway_sender_public.into_account();
	let circuit_origin_public = circuit_sign.signer.public();

	message_payload(
		circuit_runtime::VERSION.spec_version,
		circuit_call_weight,
		match origin {
			Origins::Source => CallOrigin::SourceAccount(gateway_account_id),
			Origins::Target => {
				let digest = gateway_runtime::circuit_account_ownership_digest(
					&circuit_call,
					gateway_account_id.clone(),
					circuit_runtime::VERSION.spec_version,
				);

				let digest_signature = circuit_sign.signer.sign(&digest);

				CallOrigin::TargetAccount(gateway_account_id, circuit_origin_public.into(), digest_signature.into())
			}
		},
		&circuit_call,
	)
}

fn circuit_to_gateway_message_payload(
	circuit_sign: &CircuitSigningParams,
	gateway_sign: &GatewaySigningParams,
	gateway_call: &gateway_runtime::Call,
	origin: Origins,
	user_specified_dispatch_weight: Option<ExplicitOrMaximal<Weight>>,
) -> circuit_runtime::gateway_messages::ToGatewayMessagePayload {
	let gateway_call_weight = prepare_call_dispatch_weight(
		user_specified_dispatch_weight,
		ExplicitOrMaximal::Explicit(gateway_call.get_dispatch_info().weight),
		compute_maximal_message_dispatch_weight(bp_gateway::max_extrinsic_weight()),
	);
	let circuit_sender_public: bp_circuit::AccountSigner = circuit_sign.signer.public().clone().into();
	let circuit_account_id: bp_circuit::AccountId = circuit_sender_public.into_account();
	let gateway_origin_public = gateway_sign.signer.public();

	message_payload(
		gateway_runtime::VERSION.spec_version,
		gateway_call_weight,
		match origin {
			Origins::Source => CallOrigin::SourceAccount(circuit_account_id),
			Origins::Target => {
				let digest = circuit_runtime::gateway_account_ownership_digest(
					&gateway_call,
					circuit_account_id.clone(),
					gateway_runtime::VERSION.spec_version,
				);

				let digest_signature = gateway_sign.signer.sign(&digest);

				CallOrigin::TargetAccount(circuit_account_id, gateway_origin_public.into(), digest_signature.into())
			}
		},
		&gateway_call,
	)
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

fn compute_maximal_message_arguments_size(
	maximal_source_extrinsic_size: u32,
	maximal_target_extrinsic_size: u32,
) -> u32 {
	// assume that both signed extensions and other arguments fit 1KB
	let service_tx_bytes_on_source_chain = 1024;
	let maximal_source_extrinsic_size = maximal_source_extrinsic_size - service_tx_bytes_on_source_chain;
	let maximal_call_size =
		bridge_runtime_common::messages::target::maximal_incoming_message_size(maximal_target_extrinsic_size);
	let maximal_call_size = if maximal_call_size > maximal_source_extrinsic_size {
		maximal_source_extrinsic_size
	} else {
		maximal_call_size
	};

	// bytes in Call encoding that are used to encode everything except arguments
	let service_bytes = 1 + 1 + 4;
	maximal_call_size - service_bytes
}

impl cli::CircuitToGatewayMessagePayload {
	/// Parse the CLI parameters and construct message payload.
	pub fn into_payload(
		self,
	) -> Result<MessagePayload<bp_gateway::AccountId, bp_gateway::AccountSigner, bp_gateway::Signature, Vec<u8>>, String> {
		match self {
			Self::Raw { data } => MessagePayload::decode(&mut &*data.0)
				.map_err(|e| format!("Failed to decode Circuit's MessagePayload: {:?}", e)),
			Self::Message { message, sender } => {
				let spec_version = gateway_runtime::VERSION.spec_version;
				let origin = CallOrigin::SourceAccount(sender.into_circuit());
				let call = message.into_call()?;
				let weight = call.get_dispatch_info().weight;

				Ok(message_payload(spec_version, weight, origin, &call))
			}
		}
	}
}

impl cli::GatewayToCircuitMessagePayload {
	/// Parse the CLI parameters and construct message payload.
	pub fn into_payload(
		self,
	) -> Result<MessagePayload<bp_circuit::AccountId, bp_circuit::AccountSigner, bp_circuit::Signature, Vec<u8>>, String> {
		match self {
			Self::Raw { data } => MessagePayload::decode(&mut &*data.0)
				.map_err(|e| format!("Failed to decode Gateway's MessagePayload: {:?}", e)),
			Self::Message { message, sender } => {
				let spec_version = circuit_runtime::VERSION.spec_version;
				let origin = CallOrigin::SourceAccount(sender.into_gateway());
				let call = message.into_call()?;
				let weight = call.get_dispatch_info().weight;

				Ok(message_payload(spec_version, weight, origin, &call))
			}
		}
	}
}

impl cli::GatewaySigningParams {
	/// Parse CLI parameters into typed signing params.
	pub fn parse(self) -> Result<GatewaySigningParams, String> {
		GatewaySigningParams::from_suri(&self.gateway_signer, self.gateway_signer_password.as_deref())
			.map_err(|e| format!("Failed to parse gateway-signer: {:?}", e))
	}
}

impl cli::CircuitSigningParams {
	/// Parse CLI parameters into typed signing params.
	pub fn parse(self) -> Result<CircuitSigningParams, String> {
		CircuitSigningParams::from_suri(&self.circuit_signer, self.circuit_signer_password.as_deref())
			.map_err(|e| format!("Failed to parse circuit-signer: {:?}", e))
	}
}

impl cli::CircuitConnectionParams {
	/// Convert CLI connection parameters into Circuit RPC Client.
	pub async fn into_client(self) -> relay_substrate_client::Result<CircuitClient> {
		CircuitClient::new(ConnectionParams {
			host: self.circuit_host,
			port: self.circuit_port,
			secure: self.circuit_secure,
		})
		.await
	}
}

impl cli::GatewayConnectionParams {
	/// Convert CLI connection parameters into Gateway RPC Client.
	pub async fn into_client(self) -> relay_substrate_client::Result<GatewayClient> {
		GatewayClient::new(ConnectionParams {
			host: self.gateway_host,
			port: self.gateway_port,
			secure: self.gateway_secure,
		})
		.await
	}
}

impl cli::WestendConnectionParams {
	/// Convert CLI connection parameters into Westend RPC Client.
	pub async fn into_client(self) -> relay_substrate_client::Result<WestendClient> {
		WestendClient::new(ConnectionParams {
			host: self.westend_host,
			port: self.westend_port,
			secure: self.westend_secure,
		})
		.await
	}
}

impl cli::ToGatewayMessage {
	/// Convert CLI call request into runtime `Call` instance.
	pub fn into_call(self) -> Result<gateway_runtime::Call, String> {
		let call = match self {
			cli::ToGatewayMessage::Raw { data } => {
				Decode::decode(&mut &*data.0).map_err(|e| format!("Unable to decode message: {:#?}", e))?
			}
			cli::ToGatewayMessage::Remark { remark_size } => {
				gateway_runtime::Call::System(gateway_runtime::SystemCall::remark(remark_payload(
					remark_size,
					compute_maximal_message_arguments_size(
						bp_circuit::max_extrinsic_size(),
						bp_gateway::max_extrinsic_size(),
					),
				)))
			}
			cli::ToGatewayMessage::Transfer { recipient, amount } => {
				let recipient = recipient.into_gateway();
				gateway_runtime::Call::Balances(gateway_runtime::BalancesCall::transfer(recipient, amount))
			}
			cli::ToGatewayMessage::CircuitSendMessage { lane, payload, fee } => {
				let payload = cli::GatewayToCircuitMessagePayload::Raw { data: payload }.into_payload()?;
				let lane = lane.into();
				gateway_runtime::Call::BridgeCircuitMessages(gateway_runtime::MessagesCall::send_message(
					lane, payload, fee,
				))
			}
		};

		log::info!(target: "bridge", "Generated Gateway call: {:#?}", call);
		log::info!(target: "bridge", "Weight of Gateway call: {}", call.get_dispatch_info().weight);
		log::info!(target: "bridge", "Encoded Gateway call: {:?}", HexBytes::encode(&call));

		Ok(call)
	}
}

impl cli::ToCircuitMessage {
	/// Convert CLI call request into runtime `Call` instance.
	pub fn into_call(self) -> Result<circuit_runtime::Call, String> {
		let call = match self {
			cli::ToCircuitMessage::Raw { data } => {
				Decode::decode(&mut &*data.0).map_err(|e| format!("Unable to decode message: {:#?}", e))?
			}
			cli::ToCircuitMessage::Remark { remark_size } => {
				circuit_runtime::Call::System(circuit_runtime::SystemCall::remark(remark_payload(
					remark_size,
					compute_maximal_message_arguments_size(
						bp_gateway::max_extrinsic_size(),
						bp_circuit::max_extrinsic_size(),
					),
				)))
			}
			cli::ToCircuitMessage::Transfer { recipient, amount } => {
				let recipient = recipient.into_circuit();
				circuit_runtime::Call::Balances(circuit_runtime::BalancesCall::transfer(recipient, amount))
			}
			cli::ToCircuitMessage::GatewaySendMessage { lane, payload, fee } => {
				let payload = cli::CircuitToGatewayMessagePayload::Raw { data: payload }.into_payload()?;
				let lane = lane.into();
				circuit_runtime::Call::BridgeGatewayMessages(circuit_runtime::MessagesCall::send_message(
					lane, payload, fee,
				))
			}
		};

		log::info!(target: "bridge", "Generated Circuit call: {:#?}", call);
		log::info!(target: "bridge", "Weight of Circuit call: {}", call.get_dispatch_info().weight);
		log::info!(target: "bridge", "Encoded Circuit call: {:?}", HexBytes::encode(&call));

		Ok(call)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bp_messages::source_chain::TargetHeaderChain;
	use sp_core::Pair;
	use sp_runtime::traits::{IdentifyAccount, Verify};

	#[test]
	fn circuit_signature_is_valid_on_gateway() {
		let circuit_sign = relay_circuit_client::SigningParams::from_suri("//Dave", None).unwrap();

		let call = gateway_runtime::Call::System(gateway_runtime::SystemCall::remark(vec![]));

		let circuit_public: bp_circuit::AccountSigner = circuit_sign.signer.public().clone().into();
		let circuit_account_id: bp_circuit::AccountId = circuit_public.into_account();

		let digest = circuit_runtime::gateway_account_ownership_digest(
			&call,
			circuit_account_id,
			gateway_runtime::VERSION.spec_version,
		);

		let gateway_signer = relay_gateway_client::SigningParams::from_suri("//Dave", None).unwrap();
		let signature = gateway_signer.signer.sign(&digest);

		assert!(signature.verify(&digest[..], &gateway_signer.signer.public()));
	}

	#[test]
	fn gateway_signature_is_valid_on_circuit() {
		let gateway_sign = relay_gateway_client::SigningParams::from_suri("//Dave", None).unwrap();

		let call = circuit_runtime::Call::System(circuit_runtime::SystemCall::remark(vec![]));

		let gateway_public: bp_gateway::AccountSigner = gateway_sign.signer.public().clone().into();
		let gateway_account_id: bp_gateway::AccountId = gateway_public.into_account();

		let digest = gateway_runtime::circuit_account_ownership_digest(
			&call,
			gateway_account_id,
			circuit_runtime::VERSION.spec_version,
		);

		let circuit_signer = relay_circuit_client::SigningParams::from_suri("//Dave", None).unwrap();
		let signature = circuit_signer.signer.sign(&digest);

		assert!(signature.verify(&digest[..], &circuit_signer.signer.public()));
	}

	#[test]
	fn maximal_gateway_to_circuit_message_arguments_size_is_computed_correctly() {
		use gateway_runtime::circuit_messages::Circuit;

		let maximal_remark_size =
			compute_maximal_message_arguments_size(bp_gateway::max_extrinsic_size(), bp_circuit::max_extrinsic_size());

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
