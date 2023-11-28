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

//! Rococo-to-RococoBulletin parachains sync entrypoint.

use crate::cli::bridge::{CliBridgeBase, MessagesCliBridge, ParachainToRelayHeadersCliBridge};

use bp_polkadot_core::parachains::{ParaHash, ParaHeadsProof, ParaId};
use bp_runtime::Chain;
use relay_substrate_client::{CallOf, Chain as _, HeaderIdOf};
use substrate_relay_helper::{
	messages_lane::MessagesRelayLimits,
	parachains::{SubmitParachainHeadsCallBuilder, SubstrateParachainsPipeline},
};

/// Rococo-to-RococoBulletin parachain sync description.
#[derive(Clone, Debug)]
pub struct RococoToRococoBulletin;

impl SubstrateParachainsPipeline for RococoToRococoBulletin {
	type SourceParachain = relay_bridge_hub_rococo_client::BridgeHubRococo;
	type SourceRelayChain = relay_rococo_client::Rococo;
	type TargetChain = relay_polkadot_bulletin_client::PolkadotBulletin;

	type SubmitParachainHeadsCallBuilder = RococoToRococoBulletinCallBuilder;

	fn best_finalized_source_at_target_method() -> String {
		relay_bridge_hub_polkadot_client::BridgeHubPolkadot::BEST_FINALIZED_HEADER_ID_METHOD.into()
	}

	fn best_finalized_source_relay_at_target_method() -> String {
		relay_polkadot_client::Polkadot::BEST_FINALIZED_HEADER_ID_METHOD.into()
	}
}

pub struct RococoToRococoBulletinCallBuilder;
impl SubmitParachainHeadsCallBuilder<RococoToRococoBulletin> for RococoToRococoBulletinCallBuilder {
	fn build_submit_parachain_heads_call(
		at_relay_block: HeaderIdOf<relay_rococo_client::Rococo>,
		parachains: Vec<(ParaId, ParaHash)>,
		parachain_heads_proof: ParaHeadsProof,
	) -> CallOf<relay_polkadot_bulletin_client::PolkadotBulletin> {
		relay_polkadot_bulletin_client::RuntimeCall::BridgePolkadotParachains(
			relay_polkadot_bulletin_client::BridgePolkadotParachainsCall::submit_parachain_heads {
				at_relay_block: (at_relay_block.0, at_relay_block.1),
				parachains,
				parachain_heads_proof,
			},
		)
	}
}

/// Rococo-to-RococoBulletin parachain sync description for the CLI.
pub struct RococoToRococoBulletinCliBridge {}

impl ParachainToRelayHeadersCliBridge for RococoToRococoBulletinCliBridge {
	type SourceRelay = relay_rococo_client::Rococo;
	type ParachainFinality = RococoToRococoBulletin;
	type RelayFinality =
		crate::bridges::rococo_bulletin::rococo_headers_to_rococo_bulletin::RococoFinalityToRococoBulletin;
}

impl CliBridgeBase for RococoToRococoBulletinCliBridge {
	type Source = relay_bridge_hub_rococo_client::BridgeHubRococo;
	type Target = relay_polkadot_bulletin_client::PolkadotBulletin;
}

impl MessagesCliBridge for RococoToRococoBulletinCliBridge {
	type MessagesLane =
		crate::bridges::rococo_bulletin::bridge_hub_rococo_messages_to_rococo_bulletin::BridgeHubRococoMessagesToRococoBulletinMessageLane;

	fn maybe_messages_limits() -> Option<MessagesRelayLimits> {
		// Rococo Bulletin chain is missing the `TransactionPayment` runtime API (as well as the
		// transaction payment pallet itself), so we can't estimate limits using runtime calls.
		// Let's do it here.
		//
		// Folloiung constants are just safe **underestimations**. Normally, we are able to deliver
		// and dispatch thousands of messages in the same transaction.
		Some(MessagesRelayLimits {
			max_messages_in_single_batch: 128,
			max_messages_weight_in_single_batch:
				bp_polkadot_bulletin::PolkadotBulletin::max_extrinsic_weight() / 20,
		})
	}
}
