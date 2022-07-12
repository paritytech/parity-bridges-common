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

use async_trait::async_trait;
use bp_polkadot_core::parachains::ParaId;
use parachains_relay::parachains_loop::{ParachainSyncParams, SourceClient, TargetClient};
use relay_utils::metrics::{GlobalMetrics, StandaloneMetric};
use structopt::StructOpt;
use strum::{EnumString, EnumVariantNames, VariantNames};
use substrate_relay_helper::{
	parachains::{source::ParachainsSource, target::ParachainsTarget, ParachainsPipelineAdapter},
	TransactionParams,
};

use crate::cli::{
	bridge::{
		ParachainToRelayHeadersCliBridge, RialtoParachainToMillauCliBridge,
		WestmintToMillauCliBridge,
	},
	chain_schema::*,
	PrometheusParams,
};

/// Start parachain heads relayer process.
#[derive(StructOpt)]
pub struct RelayParachains {
	/// A bridge instance to relay parachains heads for.
	#[structopt(possible_values = RelayParachainsBridge::VARIANTS, case_insensitive = true)]
	bridge: RelayParachainsBridge,
	#[structopt(flatten)]
	source: SourceConnectionParams,
	#[structopt(flatten)]
	target: TargetConnectionParams,
	#[structopt(flatten)]
	target_sign: TargetSigningParams,
	#[structopt(flatten)]
	prometheus_params: PrometheusParams,
}

/// Parachain heads relay bridge.
#[derive(Debug, EnumString, EnumVariantNames)]
#[strum(serialize_all = "kebab_case")]
pub enum RelayParachainsBridge {
	RialtoToMillau,
	WestendToMillau,
}

#[async_trait]
trait ParachainsRelayer: ParachainToRelayHeadersCliBridge
where
	ParachainsSource<Self::ParachainFinality>:
		SourceClient<ParachainsPipelineAdapter<Self::ParachainFinality>>,
	ParachainsTarget<Self::ParachainFinality>:
		TargetClient<ParachainsPipelineAdapter<Self::ParachainFinality>>,
{
	async fn relay_headers(data: RelayParachains) -> anyhow::Result<()> {
		let source_client =
			ConnectionParams::from(data.source).to_client::<Self::SourceRelay>().await?;
		let source_client = ParachainsSource::<Self::ParachainFinality>::new(source_client, None);

		let target_sign = SigningParams::from(data.target_sign);
		let target_transaction_params = TransactionParams {
			signer: target_sign.to_keypair::<Self::Target>()?,
			mortality: target_sign.transactions_mortality,
		};
		let target_client = ConnectionParams::from(data.target).to_client::<Self::Target>().await?;
		let target_client = ParachainsTarget::<Self::ParachainFinality>::new(
			target_client.clone(),
			target_transaction_params,
		);

		let metrics_params: relay_utils::metrics::MetricsParams = data.prometheus_params.into();
		GlobalMetrics::new()?.register_and_spawn(&metrics_params.registry)?;

		parachains_relay::parachains_loop::run(
			source_client,
			target_client,
			ParachainSyncParams {
				parachains: vec![ParaId(2000)],
				stall_timeout: std::time::Duration::from_secs(60),
				strategy: parachains_relay::parachains_loop::ParachainSyncStrategy::Any,
			},
			metrics_params,
			futures::future::pending(),
		)
		.await
		.map_err(|e| anyhow::format_err!("{}", e))
	}
}

impl ParachainsRelayer for RialtoParachainToMillauCliBridge {}

impl ParachainsRelayer for WestmintToMillauCliBridge {}

impl RelayParachains {
	/// Run the command.
	pub async fn run(self) -> anyhow::Result<()> {
		match self.bridge {
			RelayParachainsBridge::RialtoToMillau =>
				RialtoParachainToMillauCliBridge::relay_headers(self),
			RelayParachainsBridge::WestendToMillau =>
				WestmintToMillauCliBridge::relay_headers(self),
		}
		.await
	}
}
