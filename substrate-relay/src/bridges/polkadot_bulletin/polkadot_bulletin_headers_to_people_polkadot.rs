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

//! PolkadotBulletin-to-PeoplePolkadot headers sync entrypoint.

use async_trait::async_trait;
use substrate_relay_helper::{
	equivocation::SubstrateEquivocationDetectionPipeline,
	finality::SubstrateFinalitySyncPipeline,
	finality_base::{engine::Grandpa as GrandpaFinalityEngine, SubstrateFinalityPipeline},
};

use substrate_relay_helper::cli::bridge::{
	CliBridgeBase, MessagesCliBridge, RelayToRelayEquivocationDetectionCliBridge,
	RelayToRelayHeadersCliBridge,
};

/// Description of `PolkadotBulletin` -> `PolkadotPeople` finalized headers bridge.
#[derive(Clone, Debug)]
pub struct PolkadotBulletinFinalityToPeoplePolkadot;

substrate_relay_helper::generate_submit_finality_proof_call_builder!(
	PolkadotBulletinFinalityToPeoplePolkadot,
	SubmitFinalityProofCallBuilder,
	// TODO: https://github.com/paritytech/parity-bridges-common/issues/2547 - use BridgePolkadotBulletinGrandpa
	relay_people_polkadot_client::RuntimeCall::BridgePolkadotBulletinGrandpa,
	relay_people_polkadot_client::BridgePolkadotBulletinGrandpaCall::submit_finality_proof
);

substrate_relay_helper::generate_report_equivocation_call_builder!(
	PolkadotBulletinFinalityToPeoplePolkadot,
	ReportEquivocationCallBuilder,
	relay_polkadot_bulletin_client::RuntimeCall::Grandpa,
	relay_polkadot_bulletin_client::GrandpaCall::report_equivocation
);

#[async_trait]
impl SubstrateFinalityPipeline for PolkadotBulletinFinalityToPeoplePolkadot {
	type SourceChain = relay_polkadot_bulletin_client::PolkadotBulletin;
	type TargetChain = relay_people_polkadot_client::PeoplePolkadot;

	type FinalityEngine = GrandpaFinalityEngine<Self::SourceChain>;
}

#[async_trait]
impl SubstrateFinalitySyncPipeline for PolkadotBulletinFinalityToPeoplePolkadot {
	type SubmitFinalityProofCallBuilder = SubmitFinalityProofCallBuilder;
}

#[async_trait]
impl SubstrateEquivocationDetectionPipeline for PolkadotBulletinFinalityToPeoplePolkadot {
	type ReportEquivocationCallBuilder = ReportEquivocationCallBuilder;
}

/// `PolkadotBulletin` to People `Polkadot` bridge definition.
pub struct PolkadotBulletinToPeoplePolkadotCliBridge {}

impl CliBridgeBase for PolkadotBulletinToPeoplePolkadotCliBridge {
	type Source = relay_polkadot_bulletin_client::PolkadotBulletin;
	type Target = relay_people_polkadot_client::PeoplePolkadot;
}

impl RelayToRelayHeadersCliBridge for PolkadotBulletinToPeoplePolkadotCliBridge {
	type Finality = PolkadotBulletinFinalityToPeoplePolkadot;
}

impl RelayToRelayEquivocationDetectionCliBridge for PolkadotBulletinToPeoplePolkadotCliBridge {
	type Equivocation = PolkadotBulletinFinalityToPeoplePolkadot;
}

impl MessagesCliBridge for PolkadotBulletinToPeoplePolkadotCliBridge {
	type MessagesLane = crate::bridges::polkadot_bulletin::polkadot_bulletin_messages_to_people_polkadot::PolkadotBulletinMessagesToPeoplePolkadotMessageLane;
}
