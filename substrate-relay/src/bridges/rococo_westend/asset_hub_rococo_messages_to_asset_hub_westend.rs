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

//! AssetHubRococo-to-AssetHubWestend messages sync entrypoint.

use relay_asset_hub_rococo_client::AssetHubRococo;
use relay_asset_hub_westend_client::AssetHubWestend;
use substrate_relay_helper::{
	cli::bridge::{CliBridgeBase, MessagesCliBridge},
	messages::SubstrateMessageLane,
};

pub struct AssetHubRococoToAssetHubWestendMessagesCliBridge {}

impl CliBridgeBase for AssetHubRococoToAssetHubWestendMessagesCliBridge {
	type Source = AssetHubRococo;
	type Target = AssetHubWestend;
}

impl MessagesCliBridge for AssetHubRococoToAssetHubWestendMessagesCliBridge {
	type MessagesLane = AssetHubRococoMessagesToAssetHubWestendMessageLane;
}

substrate_relay_helper::generate_receive_message_proof_call_builder!(
	AssetHubRococoMessagesToAssetHubWestendMessageLane,
	AssetHubRococoMessagesToAssetHubWestendMessageLaneReceiveMessagesProofCallBuilder,
	relay_asset_hub_westend_client::RuntimeCall::BridgeRococoMessages,
	relay_asset_hub_westend_client::BridgeMessagesCall::receive_messages_proof
);

substrate_relay_helper::generate_receive_message_delivery_proof_call_builder!(
	AssetHubRococoMessagesToAssetHubWestendMessageLane,
	AssetHubRococoMessagesToAssetHubWestendMessageLaneReceiveMessagesDeliveryProofCallBuilder,
	relay_asset_hub_rococo_client::RuntimeCall::BridgeWestendMessages,
	relay_asset_hub_rococo_client::BridgeMessagesCall::receive_messages_delivery_proof
);

/// Description of AssetHubRococo -> AssetHubWestend messages bridge.
#[derive(Clone, Debug)]
pub struct AssetHubRococoMessagesToAssetHubWestendMessageLane;

impl SubstrateMessageLane for AssetHubRococoMessagesToAssetHubWestendMessageLane {
	type SourceChain = AssetHubRococo;
	type TargetChain = AssetHubWestend;

	type LaneId = bp_messages::HashedLaneId;

	type ReceiveMessagesProofCallBuilder =
		AssetHubRococoMessagesToAssetHubWestendMessageLaneReceiveMessagesProofCallBuilder;
	type ReceiveMessagesDeliveryProofCallBuilder =
		AssetHubRococoMessagesToAssetHubWestendMessageLaneReceiveMessagesDeliveryProofCallBuilder;

	type SourceBatchCallBuilder = ();
	type TargetBatchCallBuilder = ();
}
