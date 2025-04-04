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
		bridge_hub_kusama_messages_to_bridge_hub_polkadot::BridgeHubKusamaToBridgeHubPolkadotMessagesCliBridge,
		bridge_hub_polkadot_messages_to_bridge_hub_kusama::BridgeHubPolkadotToBridgeHubKusamaMessagesCliBridge,
	},
	polkadot_bulletin::{
		bridge_hub_polkadot_messages_to_polkadot_bulletin::BridgeHubPolkadotToPolkadotBulletinMessagesCliBridge,
		polkadot_bulletin_messages_to_bridge_hub_polkadot::PolkadotBulletinToBridgeHubPolkadotMessagesCliBridge,
	},
	rococo_bulletin::{
		bridge_hub_rococo_messages_to_rococo_bulletin::BridgeHubRococoToRococoBulletinMessagesCliBridge,
		rococo_bulletin_messages_to_bridge_hub_rococo::RococoBulletinToBridgeHubRococoMessagesCliBridge,
	},
	rococo_westend::{
		asset_hub_rococo_messages_to_asset_hub_westend::AssetHubRococoToAssetHubWestendMessagesCliBridge,
		asset_hub_westend_messages_to_asset_hub_rococo::AssetHubWestendToAssetHubRococoMessagesCliBridge,
		bridge_hub_rococo_messages_to_bridge_hub_westend::BridgeHubRococoToBridgeHubWestendMessagesCliBridge,
		bridge_hub_westend_messages_to_bridge_hub_rococo::BridgeHubWestendToBridgeHubRococoMessagesCliBridge,
	},
};
use substrate_relay_helper::cli::relay_messages::{
	MessagesRelayer, RelayMessagesDeliveryConfirmationParams, RelayMessagesParams,
	RelayMessagesRangeParams,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumString, VariantNames, ValueEnum)]
#[strum(serialize_all = "kebab_case")]
/// Supported full bridges (headers + messages).
pub enum FullBridge {
	BridgeHubRococoToBridgeHubWestend,
	BridgeHubWestendToBridgeHubRococo,
	BridgeHubKusamaToBridgeHubPolkadot,
	BridgeHubPolkadotToBridgeHubKusama,
	PolkadotBulletinToBridgeHubPolkadot,
	BridgeHubPolkadotToPolkadotBulletin,
	RococoBulletinToBridgeHubRococo,
	BridgeHubRococoToRococoBulletin,
	AssetHubRococoToBridgeHubWestend,
	AssetHubWestendToBridgeHubRococo,
}

/// Start messages relayer process.
#[derive(Parser)]
pub struct RelayMessages {
	/// A bridge instance to relay messages for.
	#[arg(value_enum, ignore_case = true)]
	bridge: FullBridge,
	#[command(flatten)]
	params: RelayMessagesParams,
}

/// Relay range of messages.
#[derive(Parser)]
pub struct RelayMessagesRange {
	/// A bridge instance to relay messages for.
	#[arg(value_enum, ignore_case = true)]
	bridge: FullBridge,
	#[command(flatten)]
	params: RelayMessagesRangeParams,
}

/// Relay messages delivery confirmation.
#[derive(Parser)]
pub struct RelayMessagesDeliveryConfirmation {
	/// A bridge instance to relay messages for.
	#[arg(value_enum, ignore_case = true)]
	bridge: FullBridge,
	#[command(flatten)]
	params: RelayMessagesDeliveryConfirmationParams,
}

impl MessagesRelayer for BridgeHubRococoToBridgeHubWestendMessagesCliBridge {}
impl MessagesRelayer for BridgeHubWestendToBridgeHubRococoMessagesCliBridge {}
impl MessagesRelayer for BridgeHubKusamaToBridgeHubPolkadotMessagesCliBridge {}
impl MessagesRelayer for BridgeHubPolkadotToBridgeHubKusamaMessagesCliBridge {}
impl MessagesRelayer for PolkadotBulletinToBridgeHubPolkadotMessagesCliBridge {}
impl MessagesRelayer for BridgeHubPolkadotToPolkadotBulletinMessagesCliBridge {}
impl MessagesRelayer for RococoBulletinToBridgeHubRococoMessagesCliBridge {}
impl MessagesRelayer for BridgeHubRococoToRococoBulletinMessagesCliBridge {}
impl MessagesRelayer for AssetHubRococoToAssetHubWestendMessagesCliBridge {}
impl MessagesRelayer for AssetHubWestendToAssetHubRococoMessagesCliBridge {}

