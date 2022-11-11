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
use sp_core::{storage::StorageKey, Pair};
use structopt::StructOpt;
use strum::{EnumString, EnumVariantNames, VariantNames};

use crate::cli::{
	chain_schema::{SourceConnectionParams, SourceSigningParams},
	CliChain,
};
use relay_polkadot_client::Polkadot;
use relay_statemine_client::Statemine;
use relay_substrate_client::{
	metrics::{FixedU128OrOne, FloatStorageValueMetric},
	AccountIdOf, AccountKeyPairOf, Chain, ChainWithTransactions, Client,
};
use relay_utils::metrics::{FloatJsonValueMetric, StandaloneMetric};
use substrate_relay_helper::conversion_rate_update::{
	run_conversion_rate_update_loop, UpdateConversionRateCallBuilder,
};

/// Start headers relayer process.
#[derive(Debug, StructOpt, PartialEq)]
pub struct ConversionRateUpdate {
	/// A bridge instance to update conversion rate for.
	#[structopt(possible_values = ConversionRateUpdateBridge::VARIANTS, case_insensitive = true)]
	bridge: ConversionRateUpdateBridge,
	#[structopt(flatten)]
	source: SourceConnectionParams,
	#[structopt(flatten)]
	source_sign: SourceSigningParams,
}

/// Update conversion rate bridge.
#[derive(Clone, Copy, Debug, EnumString, EnumVariantNames, PartialEq)]
#[strum(serialize_all = "kebab_case")]
pub enum ConversionRateUpdateBridge {
	StatemineToStatemint,
}

impl ConversionRateUpdateBridge {
	/// Return source chain runtime storage key that under which conversion rate (FixedU128) is
	/// stored.
	pub fn conversion_rate_storage_key_at_source(&self) -> StorageKey {
		match *self {
			// TODO (https://github.com/paritytech/parity-bridges-common/issues/1626): change once pallet is ready
			Self::StatemineToStatemint => StorageKey(vec![42]),
		}
	}
}

/// Helper trait to ease bridge declarations.
#[async_trait]
trait ConversionRateUpdateLoop {
	/// Source chain of the bridge.
	type SourceChain: CliChain + ChainWithTransactions;
	/// Target chain of the bridge.
	type TargetChain: Chain;

	/// Type that crafts the update-conversion-rate call at the source chain.
	type UpdateConversionRateCallBuilder: UpdateConversionRateCallBuilder<Self::SourceChain>;

	/// Run the conversion rate update loop.
	async fn run_loop(data: ConversionRateUpdate) -> anyhow::Result<()>
	where
		Self::SourceChain:
			CliChain<KeyPair = <Self::SourceChain as ChainWithTransactions>::AccountKeyPair>,
		AccountIdOf<Self::SourceChain>: From<<AccountKeyPairOf<Self::SourceChain> as Pair>::Public>,
	{
		let source_client = data.source.into_client::<Self::SourceChain>().await?;
		let source_transactions_mortality = data.source_sign.source_transactions_mortality;
		let source_sign = data.source_sign.to_keypair::<Self::SourceChain>()?;

		let source_transactions_params = substrate_relay_helper::TransactionParams {
			signer: source_sign,
			mortality: source_transactions_mortality,
		};

		let metrics = create_metrics::<_, Self::TargetChain>(data.bridge, source_client.clone())?;
		metrics.spawn();

		run_conversion_rate_update_loop::<
			_,
			Self::TargetChain,
			Self::UpdateConversionRateCallBuilder,
		>(
			source_client,
			source_transactions_params,
			metrics.target_to_source_conversion_rate.shared_value_ref(),
			metrics.source_to_base_conversion_rate.shared_value_ref(),
			metrics.target_to_base_conversion_rate.shared_value_ref(),
			crate::cli::relay_headers_and_messages::CONVERSION_RATE_ALLOWED_DIFFERENCE_RATIO,
		);

		Ok(())
	}
}

/// Statemine -> Statemint conversion rate update loop.
struct StatemineToStatemintLoop;

