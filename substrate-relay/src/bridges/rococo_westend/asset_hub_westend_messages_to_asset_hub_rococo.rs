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

//! AssetHubWestend-to-AssetHubRococo messages sync entrypoint.

use relay_asset_hub_rococo_client::AssetHubRococo;
use relay_asset_hub_westend_client::AssetHubWestend;
use substrate_relay_helper::{
	cli::bridge::{CliBridgeBase, MessagesCliBridge},
	messages::SubstrateMessageLane,
};

pub struct AssetHubWestendToAssetHubRococoMessagesCliBridge {}

impl CliBridgeBase for AssetHubWestendToAssetHubRococoMessagesCliBridge {
	type Source = AssetHubWestend;
	type Target = AssetHubRococo;
}

impl MessagesCliBridge for AssetHubWestendToAssetHubRococoMessagesCliBridge {
	type MessagesLane = AssetHubWestendMessagesToBridgeHubRococoMessageLane;
}

substrate_relay_helper::generate_receive_message_proof_call_builder!(
	AssetHubWestendMessagesToBridgeHubRococoMessageLane,
	AssetHubWestendMessagesToBridgeHubRococoMessageLaneReceiveMessagesProofCallBuilder,
	relay_asset_hub_rococo_client::RuntimeCall::BridgeWestendMessages,
	relay_asset_hub_rococo_client::BridgeMessagesCall::receive_messages_proof
);

substrate_relay_helper::generate_receive_message_delivery_proof_call_builder!(
	AssetHubWestendMessagesToBridgeHubRococoMessageLane,
	AssetHubWestendMessagesToBridgeHubRococoMessageLaneReceiveMessagesDeliveryProofCallBuilder,
	relay_asset_hub_westend_client::RuntimeCall::BridgeRococoMessages,
	relay_asset_hub_westend_client::BridgeMessagesCall::receive_messages_delivery_proof
);

/// Description of AssetHubWestend -> BridgeHubRococo messages bridge.
#[derive(Clone, Debug)]
pub struct AssetHubWestendMessagesToBridgeHubRococoMessageLane;

impl SubstrateMessageLane for AssetHubWestendMessagesToBridgeHubRococoMessageLane {
	type SourceChain = AssetHubWestend;
	type TargetChain = AssetHubRococo;

	type LaneId = bp_messages::HashedLaneId;

	type ReceiveMessagesProofCallBuilder =
		AssetHubWestendMessagesToBridgeHubRococoMessageLaneReceiveMessagesProofCallBuilder;
	type ReceiveMessagesDeliveryProofCallBuilder =
		AssetHubWestendMessagesToBridgeHubRococoMessageLaneReceiveMessagesDeliveryProofCallBuilder;

	type SourceBatchCallBuilder = ();
	type TargetBatchCallBuilder = ();
}
