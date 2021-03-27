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

//! Deal with CLI args of Gateway <> Circuit relay.

use frame_support::weights::Weight;
use structopt::StructOpt;

use crate::cli::{AccountId, ExplicitOrMaximal, HexBytes, HexLaneId, Origins, PrometheusParams};
use crate::declare_chain_options;

/// Start headers relayer process.
#[derive(StructOpt)]
pub enum RelayHeaders {
	/// Relay Circuit headers to Gateway.
	CircuitToGateway {
		#[structopt(flatten)]
		circuit: CircuitConnectionParams,
		#[structopt(flatten)]
		gateway: GatewayConnectionParams,
		#[structopt(flatten)]
		gateway_sign: GatewaySigningParams,
		#[structopt(flatten)]
		prometheus_params: PrometheusParams,
	},
	/// Relay Gateway headers to Circuit.
	GatewayToCircuit {
		#[structopt(flatten)]
		gateway: GatewayConnectionParams,
		#[structopt(flatten)]
		circuit: CircuitConnectionParams,
		#[structopt(flatten)]
		circuit_sign: CircuitSigningParams,
		#[structopt(flatten)]
		prometheus_params: PrometheusParams,
	},
	/// Relay Westend headers to Circuit.
	WestendToCircuit {
		#[structopt(flatten)]
		westend: WestendConnectionParams,
		#[structopt(flatten)]
		circuit: CircuitConnectionParams,
		#[structopt(flatten)]
		circuit_sign: CircuitSigningParams,
		#[structopt(flatten)]
		prometheus_params: PrometheusParams,
	},
}

impl RelayHeaders {
	/// Run the command.
	pub async fn run(self) -> anyhow::Result<()> {
		super::run_relay_headers(self).await.map_err(format_err)?;
		Ok(())
	}
}

/// Start message relayer process.
#[derive(StructOpt)]
pub enum RelayMessages {
	/// Serve given lane of Circuit -> Gateway messages.
	CircuitToGateway {
		#[structopt(flatten)]
		circuit: CircuitConnectionParams,
		#[structopt(flatten)]
		circuit_sign: CircuitSigningParams,
		#[structopt(flatten)]
		gateway: GatewayConnectionParams,
		#[structopt(flatten)]
		gateway_sign: GatewaySigningParams,
		#[structopt(flatten)]
		prometheus_params: PrometheusParams,
		/// Hex-encoded lane id that should be served by the relay. Defaults to `00000000`.
		#[structopt(long, default_value = "00000000")]
		lane: HexLaneId,
	},
	/// Serve given lane of Gateway -> Circuit messages.
	GatewayToCircuit {
		#[structopt(flatten)]
		gateway: GatewayConnectionParams,
		#[structopt(flatten)]
		gateway_sign: GatewaySigningParams,
		#[structopt(flatten)]
		circuit: CircuitConnectionParams,
		#[structopt(flatten)]
		circuit_sign: CircuitSigningParams,
		#[structopt(flatten)]
		prometheus_params: PrometheusParams,
		/// Hex-encoded lane id that should be served by the relay. Defaults to `00000000`.
		#[structopt(long, default_value = "00000000")]
		lane: HexLaneId,
	},
}

impl RelayMessages {
	/// Run the command.
	pub async fn run(self) -> anyhow::Result<()> {
		super::run_relay_messages(self).await.map_err(format_err)?;
		Ok(())
	}
}

/// Initialize bridge pallet.
#[derive(StructOpt)]
pub enum InitBridge {
	/// Initialize Circuit headers bridge in Gateway.
	CircuitToGateway {
		#[structopt(flatten)]
		circuit: CircuitConnectionParams,
		#[structopt(flatten)]
		gateway: GatewayConnectionParams,
		#[structopt(flatten)]
		gateway_sign: GatewaySigningParams,
	},
	/// Initialize Gateway headers bridge in Circuit.
	GatewayToCircuit {
		#[structopt(flatten)]
		gateway: GatewayConnectionParams,
		#[structopt(flatten)]
		circuit: CircuitConnectionParams,
		#[structopt(flatten)]
		circuit_sign: CircuitSigningParams,
	},
	/// Initialize Westend headers bridge in Circuit.
	WestendToCircuit {
		#[structopt(flatten)]
		westend: WestendConnectionParams,
		#[structopt(flatten)]
		circuit: CircuitConnectionParams,
		#[structopt(flatten)]
		circuit_sign: CircuitSigningParams,
	},
}

impl InitBridge {
	/// Run the command.
	pub async fn run(self) -> anyhow::Result<()> {
		super::run_init_bridge(self).await.map_err(format_err)?;
		Ok(())
	}
}

