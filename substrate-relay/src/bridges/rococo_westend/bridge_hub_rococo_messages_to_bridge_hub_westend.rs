// Copyright 2022 Parity Technologies (UK) Ltd.
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

//! BridgeHubRococo-to-BridgeHubWestend messages sync entrypoint.

use bp_runtime::RelayerVersion;
use hex_literal::hex;
use relay_bridge_hub_rococo_client::BridgeHubRococo;
use relay_bridge_hub_westend_client::BridgeHubWestend;
use sp_core::H256;
use substrate_relay_helper::{
	cli::bridge::{CliBridgeBase, MessagesCliBridge},
	messages_lane::SubstrateMessageLane,
	UtilityPalletBatchCallBuilder,
};

pub struct BridgeHubRococoToBridgeHubWestendMessagesCliBridge {}

impl CliBridgeBase for BridgeHubRococoToBridgeHubWestendMessagesCliBridge {
	type Source = BridgeHubRococo;
	type Target = BridgeHubWestend;
}

impl MessagesCliBridge for BridgeHubRococoToBridgeHubWestendMessagesCliBridge {
	type MessagesLane = BridgeHubRococoMessagesToBridgeHubWestendMessageLane;
}

substrate_relay_helper::generate_receive_message_proof_call_builder!(
	BridgeHubRococoMessagesToBridgeHubWestendMessageLane,
	BridgeHubRococoMessagesToBridgeHubWestendMessageLaneReceiveMessagesProofCallBuilder,
	relay_bridge_hub_westend_client::RuntimeCall::BridgeRococoMessages,
	relay_bridge_hub_westend_client::BridgeMessagesCall::receive_messages_proof
);

substrate_relay_helper::generate_receive_message_delivery_proof_call_builder!(
	BridgeHubRococoMessagesToBridgeHubWestendMessageLane,
	BridgeHubRococoMessagesToBridgeHubWestendMessageLaneReceiveMessagesDeliveryProofCallBuilder,
	relay_bridge_hub_rococo_client::RuntimeCall::BridgeWestendMessages,
	relay_bridge_hub_rococo_client::BridgeMessagesCall::receive_messages_delivery_proof
);

/// Description of BridgeHubRococo -> BridgeHubWestendWestend messages bridge.
#[derive(Clone, Debug)]
pub struct BridgeHubRococoMessagesToBridgeHubWestendMessageLane;

impl SubstrateMessageLane for BridgeHubRococoMessagesToBridgeHubWestendMessageLane {
	const AT_SOURCE_CHAIN_RELAYER_VERSION: Option<RelayerVersion> = Some(RelayerVersion {
		manual: 0,
		auto: H256(hex!("ec797ce348d5ce03001b3000f16b7623b0f6b654e441caf8289608e476223969")),
	});
	const AT_TARGET_CHAIN_RELAYER_VERSION: Option<RelayerVersion> = Some(RelayerVersion {
		manual: 0,
		auto: H256(hex!("815e02321b71bf951f7054f78929c11be5b9056ffc954980d94931b4522a7f02")),
	});

	type SourceChain = BridgeHubRococo;
	type TargetChain = BridgeHubWestend;

	type ReceiveMessagesProofCallBuilder =
		BridgeHubRococoMessagesToBridgeHubWestendMessageLaneReceiveMessagesProofCallBuilder;
	type ReceiveMessagesDeliveryProofCallBuilder =
		BridgeHubRococoMessagesToBridgeHubWestendMessageLaneReceiveMessagesDeliveryProofCallBuilder;

	type SourceBatchCallBuilder = UtilityPalletBatchCallBuilder<BridgeHubRococo>;
	type TargetBatchCallBuilder = UtilityPalletBatchCallBuilder<BridgeHubWestend>;
}
