// Copyright 2019-2023 Parity Technologies (UK) Ltd.
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
use strum::{EnumString, VariantNames};

use substrate_relay_helper::cli::detect_equivocations::{
	DetectEquivocationsParams, EquivocationsDetector,
};

/// Start equivocation detection loop.
#[derive(Parser)]
pub struct DetectEquivocations {
	#[arg(value_enum, ignore_case = true)]
	bridge: DetectEquivocationsBridge,
	#[command(flatten)]
	params: DetectEquivocationsParams,
}

#[derive(Clone, Copy, Debug, EnumString, VariantNames, ValueEnum)]
#[strum(serialize_all = "kebab_case")]
/// Equivocations detection bridge.
pub enum DetectEquivocationsBridge {
	KusamaToBridgeHubPolkadot,
	PolkadotToBridgeHubKusama,
	RococoToBridgeHubWestend,
	WestendToBridgeHubRococo,
}

impl EquivocationsDetector for KusamaToBridgeHubPolkadotCliBridge {}
impl EquivocationsDetector for PolkadotToBridgeHubKusamaCliBridge {}
impl EquivocationsDetector for RococoToBridgeHubWestendCliBridge {}
impl EquivocationsDetector for WestendToBridgeHubRococoCliBridge {}

impl DetectEquivocations {
	/// Run the command.
	pub async fn run(self) -> anyhow::Result<()> {
		match self.bridge {
			DetectEquivocationsBridge::KusamaToBridgeHubPolkadot =>
				KusamaToBridgeHubPolkadotCliBridge::start(self.params),
			DetectEquivocationsBridge::PolkadotToBridgeHubKusama =>
				PolkadotToBridgeHubKusamaCliBridge::start(self.params),
			DetectEquivocationsBridge::RococoToBridgeHubWestend =>
				RococoToBridgeHubWestendCliBridge::start(self.params),
			DetectEquivocationsBridge::WestendToBridgeHubRococo =>
				WestendToBridgeHubRococoCliBridge::start(self.params),
		}
		.await
	}
}
