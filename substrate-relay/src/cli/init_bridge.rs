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

use crate::bridges::{
	kusama_polkadot::{
		kusama_headers_to_bridge_hub_polkadot::KusamaToBridgeHubPolkadotCliBridge,
		polkadot_headers_to_bridge_hub_kusama::PolkadotToBridgeHubKusamaCliBridge,
	},
	rococo_westend::{
		rococo_headers_to_bridge_hub_westend::RococoToBridgeHubWestendCliBridge,
		westend_headers_to_bridge_hub_rococo::WestendToBridgeHubRococoCliBridge,
	},
};
use clap::{Parser, ValueEnum};
use relay_substrate_client::Chain;
use strum::{EnumString, VariantNames};
use substrate_relay_helper::{
	cli::init_bridge::{BridgeInitializer, InitBridgeParams},
	finality_base::engine::{Engine, Grandpa as GrandpaFinalityEngine},
};

impl BridgeInitializer for RococoToBridgeHubWestendCliBridge {
	type Engine = GrandpaFinalityEngine<Self::Source>;

	fn encode_init_bridge(
		init_data: <Self::Engine as Engine<Self::Source>>::InitializationData,
	) -> <Self::Target as Chain>::Call {
		relay_bridge_hub_westend_client::RuntimeCall::BridgeRococoGrandpa(
			relay_bridge_hub_westend_client::BridgeGrandpaCall::initialize { init_data },
		)
	}
}

impl BridgeInitializer for WestendToBridgeHubRococoCliBridge {
	type Engine = GrandpaFinalityEngine<Self::Source>;

	fn encode_init_bridge(
		init_data: <Self::Engine as Engine<Self::Source>>::InitializationData,
	) -> <Self::Target as Chain>::Call {
		relay_bridge_hub_rococo_client::RuntimeCall::BridgeWestendGrandpa(
			relay_bridge_hub_rococo_client::BridgeGrandpaCall::initialize { init_data },
		)
	}
}

impl BridgeInitializer for KusamaToBridgeHubPolkadotCliBridge {
	type Engine = GrandpaFinalityEngine<Self::Source>;

	fn encode_init_bridge(
		init_data: <Self::Engine as Engine<Self::Source>>::InitializationData,
	) -> <Self::Target as Chain>::Call {
		relay_bridge_hub_polkadot_client::RuntimeCall::BridgeKusamaGrandpa(
			relay_bridge_hub_polkadot_client::BridgeKusamaGrandpaCall::initialize { init_data },
		)
	}
}

impl BridgeInitializer for PolkadotToBridgeHubKusamaCliBridge {
	type Engine = GrandpaFinalityEngine<Self::Source>;

	fn encode_init_bridge(
		init_data: <Self::Engine as Engine<Self::Source>>::InitializationData,
	) -> <Self::Target as Chain>::Call {
		relay_bridge_hub_kusama_client::RuntimeCall::BridgePolkadotGrandpa(
			relay_bridge_hub_kusama_client::BridgeGrandpaCall::initialize { init_data },
		)
	}
}

/// Initialize bridge pallet.
#[derive(Parser)]
pub struct InitBridge {
	/// A bridge instance to initialize.
	#[arg(value_enum, ignore_case = true)]
	bridge: InitBridgeName,
	#[command(flatten)]
	params: InitBridgeParams,
}

#[derive(Clone, Copy, Debug, EnumString, VariantNames, ValueEnum)]
#[strum(serialize_all = "kebab_case")]
/// Bridge to initialize.
pub enum InitBridgeName {
	KusamaToBridgeHubPolkadot,
	PolkadotToBridgeHubKusama,
	RococoToBridgeHubWestend,
	WestendToBridgeHubRococo,
}

impl InitBridge {
	/// Run the command.
	pub async fn run(self) -> anyhow::Result<()> {
		match self.bridge {
			InitBridgeName::KusamaToBridgeHubPolkadot =>
				KusamaToBridgeHubPolkadotCliBridge::init_bridge(self.params),
			InitBridgeName::PolkadotToBridgeHubKusama =>
				PolkadotToBridgeHubKusamaCliBridge::init_bridge(self.params),
			InitBridgeName::RococoToBridgeHubWestend =>
				RococoToBridgeHubWestendCliBridge::init_bridge(self.params),
			InitBridgeName::WestendToBridgeHubRococo =>
				WestendToBridgeHubRococoCliBridge::init_bridge(self.params),
		}
		.await
	}
}
