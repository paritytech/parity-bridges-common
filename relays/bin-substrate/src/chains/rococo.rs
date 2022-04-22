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

use anyhow::anyhow;
use bp_messages::LaneId;
use bp_runtime::EncodedOrDecodedCall;
use codec::Decode;
use frame_support::weights::{DispatchClass, DispatchInfo, Pays, Weight};
use relay_rococo_client::Rococo;
use relay_substrate_client::BalanceOf;
use sp_version::RuntimeVersion;

use crate::cli::{
	bridge,
	encode_payload::{Payload, CliEncodePayload, RawPayload},
	CliChain,
};

/// Weight of the `system::remark` call at Rococo.
///
/// This weight is larger (x2) than actual weight at current Rococo runtime to avoid unsuccessful
/// calls in the future. But since it is used only in tests (and on test chains), this is ok.
pub(crate) const SYSTEM_REMARK_CALL_WEIGHT: Weight = 2 * 1_345_000;

impl CliEncodePayload for Rococo {
	fn encode_payload(payload: &Payload) -> anyhow::Result<RawPayload> {
		Ok(match payload {
			Payload::Raw { data } => data.0.clone(),
		})
	}

	fn encode_send_message_call(
		lane: LaneId,
		payload: RawPayload,
		fee: BalanceOf<Self>,
		bridge_instance_index: u8,
	) -> anyhow::Result<EncodedOrDecodedCall<Self::Call>> {
		Ok(match bridge_instance_index {
			bridge::ROCOCO_TO_WOCOCO_INDEX => {
				relay_rococo_client::runtime::Call::BridgeWococoMessages(
					relay_rococo_client::runtime::BridgeWococoMessagesCall::send_message(
						lane, payload, fee,
					),
				)
				.into()
			},
			_ => anyhow::bail!(
				"Unsupported target bridge pallet with instance index: {}",
				bridge_instance_index
			),
		})
	}
}
impl CliChain for Rococo {
	const RUNTIME_VERSION: RuntimeVersion = bp_rococo::VERSION;

	type KeyPair = sp_core::sr25519::Pair;
	type MessagePayload = Vec<u8>;

	fn ss58_format() -> u16 {
		42
	}
}
