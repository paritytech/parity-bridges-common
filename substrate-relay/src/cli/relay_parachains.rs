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
		asset_hub_kusama_parachains_to_bridge_hub_polkadot::AssetHubKusamaToBridgeHubPolkadotCliBridge,
		asset_hub_polkadot_parachains_to_bridge_hub_kusama::AssetHubPolkadotToBridgeHubKusamaCliBridge,
		bridge_hub_kusama_parachains_to_bridge_hub_polkadot::BridgeHubKusamaToBridgeHubPolkadotCliBridge,
		bridge_hub_polkadot_parachains_to_bridge_hub_kusama::BridgeHubPolkadotToBridgeHubKusamaCliBridge,
	},
	polkadot_bulletin::polkadot_parachains_to_polkadot_bulletin::PolkadotToPolkadotBulletinCliBridge,
	rococo_bulletin::rococo_parachains_to_rococo_bulletin::RococoToRococoBulletinCliBridge,
	rococo_westend::{
		asset_hub_rococo_parachains_to_bridge_hub_westend::AssetHubRococoToBridgeHubWestendCliBridge,
		asset_hub_westend_parachains_to_bridge_hub_rococo::AssetHubWestendToBridgeHubRococoCliBridge,
		bridge_hub_rococo_parachains_to_bridge_hub_westend::BridgeHubRococoToBridgeHubWestendCliBridge,
		bridge_hub_westend_parachains_to_bridge_hub_rococo::BridgeHubWestendToBridgeHubRococoCliBridge,
	},
};
use structopt::StructOpt;
use strum::{EnumString, VariantNames};
use substrate_relay_helper::cli::relay_parachains::{
	ParachainsRelayer, RelayParachainHeadParams, RelayParachainsParams,
};

/// Start parachain heads relayer process.
#[derive(StructOpt)]
pub struct RelayParachains {
	/// A bridge instance to relay parachains heads for.
	#[structopt(possible_values = RelayParachainsBridge::VARIANTS, case_insensitive = true)]
	bridge: RelayParachainsBridge,
	#[structopt(flatten)]
	params: RelayParachainsParams,
}

/// Relay single parachain head.
#[derive(StructOpt)]
pub struct RelayParachainHead {
	/// A bridge instance to relay parachains heads for.
	#[structopt(possible_values = RelayParachainsBridge::VARIANTS, case_insensitive = true)]
	bridge: RelayParachainsBridge,
	#[structopt(flatten)]
	params: RelayParachainHeadParams,
}

/// Parachain heads relay bridge.
#[derive(Debug, EnumString, VariantNames)]
#[strum(serialize_all = "kebab_case")]
pub enum RelayParachainsBridge {
	BridgeHubKusamaToBridgeHubPolkadot,
	BridgeHubPolkadotToBridgeHubKusama,
	AssetHubKusamaToBridgeHubPolkadot,
	AssetHubPolkadotToBridgeHubKusama,
	PolkadotToPolkadotBulletin,
	RococoToRococoBulletin,
	BridgeHubRococoToBridgeHubWestend,
	BridgeHubWestendToBridgeHubRococo,
	AssetHubRococoToBridgeHubWestend,
	AssetHubWestendToBridgeHubRococo,
}

impl ParachainsRelayer for BridgeHubRococoToBridgeHubWestendCliBridge {}
impl ParachainsRelayer for BridgeHubWestendToBridgeHubRococoCliBridge {}
impl ParachainsRelayer for AssetHubRococoToBridgeHubWestendCliBridge {}
impl ParachainsRelayer for AssetHubWestendToBridgeHubRococoCliBridge {}
impl ParachainsRelayer for BridgeHubKusamaToBridgeHubPolkadotCliBridge {}
impl ParachainsRelayer for BridgeHubPolkadotToBridgeHubKusamaCliBridge {}
impl ParachainsRelayer for AssetHubKusamaToBridgeHubPolkadotCliBridge {}
impl ParachainsRelayer for AssetHubPolkadotToBridgeHubKusamaCliBridge {}
impl ParachainsRelayer for PolkadotToPolkadotBulletinCliBridge {}
impl ParachainsRelayer for RococoToRococoBulletinCliBridge {}