impl RelayMessages {
	/// Run the command.
	pub async fn run(self) -> anyhow::Result<()> {
		match self.bridge {
			FullBridge::BridgeHubRococoToBridgeHubWestend =>
				BridgeHubRococoToBridgeHubWestendMessagesCliBridge::relay_messages(self.params),
			FullBridge::BridgeHubWestendToBridgeHubRococo =>
				BridgeHubWestendToBridgeHubRococoMessagesCliBridge::relay_messages(self.params),
			FullBridge::BridgeHubKusamaToBridgeHubPolkadot =>
				BridgeHubKusamaToBridgeHubPolkadotMessagesCliBridge::relay_messages(self.params),
			FullBridge::BridgeHubPolkadotToBridgeHubKusama =>
				BridgeHubPolkadotToBridgeHubKusamaMessagesCliBridge::relay_messages(self.params),
			FullBridge::PolkadotBulletinToBridgeHubPolkadot =>
				PolkadotBulletinToBridgeHubPolkadotMessagesCliBridge::relay_messages(self.params),
			FullBridge::BridgeHubPolkadotToPolkadotBulletin =>
				BridgeHubPolkadotToPolkadotBulletinMessagesCliBridge::relay_messages(self.params),
			FullBridge::RococoBulletinToBridgeHubRococo =>
				RococoBulletinToBridgeHubRococoMessagesCliBridge::relay_messages(self.params),
			FullBridge::BridgeHubRococoToRococoBulletin =>
				BridgeHubRococoToRococoBulletinMessagesCliBridge::relay_messages(self.params),
			FullBridge::AssetHubRococoToBridgeHubWestend =>
				AssetHubRococoToAssetHubWestendMessagesCliBridge::relay_messages(self.params),
			FullBridge::AssetHubWestendToBridgeHubRococo =>
				AssetHubWestendToAssetHubRococoMessagesCliBridge::relay_messages(self.params),
		}
		.await
	}
}

impl RelayMessagesRange {
	/// Run the command.
	pub async fn run(self) -> anyhow::Result<()> {
		match self.bridge {
			FullBridge::BridgeHubRococoToBridgeHubWestend =>
				BridgeHubRococoToBridgeHubWestendMessagesCliBridge::relay_messages_range(
					self.params,
				),
			FullBridge::BridgeHubWestendToBridgeHubRococo =>
				BridgeHubWestendToBridgeHubRococoMessagesCliBridge::relay_messages_range(
					self.params,
				),
			FullBridge::BridgeHubKusamaToBridgeHubPolkadot =>
				BridgeHubKusamaToBridgeHubPolkadotMessagesCliBridge::relay_messages_range(
					self.params,
				),
			FullBridge::BridgeHubPolkadotToBridgeHubKusama =>
				BridgeHubPolkadotToBridgeHubKusamaMessagesCliBridge::relay_messages_range(
					self.params,
				),
			FullBridge::PolkadotBulletinToBridgeHubPolkadot =>
				PolkadotBulletinToBridgeHubPolkadotMessagesCliBridge::relay_messages_range(
					self.params,
				),
			FullBridge::BridgeHubPolkadotToPolkadotBulletin =>
				BridgeHubPolkadotToPolkadotBulletinMessagesCliBridge::relay_messages_range(
					self.params,
				),
			FullBridge::RococoBulletinToBridgeHubRococo =>
				RococoBulletinToBridgeHubRococoMessagesCliBridge::relay_messages_range(self.params),
			FullBridge::BridgeHubRococoToRococoBulletin =>
				BridgeHubRococoToRococoBulletinMessagesCliBridge::relay_messages_range(self.params),
			FullBridge::AssetHubRococoToBridgeHubWestend =>
				AssetHubRococoToAssetHubWestendMessagesCliBridge::relay_messages_range(self.params),
			FullBridge::AssetHubWestendToBridgeHubRococo =>
				AssetHubWestendToAssetHubRococoMessagesCliBridge::relay_messages_range(self.params),
		}
		.await
	}
}

impl RelayMessagesDeliveryConfirmation {
	/// Run the command.
	pub async fn run(self) -> anyhow::Result<()> {
		match self.bridge {
			FullBridge::BridgeHubRococoToBridgeHubWestend =>
				BridgeHubRococoToBridgeHubWestendMessagesCliBridge::relay_messages_delivery_confirmation(
					self.params,
				),
			FullBridge::BridgeHubWestendToBridgeHubRococo =>
				BridgeHubWestendToBridgeHubRococoMessagesCliBridge::relay_messages_delivery_confirmation(
					self.params,
				),
			FullBridge::BridgeHubKusamaToBridgeHubPolkadot =>
				BridgeHubKusamaToBridgeHubPolkadotMessagesCliBridge::relay_messages_delivery_confirmation(
					self.params,
				),
			FullBridge::BridgeHubPolkadotToBridgeHubKusama =>
				BridgeHubPolkadotToBridgeHubKusamaMessagesCliBridge::relay_messages_delivery_confirmation(
					self.params,
				),
			FullBridge::PolkadotBulletinToBridgeHubPolkadot =>
				PolkadotBulletinToBridgeHubPolkadotMessagesCliBridge::relay_messages_delivery_confirmation(
					self.params,
				),
			FullBridge::BridgeHubPolkadotToPolkadotBulletin =>
				BridgeHubPolkadotToPolkadotBulletinMessagesCliBridge::relay_messages_delivery_confirmation(
					self.params,
				),
			FullBridge::RococoBulletinToBridgeHubRococo =>
				RococoBulletinToBridgeHubRococoMessagesCliBridge::relay_messages_delivery_confirmation(self.params),
			FullBridge::BridgeHubRococoToRococoBulletin =>
				BridgeHubRococoToRococoBulletinMessagesCliBridge::relay_messages_delivery_confirmation(self.params),
			FullBridge::AssetHubRococoToBridgeHubWestend =>
				AssetHubRococoToAssetHubWestendMessagesCliBridge::relay_messages_delivery_confirmation(self.params),
			FullBridge::AssetHubWestendToBridgeHubRococo =>
				AssetHubWestendToAssetHubRococoMessagesCliBridge::relay_messages_delivery_confirmation(self.params),
		}
		.await
	}
}
