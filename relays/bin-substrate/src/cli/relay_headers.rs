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

use crate::cli::{
	GatewayIdParams, PrometheusParams, SourceConnectionParams, TargetConnectionParams, TargetSigningParams,
};
use crate::finality_pipeline::SubstrateFinalitySyncPipeline;
use structopt::{clap::arg_enum, StructOpt};

/// Start headers relayer process.
#[derive(StructOpt)]
pub struct RelayHeaders {
	/// A bridge instance to relay headers for.
	#[structopt(possible_values = &RelayHeadersBridge::variants(), case_insensitive = true)]
	bridge: RelayHeadersBridge,
	#[structopt(long, default_value = "gate")]
	gateway_id: GatewayIdParams,
	#[structopt(flatten)]
	source: SourceConnectionParams,
	#[structopt(flatten)]
	target: TargetConnectionParams,
	#[structopt(flatten)]
	target_sign: TargetSigningParams,
	#[structopt(flatten)]
	prometheus_params: PrometheusParams,
}

// TODO [#851] Use kebab-case.
arg_enum! {
	#[derive(Debug)]
	/// Headers relay bridge.
	pub enum RelayHeadersBridge {
		MillauToRialto,
		RialtoToMillau,
		WestendToMillau,
		RococoToWococo,
		WococoToRococo,
		CircuitToGateway,
		GatewayToCircuit,
	}
}

macro_rules! select_bridge {
	($bridge: expr, $generic: tt) => {
		match $bridge {
			RelayHeadersBridge::MillauToRialto => {
				type Source = relay_millau_client::Millau;
				type Target = relay_rialto_client::Rialto;
				type Finality = crate::chains::millau_headers_to_rialto::MillauFinalityToRialto;

				$generic
			}
			RelayHeadersBridge::RialtoToMillau => {
				type Source = relay_rialto_client::Rialto;
				type Target = relay_millau_client::Millau;
				type Finality = crate::chains::rialto_headers_to_millau::RialtoFinalityToMillau;

				$generic
			}
			RelayHeadersBridge::WestendToMillau => {
				type Source = relay_westend_client::Westend;
				type Target = relay_millau_client::Millau;
				type Finality = crate::chains::westend_headers_to_millau::WestendFinalityToMillau;

				$generic
			}
			RelayHeadersBridge::RococoToWococo => {
				type Source = relay_rococo_client::Rococo;
				type Target = relay_wococo_client::Wococo;
				type Finality = crate::chains::rococo_headers_to_wococo::RococoFinalityToWococo;

				$generic
			}
			RelayHeadersBridge::WococoToRococo => {
				type Source = relay_wococo_client::Wococo;
				type Target = relay_rococo_client::Rococo;
				type Finality = crate::chains::wococo_headers_to_rococo::WococoFinalityToRococo;

				$generic
			}
			RelayHeadersBridge::CircuitToGateway => {
				type Source = relay_circuit_client::Circuit;
				type Target = relay_gateway_client::Gateway;
				type Finality = crate::chains::circuit_headers_to_gateway::CircuitFinalityToGateway;

				$generic
			}
			RelayHeadersBridge::GatewayToCircuit => {
				// ToDo: Sync Up with XDNS here to find a best way of connecting given chain
				type Source = relay_westend_client::Westend;
				// type Source = relay_gateway_client::Gateway;
				type Target = relay_circuit_client::Circuit;
				type Finality = crate::chains::gateway_headers_to_circuit::GatewayFinalityToCircuit;

				$generic
			}
		}
	};
}

impl RelayHeaders {
	/// Run the command.
	pub async fn run(self) -> anyhow::Result<()> {
		select_bridge!(self.bridge, {
			let source_client = self.source.to_client::<Source>().await?;
			let target_client = self.target.to_client::<Target>().await?;
			let target_sign = self.target_sign.to_keypair::<Target>()?;
			let metrics_params = Finality::customize_metrics(self.prometheus_params.into())?;
			Finality::start_relay_guards(&target_client);

			crate::finality_pipeline::run(
				Finality::new(target_client.clone(), target_sign, self.gateway_id.0),
				source_client,
				target_client,
				false,
				metrics_params,
			)
			.await
		})
	}
}
