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

use clap::{Parser, ValueEnum};
use strum::{EnumString, VariantNames};

use crate::bridges::{
	kusama_polkadot::{
		kusama_headers_to_bridge_hub_polkadot::KusamaToBridgeHubPolkadotCliBridge,
		polkadot_headers_to_bridge_hub_kusama::PolkadotToBridgeHubKusamaCliBridge,
	},
	polkadot_bulletin::{
		polkadot_bulletin_headers_to_bridge_hub_polkadot::PolkadotBulletinToBridgeHubPolkadotCliBridge,
		polkadot_headers_to_polkadot_bulletin::PolkadotToPolkadotBulletinCliBridge,
	},
	rococo_bulletin::{
		rococo_bulletin_headers_to_bridge_hub_rococo::RococoBulletinToBridgeHubRococoCliBridge,
		rococo_headers_to_rococo_bulletin::RococoToRococoBulletinCliBridge,
	},
	rococo_westend::{
		rococo_headers_to_bridge_hub_westend::RococoToBridgeHubWestendCliBridge,
		westend_headers_to_bridge_hub_rococo::WestendToBridgeHubRococoCliBridge,
	},
	westend_bulletin::{
		westend_bulletin_headers_to_bridge_hub_westend::WestendBulletinToBridgeHubWestendCliBridge,
		westend_headers_to_westend_bulletin::WestendToWestendBulletinCliBridge,
	},
};

use substrate_relay_helper::cli::relay_headers::{
	HeadersRelayer, RelayHeaderParams, RelayHeadersParams,
};

/// Start headers relayer process.
#[derive(Parser)]
pub struct RelayHeaders {
	/// A bridge instance to relay headers for.
	#[arg(value_enum, ignore_case = true)]
	bridge: RelayHeadersBridge,
	#[command(flatten)]
	params: RelayHeadersParams,
}

/// Relay single header.
#[derive(Parser)]
pub struct RelayHeader {
	/// A bridge instance to relay headers for.
	#[arg(value_enum, ignore_case = true)]
	bridge: RelayHeadersBridge,
	#[command(flatten)]
	params: RelayHeaderParams,
}

#[derive(Clone, Copy, Debug, EnumString, VariantNames, ValueEnum)]
#[strum(serialize_all = "kebab_case")]
/// Headers relay bridge.
pub enum RelayHeadersBridge {
	RococoToBridgeHubWestend,
	WestendToBridgeHubRococo,
	KusamaToBridgeHubPolkadot,
	PolkadotToBridgeHubKusama,
	PolkadotToPolkadotBulletin,
	PolkadotBulletinToBridgeHubPolkadot,
	RococoToRococoBulletin,
	RococoBulletinToBridgeHubRococo,
	WestendToWestendBulletin,
	WestendBulletinToBridgeHubWestend,
}

impl HeadersRelayer for RococoToBridgeHubWestendCliBridge {}
impl HeadersRelayer for WestendToBridgeHubRococoCliBridge {}
impl HeadersRelayer for KusamaToBridgeHubPolkadotCliBridge {}
impl HeadersRelayer for PolkadotToBridgeHubKusamaCliBridge {}
impl HeadersRelayer for PolkadotToPolkadotBulletinCliBridge {}
impl HeadersRelayer for PolkadotBulletinToBridgeHubPolkadotCliBridge {}
impl HeadersRelayer for RococoToRococoBulletinCliBridge {}
impl HeadersRelayer for RococoBulletinToBridgeHubRococoCliBridge {}
impl HeadersRelayer for WestendToWestendBulletinCliBridge {}
impl HeadersRelayer for WestendBulletinToBridgeHubWestendCliBridge {}

impl RelayHeaders {
	/// Run the command.
	pub async fn run(self) -> anyhow::Result<()> {
		match self.bridge {
			RelayHeadersBridge::RococoToBridgeHubWestend =>
				RococoToBridgeHubWestendCliBridge::relay_headers(self.params),
			RelayHeadersBridge::WestendToBridgeHubRococo =>
				WestendToBridgeHubRococoCliBridge::relay_headers(self.params),
			RelayHeadersBridge::KusamaToBridgeHubPolkadot =>
				KusamaToBridgeHubPolkadotCliBridge::relay_headers(self.params),
			RelayHeadersBridge::PolkadotToBridgeHubKusama =>
				PolkadotToBridgeHubKusamaCliBridge::relay_headers(self.params),
			RelayHeadersBridge::PolkadotToPolkadotBulletin =>
				PolkadotToPolkadotBulletinCliBridge::relay_headers(self.params),
			RelayHeadersBridge::PolkadotBulletinToBridgeHubPolkadot =>
				PolkadotBulletinToBridgeHubPolkadotCliBridge::relay_headers(self.params),
			RelayHeadersBridge::RococoToRococoBulletin =>
				RococoToRococoBulletinCliBridge::relay_headers(self.params),
			RelayHeadersBridge::RococoBulletinToBridgeHubRococo =>
				RococoBulletinToBridgeHubRococoCliBridge::relay_headers(self.params),
			RelayHeadersBridge::WestendToWestendBulletin =>
				WestendToWestendBulletinCliBridge::relay_headers(self.params),
			RelayHeadersBridge::WestendBulletinToBridgeHubWestend =>
				WestendBulletinToBridgeHubWestendCliBridge::relay_headers(self.params),
		}
		.await
	}
}

impl RelayHeader {
	/// Run the command.
	pub async fn run(self) -> anyhow::Result<()> {
		match self.bridge {
			RelayHeadersBridge::RococoToBridgeHubWestend =>
				RococoToBridgeHubWestendCliBridge::relay_header(self.params),
			RelayHeadersBridge::WestendToBridgeHubRococo =>
				WestendToBridgeHubRococoCliBridge::relay_header(self.params),
			RelayHeadersBridge::KusamaToBridgeHubPolkadot =>
				KusamaToBridgeHubPolkadotCliBridge::relay_header(self.params),
			RelayHeadersBridge::PolkadotToBridgeHubKusama =>
				PolkadotToBridgeHubKusamaCliBridge::relay_header(self.params),
			RelayHeadersBridge::PolkadotToPolkadotBulletin =>
				PolkadotToPolkadotBulletinCliBridge::relay_header(self.params),
			RelayHeadersBridge::PolkadotBulletinToBridgeHubPolkadot =>
				PolkadotBulletinToBridgeHubPolkadotCliBridge::relay_header(self.params),
			RelayHeadersBridge::RococoToRococoBulletin =>
				RococoToRococoBulletinCliBridge::relay_header(self.params),
			RelayHeadersBridge::RococoBulletinToBridgeHubRococo =>
				RococoBulletinToBridgeHubRococoCliBridge::relay_header(self.params),
			RelayHeadersBridge::WestendToWestendBulletin =>
				WestendToWestendBulletinCliBridge::relay_header(self.params),
			RelayHeadersBridge::WestendBulletinToBridgeHubWestend =>
				WestendBulletinToBridgeHubWestendCliBridge::relay_header(self.params),
		}
		.await
	}
}