impl RelayParachains {
	/// Run the command.
	pub async fn run(self) -> anyhow::Result<()> {
		match self.bridge {
			RelayParachainsBridge::BridgeHubRococoToBridgeHubWestend =>
				BridgeHubRococoToBridgeHubWestendCliBridge::relay_parachains(self.params),
			RelayParachainsBridge::BridgeHubWestendToBridgeHubRococo =>
				BridgeHubWestendToBridgeHubRococoCliBridge::relay_parachains(self.params),
			RelayParachainsBridge::AssetHubRococoToBridgeHubWestend =>
				AssetHubRococoToBridgeHubWestendCliBridge::relay_parachains(self.params),
			RelayParachainsBridge::AssetHubWestendToBridgeHubRococo =>
				AssetHubWestendToBridgeHubRococoCliBridge::relay_parachains(self.params),
			RelayParachainsBridge::BridgeHubKusamaToBridgeHubPolkadot =>
				BridgeHubKusamaToBridgeHubPolkadotCliBridge::relay_parachains(self.params),
			RelayParachainsBridge::BridgeHubPolkadotToBridgeHubKusama =>
				BridgeHubPolkadotToBridgeHubKusamaCliBridge::relay_parachains(self.params),
			RelayParachainsBridge::AssetHubKusamaToBridgeHubPolkadot =>
				AssetHubKusamaToBridgeHubPolkadotCliBridge::relay_parachains(self.params),
			RelayParachainsBridge::AssetHubPolkadotToBridgeHubKusama =>
				AssetHubPolkadotToBridgeHubKusamaCliBridge::relay_parachains(self.params),
			RelayParachainsBridge::PolkadotToPolkadotBulletin =>
				PolkadotToPolkadotBulletinCliBridge::relay_parachains(self.params),
			RelayParachainsBridge::RococoToRococoBulletin =>
				RococoToRococoBulletinCliBridge::relay_parachains(self.params),
		}
		.await
	}
}

impl RelayParachainHead {
	/// Run the command.
	pub async fn run(self) -> anyhow::Result<()> {
		match self.bridge {
			RelayParachainsBridge::BridgeHubRococoToBridgeHubWestend =>
				BridgeHubRococoToBridgeHubWestendCliBridge::relay_parachain_head(self.params),
			RelayParachainsBridge::BridgeHubWestendToBridgeHubRococo =>
				BridgeHubWestendToBridgeHubRococoCliBridge::relay_parachain_head(self.params),
			RelayParachainsBridge::AssetHubRococoToBridgeHubWestend =>
				AssetHubRococoToBridgeHubWestendCliBridge::relay_parachain_head(self.params),
			RelayParachainsBridge::AssetHubWestendToBridgeHubRococo =>
				AssetHubWestendToBridgeHubRococoCliBridge::relay_parachain_head(self.params),
			RelayParachainsBridge::BridgeHubKusamaToBridgeHubPolkadot =>
				BridgeHubKusamaToBridgeHubPolkadotCliBridge::relay_parachain_head(self.params),
			RelayParachainsBridge::BridgeHubPolkadotToBridgeHubKusama =>
				BridgeHubPolkadotToBridgeHubKusamaCliBridge::relay_parachain_head(self.params),
			RelayParachainsBridge::AssetHubKusamaToBridgeHubPolkadot =>
				AssetHubKusamaToBridgeHubPolkadotCliBridge::relay_parachain_head(self.params),
			RelayParachainsBridge::AssetHubPolkadotToBridgeHubKusama =>
				AssetHubPolkadotToBridgeHubKusamaCliBridge::relay_parachain_head(self.params),
			RelayParachainsBridge::PolkadotToPolkadotBulletin =>
				PolkadotToPolkadotBulletinCliBridge::relay_parachain_head(self.params),
			RelayParachainsBridge::RococoToRococoBulletin =>
				RococoToRococoBulletinCliBridge::relay_parachain_head(self.params),
		}
		.await
	}
}
