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

//! Polkadot-to-Kusama messages sync entrypoint.

use frame_support::weights::Weight;
use messages_relay::relay_strategy::MixStrategy;
use relay_kusama_client::Kusama;
use relay_polkadot_client::Polkadot;
use substrate_relay_helper::messages_lane::SubstrateMessageLane;

/// Description of Polkadot -> Kusama messages bridge.
#[derive(Clone, Debug)]
pub struct PolkadotMessagesToKusama;
substrate_relay_helper::generate_mocked_receive_message_proof_call_builder!(
	PolkadotMessagesToKusama,
	PolkadotMessagesToKusamaReceiveMessagesProofCallBuilder,
	relay_kusama_client::runtime::Call::BridgePolkadotMessages,
	relay_kusama_client::runtime::BridgePolkadotMessagesCall::receive_messages_proof
);
substrate_relay_helper::generate_mocked_receive_message_delivery_proof_call_builder!(
	PolkadotMessagesToKusama,
	PolkadotMessagesToKusamaReceiveMessagesDeliveryProofCallBuilder,
	relay_polkadot_client::runtime::Call::BridgeKusamaMessages,
	relay_polkadot_client::runtime::BridgeKusamaMessagesCall::receive_messages_delivery_proof
);
substrate_relay_helper::generate_mocked_update_conversion_rate_call_builder!(
	Polkadot,
	PolkadotMessagesToKusamaUpdateConversionRateCallBuilder,
	relay_polkadot_client::runtime::Call::BridgeKusamaMessages,
	relay_polkadot_client::runtime::BridgeKusamaMessagesCall::update_pallet_parameter,
	relay_polkadot_client::runtime::BridgeKusamaMessagesParameter::KusamaToPolkadotConversionRate
);

impl SubstrateMessageLane for PolkadotMessagesToKusama {
	const SOURCE_TO_TARGET_CONVERSION_RATE_PARAMETER_NAME: Option<&'static str> =
		Some(bp_kusama::POLKADOT_TO_KUSAMA_CONVERSION_RATE_PARAMETER_NAME);
	const TARGET_TO_SOURCE_CONVERSION_RATE_PARAMETER_NAME: Option<&'static str> =
		Some(bp_polkadot::KUSAMA_TO_POLKADOT_CONVERSION_RATE_PARAMETER_NAME);

	type SourceChain = Polkadot;
	type TargetChain = Kusama;

	type SourceTransactionSignScheme = Polkadot;
	type TargetTransactionSignScheme = Kusama;

	type ReceiveMessagesProofCallBuilder = PolkadotMessagesToKusamaReceiveMessagesProofCallBuilder;
	type ReceiveMessagesDeliveryProofCallBuilder =
		PolkadotMessagesToKusamaReceiveMessagesDeliveryProofCallBuilder;

	type TargetToSourceChainConversionRateUpdateBuilder = PolkadotMessagesToKusamaUpdateConversionRateCallBuilder;

	type RelayStrategy = MixStrategy;
}
