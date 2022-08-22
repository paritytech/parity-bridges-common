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

//! Rococo-to-Wococo bridge hubs headers sync entrypoint.

use crate::cli::bridge::{CliBridgeBase, RelayToRelayHeadersCliBridge};
use substrate_relay_helper::finality::{
	engine::Grandpa as GrandpaFinalityEngine, SubstrateFinalitySyncPipeline,
};

/// Description of Rococo -> Wococo finalized headers bridge.
#[derive(Clone, Debug)]
pub struct BridgeHubRococoFinalityToBridgeHubWococo;

substrate_relay_helper::generate_mocked_submit_finality_proof_call_builder!(
	BridgeHubRococoFinalityToBridgeHubWococo,
	BridgeHubRococoFinalityToBridgeHubWococoCallBuilder,
	relay_bridge_hub_wococo_client::runtime::Call::BridgeGrandpaRococo,
	relay_bridge_hub_wococo_client::runtime::BridgeGrandpaRococoCall::submit_finality_proof
);

impl SubstrateFinalitySyncPipeline for BridgeHubRococoFinalityToBridgeHubWococo {
	type SourceChain = relay_bridge_hub_rococo_client::BridgeHubRococo;
	type TargetChain = relay_bridge_hub_wococo_client::BridgeHubWococo;

	type FinalityEngine = GrandpaFinalityEngine<Self::SourceChain>;
	type SubmitFinalityProofCallBuilder = BridgeHubRococoFinalityToBridgeHubWococoCallBuilder;
	type TransactionSignScheme = relay_bridge_hub_wococo_client::BridgeHubWococo;
}

/// BridgeHub `Rococo` to BridgeHub `Wococo` bridge definition.
pub struct BridgeHubRococoToBridgeHubWococoCliBridge {}

impl CliBridgeBase for BridgeHubRococoToBridgeHubWococoCliBridge {
	type Source = relay_bridge_hub_rococo_client::BridgeHubRococo;
	type Target = relay_bridge_hub_wococo_client::BridgeHubWococo;
}

impl RelayToRelayHeadersCliBridge for BridgeHubRococoToBridgeHubWococoCliBridge {
	type Finality = BridgeHubRococoFinalityToBridgeHubWococo;
}
