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

//! Westend-to-WestendBulletin parachains sync entrypoint.

use relay_bridge_hub_westend_client::BridgeHubWestend;
use relay_westend_client::Westend;

use bp_polkadot_core::parachains::{ParaHash, ParaHeadsProof, ParaId};
use bp_runtime::Chain;
use relay_substrate_client::{CallOf, HeaderIdOf};
use substrate_relay_helper::{
	cli::bridge::{CliBridgeBase, MessagesCliBridge, ParachainToRelayHeadersCliBridge},
	messages::MessagesRelayLimits,
	parachains::{SubmitParachainHeadsCallBuilder, SubstrateParachainsPipeline},
};

/// Westend-to-WestendBulletin parachain sync description.
#[derive(Clone, Debug)]
pub struct WestendToWestendBulletin;

impl SubstrateParachainsPipeline for WestendToWestendBulletin {
	type SourceParachain = BridgeHubWestend;
	type SourceRelayChain = Westend;
	type TargetChain = relay_polkadot_bulletin_client::PolkadotBulletin;

	type SubmitParachainHeadsCallBuilder = WestendToWestendBulletinCallBuilder;
}

pub struct WestendToWestendBulletinCallBuilder;
impl SubmitParachainHeadsCallBuilder<WestendToWestendBulletin> for WestendToWestendBulletinCallBuilder {
	fn build_submit_parachain_heads_call(
		at_relay_block: HeaderIdOf<relay_westend_client::Westend>,
		parachains: Vec<(ParaId, ParaHash)>,
		parachain_heads_proof: ParaHeadsProof,
		_is_free_execution_expected: bool,
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

/// Westend-to-WestendBulletin parachain sync description for the CLI.
pub struct WestendToWestendBulletinCliBridge {}

impl ParachainToRelayHeadersCliBridge for WestendToWestendBulletinCliBridge {
	type SourceRelay = Westend;
	type ParachainFinality = WestendToWestendBulletin;
	type RelayFinality =
		crate::bridges::westend_bulletin::westend_headers_to_westend_bulletin::WestendFinalityToWestendBulletin;
}

impl CliBridgeBase for WestendToWestendBulletinCliBridge {
	type Source = BridgeHubWestend;
	type Target = relay_polkadot_bulletin_client::PolkadotBulletin;
}

impl MessagesCliBridge for WestendToWestendBulletinCliBridge {
	type MessagesLane =
		crate::bridges::westend_bulletin::bridge_hub_westend_messages_to_westend_bulletin::BridgeHubWestendMessagesToWestendBulletinMessageLane;

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
