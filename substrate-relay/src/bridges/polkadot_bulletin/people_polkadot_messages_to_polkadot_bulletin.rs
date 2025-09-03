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

//! PeoplePolkadot-to-PolkadotBulletin messages sync entrypoint.

use relay_people_polkadot_client::PeoplePolkadot;
use relay_polkadot_bulletin_client::PolkadotBulletin;
use substrate_relay_helper::{
	cli::bridge::{CliBridgeBase, MessagesCliBridge},
	messages::SubstrateMessageLane,
	UtilityPalletBatchCallBuilder,
};

/// PeoplePolkadot-to-PolkadotBulletin messages bridge.
pub struct PeoplePolkadotToPolkadotBulletinMessagesCliBridge {}

impl CliBridgeBase for PeoplePolkadotToPolkadotBulletinMessagesCliBridge {
	type Source = PeoplePolkadot;
	type Target = PolkadotBulletin;
}

impl MessagesCliBridge for PeoplePolkadotToPolkadotBulletinMessagesCliBridge {
	type MessagesLane = PeoplePolkadotMessagesToPolkadotBulletinMessageLane;
}

substrate_relay_helper::generate_receive_message_proof_call_builder!(
	PeoplePolkadotMessagesToPolkadotBulletinMessageLane,
	PeoplePolkadotMessagesToPolkadotBulletinMessageLaneReceiveMessagesProofCallBuilder,
	relay_polkadot_bulletin_client::RuntimeCall::BridgePolkadotMessages,
	relay_polkadot_bulletin_client::BridgePolkadotMessagesCall::receive_messages_proof
);

substrate_relay_helper::generate_receive_message_delivery_proof_call_builder!(
	PeoplePolkadotMessagesToPolkadotBulletinMessageLane,
	PeoplePolkadotMessagesToPolkadotBulletinMessageLaneReceiveMessagesDeliveryProofCallBuilder,
	relay_people_polkadot_client::RuntimeCall::BridgePolkadotBulletinMessages,
	relay_people_polkadot_client::BridgePolkadotBulletinMessagesCall::receive_messages_delivery_proof
);

/// PeoplePolkadot-to-PolkadotBulletin messages lane.
#[derive(Clone, Debug)]
pub struct PeoplePolkadotMessagesToPolkadotBulletinMessageLane;

impl SubstrateMessageLane for PeoplePolkadotMessagesToPolkadotBulletinMessageLane {
	type SourceChain = PeoplePolkadot;
	type TargetChain = PolkadotBulletin;

	type LaneId = bp_messages::LegacyLaneId;

	type ReceiveMessagesProofCallBuilder =
		PeoplePolkadotMessagesToPolkadotBulletinMessageLaneReceiveMessagesProofCallBuilder;
	type ReceiveMessagesDeliveryProofCallBuilder =
		PeoplePolkadotMessagesToPolkadotBulletinMessageLaneReceiveMessagesDeliveryProofCallBuilder;

	type SourceBatchCallBuilder = UtilityPalletBatchCallBuilder<PeoplePolkadot>;
	type TargetBatchCallBuilder = ();
}