impl ConversionRateUpdateLoop for StatemineToStatemintLoop {
	type SourceChain = Statemine;
	// this is a bit hacky - we are not going (?) to connect to the Statemint now, so we don't need
	// those primitives and client crates. We're only using the `TargetChain` to get its `TOKEN_ID`.
	// Statemint is the common god parachain of the Polkadot, so let's use Polkadot here
	type TargetChain = Polkadot;
	// TODO (https://github.com/paritytech/parity-bridges-common/issues/1626): change once pallet is ready
	type UpdateConversionRateCallBuilder = ();
}

impl ConversionRateUpdate {
	/// Run the command.
	pub async fn run(self) -> anyhow::Result<()> {
		match self.bridge {
			ConversionRateUpdateBridge::StatemineToStatemint =>
				StatemineToStatemintLoop::run_loop(self).await,
		}
	}
}

/// Conversion rate metrics, used by the loop.
#[derive(Debug)]
pub struct Metrics<SC: Chain> {
	/// Source tokens to base conversion rate metric.
	pub source_to_base_conversion_rate: FloatJsonValueMetric,
	/// Target tokens to base conversion rate metric.
	pub target_to_base_conversion_rate: FloatJsonValueMetric,
	/// Target tokens to source tokens conversion rate metric. This rate is stored by the source
	/// chain.
	pub target_to_source_conversion_rate: FloatStorageValueMetric<SC, FixedU128OrOne>,
}

impl<SC: Chain> Metrics<SC> {
	/// Spawn all metrics.
	fn spawn(&self) {
		self.source_to_base_conversion_rate.clone().spawn();
		self.target_to_base_conversion_rate.clone().spawn();
		self.target_to_source_conversion_rate.clone().spawn();
	}
}

/// Create all metrics required by the loop.
fn create_metrics<SC: Chain, TC: Chain>(
	bridge: ConversionRateUpdateBridge,
	source_client: Client<SC>,
) -> anyhow::Result<Metrics<SC>> {
	Ok(Metrics {
		source_to_base_conversion_rate: substrate_relay_helper::helpers::token_price_metric(
			SC::TOKEN_ID
				.ok_or_else(|| anyhow::format_err!("Missing token id for chain {}", SC::NAME))?,
		)?,
		target_to_base_conversion_rate: substrate_relay_helper::helpers::token_price_metric(
			TC::TOKEN_ID
				.ok_or_else(|| anyhow::format_err!("Missing token id for chain {}", TC::NAME))?,
		)?,
		target_to_source_conversion_rate: FloatStorageValueMetric::new(
			FixedU128OrOne::default(),
			source_client,
			bridge.conversion_rate_storage_key_at_source(),
			format!("{}_{}_to_{}_conversion_rate", SC::NAME, TC::NAME, SC::NAME,),
			format!("{} to {} tokens conversion rate (used by {})", TC::NAME, TC::NAME, SC::NAME,),
		)?,
	})
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::cli::chain_schema::{RuntimeVersionType, SourceRuntimeVersionParams};

	#[test]
	fn register_rialto_parachain() {
		let conversion_rate_update = ConversionRateUpdate::from_iter(vec![
			"conversion-rate-update",
			"statemine-to-statemint",
			"--source-host",
			"127.0.0.1",
			"--source-port",
			"11949",
			"--source-signer",
			"//Alice",
		]);

		assert_eq!(
			conversion_rate_update,
			ConversionRateUpdate {
				bridge: ConversionRateUpdateBridge::StatemineToStatemint,
				source: SourceConnectionParams {
					source_host: "127.0.0.1".into(),
					source_port: 11949,
					source_secure: false,
					source_runtime_version: SourceRuntimeVersionParams {
						source_version_mode: RuntimeVersionType::Bundle,
						source_spec_version: None,
						source_transaction_version: None,
					}
				},
				source_sign: SourceSigningParams {
					source_signer: Some("//Alice".into()),
					source_signer_password: None,
					source_signer_file: None,
					source_signer_password_file: None,
					source_transactions_mortality: None,
				},
			}
		);
	}
}
