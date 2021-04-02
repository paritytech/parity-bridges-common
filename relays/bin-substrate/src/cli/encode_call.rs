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

use crate::cli::{AccountId, Balance, CliChain, ExplicitOrMaximal, HexBytes, HexLaneId};
use frame_support::dispatch::GetDispatchInfo;
use relay_substrate_client::Chain;
use structopt::{clap::arg_enum, StructOpt};

/// Encode source chain runtime call.
#[derive(StructOpt)]
pub struct EncodeCall {
	/// A bridge instance to encode call for.
	#[structopt(possible_values = &EncodeCallBridge::variants(), case_insensitive = true)]
	bridge: EncodeCallBridge,
	#[structopt(flatten)]
	call: Call,
}

/// All possible messages that may be delivered to generic Substrate chain.
///
/// Note this enum may be used in the context of both Source (as part of `encode-call`)
/// and Target chain (as part of `encode-message/send-message`).
#[derive(StructOpt, Debug)]
pub enum Call {
	/// Raw bytes for the message
	Raw {
		/// Raw, SCALE-encoded message
		data: HexBytes,
	},
	/// Make an on-chain remark (comment).
	Remark {
		/// Explicit remark payload.
		#[structopt(long, conflicts_with("remark_size"))]
		remark_payload: HexBytes,
		/// Remark size. If not passed, small UTF8-encoded string is generated by relay as remark.
		#[structopt(long, conflicts_with("remark_payload"))]
		remark_size: Option<ExplicitOrMaximal<usize>>,
	},
	/// Transfer the specified `amount` of native tokens to a particular `recipient`.
	Transfer {
		/// Address of an account to receive the transfer.
		#[structopt(long)]
		recipient: AccountId,
		/// Amount of target tokens to send in target chain base currency units.
		#[structopt(long)]
		amount: Balance,
	},
	/// A call to the specific Bridge Messages pallet to queue message to be sent over a bridge.
	BridgeSendMessage {
		/// An index of the bridge instance which represents the expected target chain.
		#[structopt(skip = 255)]
		bridge_instance_index: u8,
		/// Hex-encoded lane id that should be served by the relay. Defaults to `00000000`.
		#[structopt(long, default_value = "00000000")]
		lane: HexLaneId,
		/// Raw SCALE-encoded Message Payload to submit to the messages pallet.
		///
		/// This can be obtained by encoding call for the target chain.
		#[structopt(long)]
		payload: HexBytes,
		/// Declared delivery and dispatch fee in base source-chain currency units.
		#[structopt(long)]
		fee: Balance,
	},
}

pub trait CliEncodeCall: Chain {
	/// Maximal size (in bytes) of any extrinsic (from the runtime).
	fn max_extrinsic_size() -> u32;

	/// Encode a CLI call.
	fn encode_call(call: &Call) -> anyhow::Result<Self::Call>;
}

arg_enum! {
	#[derive(Debug)]
	/// Bridge to encode call for.
	pub enum EncodeCallBridge {
		MillauToRialto,
		RialtoToMillau,
	}
}

impl EncodeCallBridge {
	fn bridge_instance_index(&self) -> u8 {
		match self {
			Self::MillauToRialto => MILLAU_TO_RIALTO_INDEX,
			Self::RialtoToMillau => RIALTO_TO_MILLAU_INDEX,
		}
	}
}

pub const RIALTO_TO_MILLAU_INDEX: u8 = 0;
pub const MILLAU_TO_RIALTO_INDEX: u8 = 0;

#[macro_export]
macro_rules! select_bridge {
	($bridge: expr, $generic: tt) => {
		match $bridge {
			EncodeCallBridge::MillauToRialto => {
				type Source = relay_millau_client::Millau;
				type Target = relay_rialto_client::Rialto;

				$generic
			}
			EncodeCallBridge::RialtoToMillau => {
				type Source = relay_rialto_client::Rialto;
				type Target = relay_millau_client::Millau;

				$generic
			}
		}
	};
}

impl EncodeCall {
	fn encode(&mut self) -> anyhow::Result<HexBytes> {
		select_bridge!(self.bridge, {
			preprocess_call::<Source, Target>(&mut self.call, self.bridge.bridge_instance_index());
			let call = Source::encode_call(&self.call)?;

			let encoded = HexBytes::encode(&call);

			log::info!(target: "bridge", "Generated {} call: {:#?}", Source::NAME, call);
			log::info!(target: "bridge", "Weight of {} call: {}", Source::NAME, call.get_dispatch_info().weight);
			log::info!(target: "bridge", "Encoded {} call: {:?}", Source::NAME, encoded);

			Ok(encoded)
		})
	}

	/// Run the command.
	pub async fn run(mut self) -> anyhow::Result<()> {
		println!("{:?}", self.encode()?);
		Ok(())
	}
}

/// Prepare the call to be passed to [`CliEncodeCall::encode_call`].
///
/// This function will fill in all optional and missing pieces and will make sure that
/// values are converted to bridge-specific ones.
///
/// Most importantly, the method will fill-in [`bridge_instance_index`] parameter for
/// target-chain specific calls.
pub(crate) fn preprocess_call<Source: CliEncodeCall + CliChain, Target: CliEncodeCall>(
	call: &mut Call,
	bridge_instance: u8,
) {
	match *call {
		Call::Raw { .. } => {}
		Call::Remark {
			ref remark_size,
			ref mut remark_payload,
		} => {
			if remark_payload.0.is_empty() {
				*remark_payload = HexBytes(generate_remark_payload(
					&remark_size,
					compute_maximal_message_arguments_size(Source::max_extrinsic_size(), Target::max_extrinsic_size()),
				));
			}
		}
		Call::Transfer { ref mut recipient, .. } => {
			recipient.enforce_chain::<Source>();
		}
		Call::BridgeSendMessage {
			ref mut bridge_instance_index,
			..
		} => {
			*bridge_instance_index = bridge_instance;
		}
	};
}

fn generate_remark_payload(remark_size: &Option<ExplicitOrMaximal<usize>>, maximal_allowed_size: u32) -> Vec<u8> {
	match remark_size {
		Some(ExplicitOrMaximal::Explicit(remark_size)) => vec![0; *remark_size],
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

pub(crate) fn compute_maximal_message_arguments_size(
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

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn should_encode_transfer_call() {
		// given
		let mut encode_call = EncodeCall::from_iter(vec![
			"encode-call",
			"RialtoToMillau",
			"transfer",
			"--amount",
			"12345",
			"--recipient",
			"5sauUXUfPjmwxSgmb3tZ5d6yx24eZX4wWJ2JtVUBaQqFbvEU",
		]);

		// when
		let hex = encode_call.encode().unwrap();

		// then
		assert_eq!(
			format!("{:?}", hex),
			"0x0c00d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27de5c0"
		);
	}
}