/// Send bridge message.
#[derive(StructOpt)]
pub enum SendMessage {
	/// Submit message to given Circuit -> Gateway lane.
	CircuitToGateway {
		#[structopt(flatten)]
		circuit: CircuitConnectionParams,
		#[structopt(flatten)]
		circuit_sign: CircuitSigningParams,
		#[structopt(flatten)]
		gateway_sign: GatewaySigningParams,
		/// Hex-encoded lane id. Defaults to `00000000`.
		#[structopt(long, default_value = "00000000")]
		lane: HexLaneId,
		/// Dispatch weight of the message. If not passed, determined automatically.
		#[structopt(long)]
		dispatch_weight: Option<ExplicitOrMaximal<Weight>>,
		/// Delivery and dispatch fee in source chain base currency units. If not passed, determined automatically.
		#[structopt(long)]
		fee: Option<bp_circuit::Balance>,
		/// Message type.
		#[structopt(subcommand)]
		message: ToGatewayMessage,
		/// The origin to use when dispatching the message on the target chain. Defaults to
		/// `SourceAccount`.
		#[structopt(long, possible_values = &Origins::variants(), default_value = "Source")]
		origin: Origins,
	},
	/// Submit message to given Gateway -> Circuit lane.
	GatewayToCircuit {
		#[structopt(flatten)]
		gateway: GatewayConnectionParams,
		#[structopt(flatten)]
		gateway_sign: GatewaySigningParams,
		#[structopt(flatten)]
		circuit_sign: CircuitSigningParams,
		/// Hex-encoded lane id. Defaults to `00000000`.
		#[structopt(long, default_value = "00000000")]
		lane: HexLaneId,
		/// Dispatch weight of the message. If not passed, determined automatically.
		#[structopt(long)]
		dispatch_weight: Option<ExplicitOrMaximal<Weight>>,
		/// Delivery and dispatch fee in source chain base currency units. If not passed, determined automatically.
		#[structopt(long)]
		fee: Option<bp_gateway::Balance>,
		/// Message type.
		#[structopt(subcommand)]
		message: ToCircuitMessage,
		/// The origin to use when dispatching the message on the target chain. Defaults to
		/// `SourceAccount`.
		#[structopt(long, possible_values = &Origins::variants(), default_value = "Source")]
		origin: Origins,
	},
}

impl SendMessage {
	/// Run the command.
	pub async fn run(self) -> anyhow::Result<()> {
		super::run_send_message(self).await.map_err(format_err)?;
		Ok(())
	}
}

/// A call to encode.
#[derive(StructOpt)]
pub enum EncodeCall {
	/// Encode Gateway's Call.
	Gateway {
		#[structopt(flatten)]
		call: ToGatewayMessage,
	},
	/// Encode Circuit's Call.
	Circuit {
		#[structopt(flatten)]
		call: ToCircuitMessage,
	},
}

impl EncodeCall {
	/// Run the command.
	pub async fn run(self) -> anyhow::Result<()> {
		super::run_encode_call(self).await.map_err(format_err)?;
		Ok(())
	}
}

/// A `MessagePayload` to encode.
#[derive(StructOpt)]
pub enum EncodeMessagePayload {
	/// Message Payload of Gateway to Circuit call.
	GatewayToCircuit {
		#[structopt(flatten)]
		payload: GatewayToCircuitMessagePayload,
	},
	/// Message Payload of Circuit to Gateway call.
	CircuitToGateway {
		#[structopt(flatten)]
		payload: CircuitToGatewayMessagePayload,
	},
}

impl EncodeMessagePayload {
	/// Run the command.
	pub async fn run(self) -> anyhow::Result<()> {
		super::run_encode_message_payload(self).await.map_err(format_err)?;
		Ok(())
	}
}

/// Estimate Delivery & Dispatch Fee command.
#[derive(StructOpt)]
pub enum EstimateFee {
	/// Estimate fee of Gateway to Circuit message.
	GatewayToCircuit {
		#[structopt(flatten)]
		gateway: GatewayConnectionParams,
		/// Hex-encoded id of lane that will be delivering the message.
		#[structopt(long)]
		lane: HexLaneId,
		/// Payload to send over the bridge.
		#[structopt(flatten)]
		payload: GatewayToCircuitMessagePayload,
	},
	/// Estimate fee of Gateway to Circuit message.
	CircuitToGateway {
		#[structopt(flatten)]
		circuit: CircuitConnectionParams,
		/// Hex-encoded id of lane that will be delivering the message.
		#[structopt(long)]
		lane: HexLaneId,
		/// Payload to send over the bridge.
		#[structopt(flatten)]
		payload: CircuitToGatewayMessagePayload,
	},
}

impl EstimateFee {
	/// Run the command.
	pub async fn run(self) -> anyhow::Result<()> {
		super::run_estimate_fee(self).await.map_err(format_err)?;
		Ok(())
	}
}

