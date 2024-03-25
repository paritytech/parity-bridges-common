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

//! Westend-to-Rococo parachains sync entrypoint.

use crate::cli::bridge::{CliBridgeBase, MessagesCliBridge, ParachainToRelayHeadersCliBridge};
use bp_polkadot_core::parachains::{ParaHash, ParaHeadsProof, ParaId};
use relay_substrate_client::{CallOf, HeaderIdOf};
use substrate_relay_helper::parachains::{
	SubmitParachainHeadsCallBuilder, SubstrateParachainsPipeline,
};

/// BridgeHub-to-BridgeHub parachain sync description.
#[derive(Clone, Debug)]
pub struct BridgeHubRococoToBridgeHubWestend;

impl SubstrateParachainsPipeline for BridgeHubRococoToBridgeHubWestend {
	type SourceParachain = relay_bridge_hub_rococo_client::BridgeHubRococo;
	type SourceRelayChain = relay_rococo_client::Rococo;
	type TargetChain = relay_bridge_hub_westend_client::BridgeHubWestend;

	type SubmitParachainHeadsCallBuilder = BridgeHubRococoToBridgeHubWestendCallBuilder;
}

pub struct BridgeHubRococoToBridgeHubWestendCallBuilder;
impl SubmitParachainHeadsCallBuilder<BridgeHubRococoToBridgeHubWestend>
	for BridgeHubRococoToBridgeHubWestendCallBuilder
{
	fn build_submit_parachain_heads_call(
		at_relay_block: HeaderIdOf<relay_rococo_client::Rococo>,
		parachains: Vec<(ParaId, ParaHash)>,
		parachain_heads_proof: ParaHeadsProof,
		is_free_execution_expected: bool,
	) -> CallOf<relay_bridge_hub_westend_client::BridgeHubWestend> {
		relay_bridge_hub_westend_client::RuntimeCall::BridgeRococoParachains(
			relay_bridge_hub_westend_client::BridgeParachainCall::submit_parachain_heads_ex {
				at_relay_block: (at_relay_block.0, at_relay_block.1),
				parachains,
				parachain_heads_proof,
				is_free_execution_expected,
			},
		)
	}
}

/// `BridgeHubParachain` to `BridgeHubParachain` bridge definition.
pub struct BridgeHubRococoToBridgeHubWestendCliBridge {}

impl ParachainToRelayHeadersCliBridge for BridgeHubRococoToBridgeHubWestendCliBridge {
	type SourceRelay = relay_rococo_client::Rococo;
	type ParachainFinality = BridgeHubRococoToBridgeHubWestend;
	type RelayFinality =
		crate::bridges::rococo_westend::rococo_headers_to_bridge_hub_westend::RococoFinalityToBridgeHubWestend;
}

impl CliBridgeBase for BridgeHubRococoToBridgeHubWestendCliBridge {
	type Source = relay_bridge_hub_rococo_client::BridgeHubRococo;
	type Target = relay_bridge_hub_westend_client::BridgeHubWestend;
}

impl MessagesCliBridge for BridgeHubRococoToBridgeHubWestendCliBridge {
	type MessagesLane =
	crate::bridges::rococo_westend::bridge_hub_rococo_messages_to_bridge_hub_westend::BridgeHubRococoMessagesToBridgeHubWestendMessageLane;
}
