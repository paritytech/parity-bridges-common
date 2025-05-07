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

//! PeopleHubPolkadot-to-PolkadotBulletin messages sync entrypoint.

use relay_people_hub_polkadot_client::PeopleHubPolkadot;
use relay_polkadot_bulletin_client::PolkadotBulletin;
use substrate_relay_helper::{
	cli::bridge::{CliBridgeBase, MessagesCliBridge},
	messages::SubstrateMessageLane,
	UtilityPalletBatchCallBuilder,
};

/// PeopleHubPolkadot-to-PolkadotBulletin messages bridge.
pub struct PeopleHubPolkadotToPolkadotBulletinMessagesCliBridge {}

impl CliBridgeBase for PeopleHubPolkadotToPolkadotBulletinMessagesCliBridge {
	type Source = PeopleHubPolkadot;
	type Target = PolkadotBulletin;
}

impl MessagesCliBridge for PeopleHubPolkadotToPolkadotBulletinMessagesCliBridge {
	type MessagesLane = PeopleHubPolkadotMessagesToPolkadotBulletinMessageLane;
}

substrate_relay_helper::generate_receive_message_proof_call_builder!(
	PeopleHubPolkadotMessagesToPolkadotBulletinMessageLane,
	PeopleHubPolkadotMessagesToPolkadotBulletinMessageLaneReceiveMessagesProofCallBuilder,
	relay_polkadot_bulletin_client::RuntimeCall::BridgePolkadotMessages,
	relay_polkadot_bulletin_client::BridgePolkadotMessagesCall::receive_messages_proof
);

substrate_relay_helper::generate_receive_message_delivery_proof_call_builder!(
	PeopleHubPolkadotMessagesToPolkadotBulletinMessageLane,
	PeopleHubPolkadotMessagesToPolkadotBulletinMessageLaneReceiveMessagesDeliveryProofCallBuilder,
	// TODO: https://github.com/paritytech/parity-bridges-common/issues/2547 - use BridgePolkadotBulletinMessages
	relay_people_hub_polkadot_client::RuntimeCall::BridgePolkadotBulletinMessages,
	relay_people_hub_polkadot_client::BridgePolkadotBulletinMessagesCall::receive_messages_delivery_proof
);

/// PeopleHubPolkadot-to-PolkadotBulletin messages lane.
#[derive(Clone, Debug)]
pub struct PeopleHubPolkadotMessagesToPolkadotBulletinMessageLane;

impl SubstrateMessageLane for PeopleHubPolkadotMessagesToPolkadotBulletinMessageLane {
	type SourceChain = PeopleHubPolkadot;
	type TargetChain = PolkadotBulletin;

	type LaneId = bp_messages::LegacyLaneId;

	type ReceiveMessagesProofCallBuilder =
		PeopleHubPolkadotMessagesToPolkadotBulletinMessageLaneReceiveMessagesProofCallBuilder;
	type ReceiveMessagesDeliveryProofCallBuilder =
		PeopleHubPolkadotMessagesToPolkadotBulletinMessageLaneReceiveMessagesDeliveryProofCallBuilder;

	type SourceBatchCallBuilder = UtilityPalletBatchCallBuilder<PeopleHubPolkadot>;
	type TargetBatchCallBuilder = ();
}
