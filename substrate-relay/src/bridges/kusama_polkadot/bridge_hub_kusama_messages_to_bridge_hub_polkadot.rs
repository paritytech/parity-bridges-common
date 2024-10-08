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

//! BridgeHubKusama-to-BridgeHubPolkadot messages sync entrypoint.

use relay_bridge_hub_kusama_client::BridgeHubKusama;
use relay_bridge_hub_polkadot_client::BridgeHubPolkadot;
use substrate_relay_helper::{
	cli::bridge::{CliBridgeBase, MessagesCliBridge},
	messages::SubstrateMessageLane,
	UtilityPalletBatchCallBuilder,
};

/// BridgeHubKusama-to-BridgeHubPolkadot messages bridge.
pub struct BridgeHubKusamaToBridgeHubPolkadotMessagesCliBridge {}

impl CliBridgeBase for BridgeHubKusamaToBridgeHubPolkadotMessagesCliBridge {
	type Source = BridgeHubKusama;
	type Target = BridgeHubPolkadot;
}

impl MessagesCliBridge for BridgeHubKusamaToBridgeHubPolkadotMessagesCliBridge {
	type MessagesLane = BridgeHubKusamaMessagesToBridgeHubPolkadotMessageLane;
}

substrate_relay_helper::generate_receive_message_proof_call_builder!(
	BridgeHubKusamaMessagesToBridgeHubPolkadotMessageLane,
	BridgeHubKusamaMessagesToBridgeHubPolkadotMessageLaneReceiveMessagesProofCallBuilder,
	relay_bridge_hub_polkadot_client::RuntimeCall::BridgeKusamaMessages,
	relay_bridge_hub_polkadot_client::BridgeKusamaMessagesCall::receive_messages_proof
);

substrate_relay_helper::generate_receive_message_delivery_proof_call_builder!(
	BridgeHubKusamaMessagesToBridgeHubPolkadotMessageLane,
	BridgeHubKusamaMessagesToBridgeHubPolkadotMessageLaneReceiveMessagesDeliveryProofCallBuilder,
	relay_bridge_hub_kusama_client::RuntimeCall::BridgePolkadotMessages,
	relay_bridge_hub_kusama_client::BridgeMessagesCall::receive_messages_delivery_proof
);

/// BridgeHubKusama-to-BridgeHubPolkadot messages lane.
#[derive(Clone, Debug)]
pub struct BridgeHubKusamaMessagesToBridgeHubPolkadotMessageLane;

impl SubstrateMessageLane for BridgeHubKusamaMessagesToBridgeHubPolkadotMessageLane {
	type SourceChain = BridgeHubKusama;
	type TargetChain = BridgeHubPolkadot;

	type LaneId = bp_messages::LegacyLaneId;

	type ReceiveMessagesProofCallBuilder =
		BridgeHubKusamaMessagesToBridgeHubPolkadotMessageLaneReceiveMessagesProofCallBuilder;
	type ReceiveMessagesDeliveryProofCallBuilder =
		BridgeHubKusamaMessagesToBridgeHubPolkadotMessageLaneReceiveMessagesDeliveryProofCallBuilder;

	type SourceBatchCallBuilder = UtilityPalletBatchCallBuilder<BridgeHubKusama>;
	type TargetBatchCallBuilder = UtilityPalletBatchCallBuilder<BridgeHubPolkadot>;
}