/// Given a source chain `AccountId`, derive the corresponding `AccountId` for the target chain.
///
/// The (derived) target chain `AccountId` is going to be used as dispatch origin of the call
/// that has been sent over the bridge.
/// This account can also be used to receive target-chain funds (or other form of ownership),
/// since messages sent over the bridge will be able to spend these.
#[derive(StructOpt)]
pub enum DeriveAccount {
	/// Given Gateway AccountId, display corresponding Circuit AccountId.
	GatewayToCircuit { account: AccountId },
	/// Given Circuit AccountId, display corresponding Gateway AccountId.
	CircuitToGateway { account: AccountId },
}

impl DeriveAccount {
	/// Run the command.
	pub async fn run(self) -> anyhow::Result<()> {
		super::run_derive_account(self).await.map_err(format_err)?;
		Ok(())
	}
}

fn format_err(err: String) -> anyhow::Error {
	anyhow::anyhow!(err)
}

/// MessagePayload that can be delivered to messages pallet on Circuit.
#[derive(StructOpt, Debug)]
pub enum CircuitToGatewayMessagePayload {
	/// Raw, SCALE-encoded `MessagePayload`.
	Raw {
		/// Hex-encoded SCALE data.
		data: HexBytes,
	},
	/// Construct message to send over the bridge.
	Message {
		/// Message details.
		#[structopt(flatten)]
		message: ToGatewayMessage,
		/// SS58 encoded account that will send the payload (must have SS58Prefix = 42)
		#[structopt(long)]
		sender: AccountId,
	},
}

/// MessagePayload that can be delivered to messages pallet on Gateway.
#[derive(StructOpt, Debug)]
pub enum GatewayToCircuitMessagePayload {
	/// Raw, SCALE-encoded `MessagePayload`.
	Raw {
		/// Hex-encoded SCALE data.
		data: HexBytes,
	},
	/// Construct message to send over the bridge.
	Message {
		/// Message details.
		#[structopt(flatten)]
		message: ToCircuitMessage,
		/// SS58 encoded account that will send the payload (must have SS58Prefix = 42)
		#[structopt(long)]
		sender: AccountId,
	},
}

/// All possible messages that may be delivered to the Gateway chain.
#[derive(StructOpt, Debug)]
pub enum ToGatewayMessage {
	/// Raw bytes for the message
	Raw {
		/// Raw, SCALE-encoded message
		data: HexBytes,
	},
	/// Make an on-chain remark (comment).
	Remark {
		/// Remark size. If not passed, small UTF8-encoded string is generated by relay as remark.
		#[structopt(long)]
		remark_size: Option<ExplicitOrMaximal<usize>>,
	},
	/// Transfer the specified `amount` of native tokens to a particular `recipient`.
	Transfer {
		/// SS58 encoded account that will receive the transfer (must have SS58Prefix = 42)
		#[structopt(long)]
		recipient: AccountId,
		/// Amount of target tokens to send in target chain base currency units.
		#[structopt(long)]
		amount: bp_gateway::Balance,
	},
	/// A call to the Circuit Bridge Messages pallet to send a message over the bridge.
	CircuitSendMessage {
		/// Hex-encoded lane id that should be served by the relay. Defaults to `00000000`.
		#[structopt(long, default_value = "00000000")]
		lane: HexLaneId,
		/// Raw SCALE-encoded Message Payload to submit to the messages pallet.
		#[structopt(long)]
		payload: HexBytes,
		/// Declared delivery and dispatch fee in base source-chain currency units.
		#[structopt(long)]
		fee: bp_gateway::Balance,
	},
}

/// All possible messages that may be delivered to the Circuit chain.
#[derive(StructOpt, Debug)]
pub enum ToCircuitMessage {
	/// Raw bytes for the message
	Raw {
		/// Raw, SCALE-encoded message
		data: HexBytes,
	},
	/// Make an on-chain remark (comment).
	Remark {
		/// Size of the remark. If not passed, small UTF8-encoded string is generated by relay as remark.
		#[structopt(long)]
		remark_size: Option<ExplicitOrMaximal<usize>>,
	},
	/// Transfer the specified `amount` of native tokens to a particular `recipient`.
	Transfer {
		/// SS58 encoded account that will receive the transfer (must have SS58Prefix = 42)
		#[structopt(long)]
		recipient: AccountId,
		/// Amount of target tokens to send in target chain base currency units.
		#[structopt(long)]
		amount: bp_circuit::Balance,
	},
	/// A call to the Gateway Bridge Messages pallet to send a message over the bridge.
	GatewaySendMessage {
		/// Hex-encoded lane id that should be served by the relay. Defaults to `00000000`.
		#[structopt(long, default_value = "00000000")]
		lane: HexLaneId,
		/// Raw SCALE-encoded Message Payload to submit to the messages pallet.
		#[structopt(long)]
		payload: HexBytes,
		/// Declared delivery and dispatch fee in base source-chain currency units.
		#[structopt(long)]
		fee: bp_circuit::Balance,
	},
}

declare_chain_options!(Gateway, gateway);
declare_chain_options!(Circuit, circuit);
declare_chain_options!(Westend, westend);
