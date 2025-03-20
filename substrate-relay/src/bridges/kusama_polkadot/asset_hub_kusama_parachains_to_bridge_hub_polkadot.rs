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

//! AssetHubKusama-to-BridgeHubPolkadot parachains sync entrypoint.

use bp_polkadot_core::parachains::{ParaHash, ParaHeadsProof, ParaId};
use relay_substrate_client::{CallOf, HeaderIdOf};
use substrate_relay_helper::{
	cli::bridge::{CliBridgeBase, MessagesCliBridge, ParachainToRelayHeadersCliBridge},
	parachains::{SubmitParachainHeadsCallBuilder, SubstrateParachainsPipeline},
};

/// AssetHubKusama-to-BridgeHubPolkadot parachain sync description.
#[derive(Clone, Debug)]
pub struct AssetHubKusamaToBridgeHubPolkadot;

impl SubstrateParachainsPipeline for AssetHubKusamaToBridgeHubPolkadot {
	type SourceParachain = relay_asset_hub_kusama_client::AssetHubKusama;
	type SourceRelayChain = relay_kusama_client::Kusama;
	type TargetChain = relay_bridge_hub_polkadot_client::BridgeHubPolkadot;

	type SubmitParachainHeadsCallBuilder = AssetHubKusamaToBridgeHubPolkadotCallBuilder;
}

pub struct AssetHubKusamaToBridgeHubPolkadotCallBuilder;
impl SubmitParachainHeadsCallBuilder<AssetHubKusamaToBridgeHubPolkadot>
	for AssetHubKusamaToBridgeHubPolkadotCallBuilder
{
	fn build_submit_parachain_heads_call(
		at_relay_block: HeaderIdOf<relay_kusama_client::Kusama>,
		parachains: Vec<(ParaId, ParaHash)>,
		parachain_heads_proof: ParaHeadsProof,
		_is_free_execution_expected: bool,
	) -> CallOf<relay_bridge_hub_polkadot_client::BridgeHubPolkadot> {
		relay_bridge_hub_polkadot_client::RuntimeCall::BridgeKusamaParachains(
			relay_bridge_hub_polkadot_client::BridgeParachainCall::submit_parachain_heads {
				at_relay_block: (at_relay_block.0, at_relay_block.1),
				parachains,
				parachain_heads_proof,
			},
		)
	}
}

/// AssetHubKusama-to-BridgeHubPolkadot parachain sync description for the CLI.
pub struct AssetHubKusamaToBridgeHubPolkadotCliBridge {}

impl ParachainToRelayHeadersCliBridge for AssetHubKusamaToBridgeHubPolkadotCliBridge {
	type SourceRelay = relay_kusama_client::Kusama;
	type ParachainFinality = AssetHubKusamaToBridgeHubPolkadot;
	type RelayFinality =
		crate::bridges::kusama_polkadot::kusama_headers_to_bridge_hub_polkadot::KusamaFinalityToBridgeHubPolkadot;
}

impl CliBridgeBase for AssetHubKusamaToBridgeHubPolkadotCliBridge {
	type Source = relay_asset_hub_kusama_client::AssetHubKusama;
	type Target = relay_bridge_hub_polkadot_client::BridgeHubPolkadot;
}

impl MessagesCliBridge for AssetHubKusamaToBridgeHubPolkadotCliBridge {
	type MessagesLane =
	crate::bridges::kusama_polkadot::bridge_hub_kusama_messages_to_bridge_hub_polkadot::BridgeHubKusamaMessagesToBridgeHubPolkadotMessageLane;
}
