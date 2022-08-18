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
use bp_runtime::HeaderOf;
use relay_substrate_client::{CallOf, SyncHeader};
use substrate_relay_helper::finality::{
	engine::Grandpa as GrandpaFinalityEngine, source::SubstrateFinalityProof,
	DirectSubmitGrandpaFinalityProofCallBuilder, SubmitFinalityProofCallBuilder,
	SubstrateFinalitySyncPipeline,
};

/// Description of Rococo -> Wococo finalized headers bridge.
#[derive(Clone, Debug)]
pub struct RococoFinalityToWococo;

impl SubstrateFinalitySyncPipeline for RococoFinalityToWococo {
	type SourceChain = relay_bridge_hub_rococo_client::BridgeHubRococo;
	type TargetChain = relay_bridge_hub_wococo_client::BridgeHubWococo;

	type FinalityEngine = GrandpaFinalityEngine<Self::SourceChain>;
	type SubmitFinalityProofCallBuilder = DirectSubmitGrandpaFinalityProofCallBuilder<
		Self,
		relay_bridge_hub_wococo_client::runtime::Runtime,
		relay_bridge_hub_wococo_client::runtime::BridgeGrandpaRococoInstance,
	>;
	type TransactionSignScheme = relay_bridge_hub_wococo_client::BridgeHubWococo;
}

/// `Rococo` to `Wococo` bridge definition.
pub struct RococoToWococoCliBridge {}

impl CliBridgeBase for RococoToWococoCliBridge {
	type Source = relay_bridge_hub_rococo_client::BridgeHubRococo;
	type Target = relay_bridge_hub_wococo_client::BridgeHubWococo;
}

impl RelayToRelayHeadersCliBridge for RococoToWococoCliBridge {
	type Finality = RococoFinalityToWococo;
}
