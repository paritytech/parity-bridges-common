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

use crate::{
	cli::{
		bridge::FullBridge, AccountId, Balance, CliChain, ExplicitOrMaximal, HexBytes, HexLaneId,
	},
	select_full_bridge,
};
use bp_messages::LaneId;
use bp_runtime::EncodedOrDecodedCall;
use frame_support::weights::DispatchInfo;
use relay_substrate_client::{BalanceOf, Chain};
use structopt::StructOpt;
use strum::VariantNames;

/// All possible messages that may be delivered to generic Substrate chain.
///
/// Note this enum may be used in the context of both Source (as part of `encode-call`)
/// and Target chain (as part of `encode-message/send-message`).
#[derive(StructOpt, Debug, PartialEq, Eq)]
pub enum Payload {
	/// Raw bytes for the message
	Raw {
		/// Raw, SCALE-encoded message
		data: HexBytes,
	},
}

/// Raw, SCALE-encoded message payload used in expected deployment.
pub type RawPayload = Vec<u8>;

pub trait CliEncodePayload: Chain {
	/// Encode a CLI payload.
	fn encode_payload(payload: &Payload) -> anyhow::Result<RawPayload>;

	/// Encode a send message call.
	fn encode_send_message_call(
		lane: LaneId,
		payload: RawPayload,
		fee: Self::Balance,
		bridge_instance_index: u8,
	) -> anyhow::Result<EncodedOrDecodedCall<Self::Call>>;
}

pub(crate) fn compute_maximal_message_arguments_size(
	maximal_source_extrinsic_size: u32,
	maximal_target_extrinsic_size: u32,
) -> u32 {
	// assume that both signed extensions and other arguments fit 1KB
	let service_tx_bytes_on_source_chain = 1024;
	let maximal_source_extrinsic_size =
		maximal_source_extrinsic_size - service_tx_bytes_on_source_chain;
	let maximal_call_size = bridge_runtime_common::messages::target::maximal_incoming_message_size(
		maximal_target_extrinsic_size,
	);
	let maximal_call_size = if maximal_call_size > maximal_source_extrinsic_size {
		maximal_source_extrinsic_size
	} else {
		maximal_call_size
	};

	// bytes in Call encoding that are used to encode everything except arguments
	let service_bytes = 1 + 1 + 4;
	maximal_call_size - service_bytes
}
/*
#[cfg(test)]
mod tests {
	use super::*;
	use crate::cli::send_message::SendMessage;

	#[test]
	fn should_encode_transfer_call() {
		// given
		let mut encode_call = EncodeCall::from_iter(vec![
			"encode-call",
			"rialto-to-millau",
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
			"0x040000d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27de5c0"
		);
	}

	#[test]
	fn should_encode_remark_with_default_payload() {
		// given
		let mut encode_call =
			EncodeCall::from_iter(vec!["encode-call", "rialto-to-millau", "remark"]);

		// when
		let hex = encode_call.encode().unwrap();

		// then
		assert!(format!("{:?}", hex).starts_with("0x000154556e69782074696d653a"));
	}

	#[test]
	fn should_encode_remark_with_explicit_payload() {
		// given
		let mut encode_call = EncodeCall::from_iter(vec![
			"encode-call",
			"rialto-to-millau",
			"remark",
			"--remark-payload",
			"1234",
		]);

		// when
		let hex = encode_call.encode().unwrap();

		// then
		assert_eq!(format!("{:?}", hex), "0x0001081234");
	}

	#[test]
	fn should_encode_remark_with_size() {
		// given
		let mut encode_call = EncodeCall::from_iter(vec![
			"encode-call",
			"rialto-to-millau",
			"remark",
			"--remark-size",
			"12",
		]);

		// when
		let hex = encode_call.encode().unwrap();

		// then
		assert_eq!(format!("{:?}", hex), "0x000130000000000000000000000000");
	}

	#[test]
	fn should_disallow_both_payload_and_size() {
		// when
		let err = EncodeCall::from_iter_safe(vec![
			"encode-call",
			"rialto-to-millau",
			"remark",
			"--remark-payload",
			"1234",
			"--remark-size",
			"12",
		])
		.unwrap_err();

		// then
		assert_eq!(err.kind, structopt::clap::ErrorKind::ArgumentConflict);

		let info = err.info.unwrap();
		assert!(
			info.contains(&"remark-payload".to_string()) |
				info.contains(&"remark-size".to_string())
		)
	}

	#[test]
	fn should_encode_raw_call() {
		// given
		let mut encode_call = EncodeCall::from_iter(vec![
			"encode-call",
			"rialto-to-millau",
			"raw",
			"040000d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27de5c0",
		]);

		// when
		let hex = encode_call.encode().unwrap();

		// then
		assert_eq!(
			format!("{:?}", hex),
			"0x040000d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27de5c0"
		);
	}

	#[async_std::test]
	async fn should_encode_bridge_send_message_call() {
		// given
		let encode_message = SendMessage::from_iter(vec![
			"send-message",
			"millau-to-rialto",
			"--source-port",
			"10946",
			"--source-signer",
			"//Alice",
			"--target-signer",
			"//Alice",
			"--origin",
			"Target",
			"remark",
		])
		.encode_payload()
		.await
		.unwrap();

		let mut encode_call = EncodeCall::from_iter(vec![
			"encode-call",
			"rialto-to-millau",
			"bridge-send-message",
			"--fee",
			"12345",
			"--payload",
			format!("{:}", &HexBytes::encode(&encode_message)).as_str(),
		]);

		// when
		let call_hex = encode_call.encode().unwrap();

		// then
		assert!(format!("{:?}", call_hex).starts_with(
			"0x0f030000000001000000000000000000000001d43593c715fdd31c61141abd04a99fd6822c8558854cc\
			de39a5684e7a56da27d01d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d01"
		))
	}
}
*/