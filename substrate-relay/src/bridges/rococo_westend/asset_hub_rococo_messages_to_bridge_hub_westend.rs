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

//! AssetHubRococo-to-BridgeHubWestend messages sync entrypoint.

use relay_asset_hub_rococo_client::AssetHubRococo;
use relay_bridge_hub_westend_client::BridgeHubWestend;
use substrate_relay_helper::{
	cli::bridge::{CliBridgeBase, MessagesCliBridge},
	messages::SubstrateMessageLane,
	UtilityPalletBatchCallBuilder,
};

pub struct AssetHubRococoToAssetHubWestendMessagesCliBridge {}

impl CliBridgeBase for AssetHubRococoToAssetHubWestendMessagesCliBridge {
	type Source = AssetHubRococo;
	type Target = AssetHubWestend;
}

impl MessagesCliBridge for AssetHubRococoToBridgeHubWestendMessagesCliBridge {
	type MessagesLane = AssetHubRococoMessagesToBridgeHubWestendMessageLane;
}

substrate_relay_helper::generate_receive_message_proof_call_builder!(
	AssetHubRococoMessagesToBridgeHubWestendMessageLane,
	AssetHubRococoMessagesToBridgeHubWestendMessageLaneReceiveMessagesProofCallBuilder,
	relay_bridge_hub_westend_client::RuntimeCall::BridgeRococoMessages,
	relay_bridge_hub_westend_client::BridgeMessagesCall::receive_messages_proof
);

substrate_relay_helper::generate_receive_message_delivery_proof_call_builder!(
	AssetHubRococoMessagesToBridgeHubWestendMessageLane,
	AssetHubRococoMessagesToBridgeHubWestendMessageLaneReceiveMessagesDeliveryProofCallBuilder,
	relay_asset_hub_rococo_client::RuntimeCall::BridgeWestendMessages,
	relay_bridge_hub_rococo_client::BridgeMessagesCall::receive_messages_delivery_proof
);

/// Description of AssetHubRococo -> BridgeHubWestend messages bridge.
#[derive(Clone, Debug)]
pub struct AssetHubRococoMessagesToBridgeHubWestendMessageLane;

impl SubstrateMessageLane for AssetHubRococoMessagesToBridgeHubWestendMessageLane {
	type SourceChain = AssetHubRococo;
	type TargetChain = BridgeHubWestend;

	type LaneId = bp_messages::LegacyLaneId;

	type ReceiveMessagesProofCallBuilder =
		AssetHubRococoMessagesToBridgeHubWestendMessageLaneReceiveMessagesProofCallBuilder;
	type ReceiveMessagesDeliveryProofCallBuilder =
		AssetHubRococoMessagesToBridgeHubWestendMessageLaneReceiveMessagesDeliveryProofCallBuilder;

	type SourceBatchCallBuilder = UtilityPalletBatchCallBuilder<AssetHubRococo>;
	type TargetBatchCallBuilder = UtilityPalletBatchCallBuilder<BridgeHubWestend>;
}
