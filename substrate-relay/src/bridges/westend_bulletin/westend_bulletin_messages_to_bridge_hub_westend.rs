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

//! WestendBulletin-to-BridgeHubWestend messages sync entrypoint.

use relay_bridge_hub_westend_client::BridgeHubWestend;
use relay_polkadot_bulletin_client::PolkadotBulletin as WestendBulletin;
use substrate_relay_helper::{
	cli::bridge::{CliBridgeBase, MessagesCliBridge},
	messages::SubstrateMessageLane,
	UtilityPalletBatchCallBuilder,
};

/// WestendBulletin-to-BridgeHubWestend messages bridge.
pub struct WestendBulletinToBridgeHubWestendMessagesCliBridge {}

impl CliBridgeBase for WestendBulletinToBridgeHubWestendMessagesCliBridge {
	type Source = WestendBulletin;
	type Target = BridgeHubWestend;
}

impl MessagesCliBridge for WestendBulletinToBridgeHubWestendMessagesCliBridge {
	type MessagesLane = WestendBulletinMessagesToBridgeHubWestendMessageLane;
}

substrate_relay_helper::generate_receive_message_proof_call_builder!(
	WestendBulletinMessagesToBridgeHubWestendMessageLane,
	WestendBulletinMessagesToBridgeHubWestendMessageLaneReceiveMessagesProofCallBuilder,
	relay_bridge_hub_westend_client::RuntimeCall::BridgePolkadotBulletinMessages,
	relay_bridge_hub_westend_client::BridgeBulletinMessagesCall::receive_messages_proof
);

substrate_relay_helper::generate_receive_message_delivery_proof_call_builder!(
	WestendBulletinMessagesToBridgeHubWestendMessageLane,
	WestendBulletinMessagesToBridgeHubWestendMessageLaneReceiveMessagesDeliveryProofCallBuilder,
	relay_polkadot_bulletin_client::RuntimeCall::BridgePolkadotMessages,
	relay_polkadot_bulletin_client::BridgePolkadotMessagesCall::receive_messages_delivery_proof
);

/// WestendBulletin-to-BridgeHubWestend messages lane.
#[derive(Clone, Debug)]
pub struct WestendBulletinMessagesToBridgeHubWestendMessageLane;

impl SubstrateMessageLane for WestendBulletinMessagesToBridgeHubWestendMessageLane {
	type SourceChain = WestendBulletin;
	type TargetChain = BridgeHubWestend;

	type LaneId = bp_messages::LegacyLaneId;

	type ReceiveMessagesProofCallBuilder =
		WestendBulletinMessagesToBridgeHubWestendMessageLaneReceiveMessagesProofCallBuilder;
	type ReceiveMessagesDeliveryProofCallBuilder =
		WestendBulletinMessagesToBridgeHubWestendMessageLaneReceiveMessagesDeliveryProofCallBuilder;

	type SourceBatchCallBuilder = ();
	type TargetBatchCallBuilder = UtilityPalletBatchCallBuilder<BridgeHubWestend>;
}
