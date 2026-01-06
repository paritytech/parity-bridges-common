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

//! Westend-to-WestendBulletin headers sync entrypoint.

use relay_westend_client::Westend;

use async_trait::async_trait;
use substrate_relay_helper::{
	equivocation::SubstrateEquivocationDetectionPipeline,
	finality::SubstrateFinalitySyncPipeline,
	finality_base::{engine::Grandpa as GrandpaFinalityEngine, SubstrateFinalityPipeline},
};

use substrate_relay_helper::cli::bridge::{
	CliBridgeBase, RelayToRelayEquivocationDetectionCliBridge, RelayToRelayHeadersCliBridge,
};

/// Description of Westend -> `WestendBulletin` finalized headers bridge.
#[derive(Clone, Debug)]
pub struct WestendFinalityToWestendBulletin;

substrate_relay_helper::generate_submit_finality_proof_call_builder!(
	WestendFinalityToWestendBulletin,
	SubmitFinalityProofCallBuilder,
	relay_polkadot_bulletin_client::RuntimeCall::BridgePolkadotGrandpa,
	relay_polkadot_bulletin_client::BridgePolkadotGrandpaCall::submit_finality_proof
);

substrate_relay_helper::generate_report_equivocation_call_builder!(
	WestendFinalityToWestendBulletin,
	ReportEquivocationCallBuilder,
	relay_westend_client::RuntimeCall::Grandpa,
	relay_westend_client::GrandpaCall::report_equivocation
);

#[async_trait]
impl SubstrateFinalityPipeline for WestendFinalityToWestendBulletin {
	type SourceChain = Westend;
	type TargetChain = relay_polkadot_bulletin_client::PolkadotBulletin;

	type FinalityEngine = GrandpaFinalityEngine<Self::SourceChain>;
}

#[async_trait]
impl SubstrateFinalitySyncPipeline for WestendFinalityToWestendBulletin {
	type SubmitFinalityProofCallBuilder = SubmitFinalityProofCallBuilder;
}

#[async_trait]
impl SubstrateEquivocationDetectionPipeline for WestendFinalityToWestendBulletin {
	type ReportEquivocationCallBuilder = ReportEquivocationCallBuilder;
}

/// `Westend` to BridgeHub `WestendBulletin` bridge definition.
pub struct WestendToWestendBulletinCliBridge {}

impl CliBridgeBase for WestendToWestendBulletinCliBridge {
	type Source = Westend;
	type Target = relay_polkadot_bulletin_client::PolkadotBulletin;
}

impl RelayToRelayHeadersCliBridge for WestendToWestendBulletinCliBridge {
	type Finality = WestendFinalityToWestendBulletin;
}

impl RelayToRelayEquivocationDetectionCliBridge for WestendToWestendBulletinCliBridge {
	type Equivocation = WestendFinalityToWestendBulletin;
}
