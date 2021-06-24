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

use codec::Decode;
use frame_support::weights::Weight;
use relay_rococo_client::Rococo;
use sp_version::RuntimeVersion;

use crate::cli::{bridge, encode_call::{Call, CliEncodeCall}, encode_message, CliChain};

impl CliEncodeCall for Rococo {
	fn max_extrinsic_size() -> u32 {
		bp_rococo::max_extrinsic_size()
	}

	fn encode_call(call: &Call) -> anyhow::Result<Self::Call> {
		Ok(match call {
			Call::Remark { remark_payload, .. } => relay_rococo_client::runtime::Call::System(relay_rococo_client::runtime::SystemCall::remark(
				remark_payload.as_ref().map(|x| x.0.clone()).unwrap_or_default(),
			)),
			Call::BridgeSendMessage {
				lane,
				payload,
				fee,
				bridge_instance_index,
			} => match *bridge_instance_index {
				bridge::ROCOCO_TO_WOCOCO_INDEX => {
					let payload = Decode::decode(&mut &*payload.0)?;
					relay_rococo_client::runtime::Call::BridgeMessagesWococo(relay_rococo_client::runtime::BridgeMessagesWococoCall::send_message(
						lane.0, payload, fee.0,
					))
				}
				_ => anyhow::bail!("Unsupported target bridge pallet with instance index: {}", bridge_instance_index),
			},
			_ => anyhow::bail!("The call is not supported"),
		})
	}

	fn get_dispatch_info(call: &relay_rococo_client::runtime::Call) -> frame_support::weights::DispatchInfo {
		match *call {
			relay_rococo_client::runtime::Call::System(relay_rococo_client::runtime::SystemCall::remark(_)) => frame_support::weights::DispatchInfo {
				weight: 1_345_000,
				class: frame_support::weights::DispatchClass::Normal,
				pays_fee: frame_support::weights::Pays::Yes,
			},
			_ => unimplemented!("Unsupported Rococo call: {:?}", call),
		}
	}
}

impl CliChain for Rococo {
	const RUNTIME_VERSION: RuntimeVersion = bp_rococo::VERSION;

	type KeyPair = sp_core::sr25519::Pair;
	type MessagePayload = ();

	fn ss58_format() -> u16 {
		42
	}

	fn max_extrinsic_weight() -> Weight {
		0
	}

	fn encode_message(_message: encode_message::MessagePayload) -> Result<Self::MessagePayload, String> {
		Err("Sending messages from Rococo is not yet supported.".into())
	}
}
