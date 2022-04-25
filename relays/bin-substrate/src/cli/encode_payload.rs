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
