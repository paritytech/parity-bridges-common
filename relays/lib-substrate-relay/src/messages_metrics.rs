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

//! Tools for supporting message lanes between two Substrate-based chains.

use relay_substrate_client::{
	metrics::{FloatStorageValueMetric, StorageProofOverheadMetric},
	Chain, Client,
};
use relay_utils::{
	metrics::{
		FloatJsonValueMetric, GlobalMetrics, MetricsParams, PrometheusError, StandaloneMetric,
	},
};
use sp_core::{storage::StorageKey};
use sp_runtime::FixedU128;
use std::{fmt::Debug};

/// Shared references to the standalone metrics of the message lane relay loop.
#[derive(Debug, Clone)]
pub struct StandaloneMessagesMetrics<SC: Chain, TC: Chain> {
	/// Global metrics.
	pub global: GlobalMetrics,
	/// Storage chain proof overhead metric.
	pub source_storage_proof_overhead: StorageProofOverheadMetric<SC>,
	/// Target chain proof overhead metric.
	pub target_storage_proof_overhead: StorageProofOverheadMetric<TC>,
	/// Source tokens to base conversion rate metric.
	pub source_to_base_conversion_rate: Option<FloatJsonValueMetric>,
	/// Target tokens to base conversion rate metric.
	pub target_to_base_conversion_rate: Option<FloatJsonValueMetric>,
	/// Source tokens to target tokens conversion rate metric. This rate is stored by the target
	/// chain.
	pub source_to_target_conversion_rate:
		Option<FloatStorageValueMetric<TC, sp_runtime::FixedU128>>,
	/// Target tokens to source tokens conversion rate metric. This rate is stored by the source
	/// chain.
	pub target_to_source_conversion_rate:
		Option<FloatStorageValueMetric<SC, sp_runtime::FixedU128>>,
}

impl<SC: Chain, TC: Chain> StandaloneMessagesMetrics<SC, TC> {
	/// Swap source and target sides.
	pub fn reverse(self) -> StandaloneMessagesMetrics<TC, SC> {
		StandaloneMessagesMetrics {
			global: self.global,
			source_storage_proof_overhead: self.target_storage_proof_overhead,
			target_storage_proof_overhead: self.source_storage_proof_overhead,
			source_to_base_conversion_rate: self.target_to_base_conversion_rate,
			target_to_base_conversion_rate: self.source_to_base_conversion_rate,
			source_to_target_conversion_rate: self.target_to_source_conversion_rate,
			target_to_source_conversion_rate: self.source_to_target_conversion_rate,
		}
	}

	/// Register all metrics in the registry.
	pub fn register_and_spawn(
		self,
		metrics: MetricsParams,
	) -> Result<MetricsParams, PrometheusError> {
		self.global.register_and_spawn(&metrics.registry)?;
		self.source_storage_proof_overhead.register_and_spawn(&metrics.registry)?;
		self.target_storage_proof_overhead.register_and_spawn(&metrics.registry)?;
		if let Some(m) = self.source_to_base_conversion_rate {
			m.register_and_spawn(&metrics.registry)?;
		}
		if let Some(m) = self.target_to_base_conversion_rate {
			m.register_and_spawn(&metrics.registry)?;
		}
		if let Some(m) = self.target_to_source_conversion_rate {
			m.register_and_spawn(&metrics.registry)?;
		}
		Ok(metrics)
	}

	/// Return conversion rate from target to source tokens.
	pub async fn target_to_source_conversion_rate(&self) -> Option<f64> {
		Self::compute_target_to_source_conversion_rate(
			*self.target_to_base_conversion_rate.as_ref()?.shared_value_ref().read().await,
			*self.source_to_base_conversion_rate.as_ref()?.shared_value_ref().read().await,
		)
	}

	/// Return conversion rate from target to source tokens, given conversion rates from
	/// target/source tokens to some base token.
	fn compute_target_to_source_conversion_rate(
		target_to_base_conversion_rate: Option<f64>,
		source_to_base_conversion_rate: Option<f64>,
	) -> Option<f64> {
		Some(source_to_base_conversion_rate? / target_to_base_conversion_rate?)
	}
}

/// Create standalone metrics for the message lane relay loop.
///
/// All metrics returned by this function are exposed by loops that are serving given lane (`P`)
/// and by loops that are serving reverse lane (`P` with swapped `TargetChain` and `SourceChain`).
pub fn standalone_metrics<SC: Chain, TC: Chain>(
	source_client: Client<SC>,
	target_client: Client<TC>,
	source_chain_token_id: Option<&str>,
	target_chain_token_id: Option<&str>,
	source_to_target_conversion_rate_params: Option<(StorageKey, FixedU128)>,
	target_to_source_conversion_rate_params: Option<(StorageKey, FixedU128)>,
) -> anyhow::Result<StandaloneMessagesMetrics<SC, TC>> {
	Ok(StandaloneMessagesMetrics {
		global: GlobalMetrics::new()?,
		source_storage_proof_overhead: StorageProofOverheadMetric::new(
			source_client.clone(),
			format!("{}_storage_proof_overhead", SC::NAME.to_lowercase()),
			format!("{} storage proof overhead", SC::NAME),
		)?,
		target_storage_proof_overhead: StorageProofOverheadMetric::new(
			target_client.clone(),
			format!("{}_storage_proof_overhead", TC::NAME.to_lowercase()),
			format!("{} storage proof overhead", TC::NAME),
		)?,
		source_to_base_conversion_rate: source_chain_token_id
			.map(|source_chain_token_id| {
				crate::helpers::token_price_metric(source_chain_token_id).map(Some)
			})
			.unwrap_or(Ok(None))?,
		target_to_base_conversion_rate: target_chain_token_id
			.map(|target_chain_token_id| {
				crate::helpers::token_price_metric(target_chain_token_id).map(Some)
			})
			.unwrap_or(Ok(None))?,
		source_to_target_conversion_rate: source_to_target_conversion_rate_params
			.map(|(key, rate)| {
				FloatStorageValueMetric::<_, sp_runtime::FixedU128>::new(
					target_client,
					key,
					Some(rate),
					format!("{}_{}_to_{}_conversion_rate", TC::NAME, SC::NAME, TC::NAME),
					format!(
						"{} to {} tokens conversion rate (used by {})",
						SC::NAME,
						TC::NAME,
						TC::NAME
					),
				)
				.map(Some)
			})
			.unwrap_or(Ok(None))?,
		target_to_source_conversion_rate: target_to_source_conversion_rate_params
			.map(|(key, rate)| {
				FloatStorageValueMetric::<_, sp_runtime::FixedU128>::new(
					source_client,
					key,
					Some(rate),
					format!("{}_{}_to_{}_conversion_rate", SC::NAME, TC::NAME, SC::NAME),
					format!(
						"{} to {} tokens conversion rate (used by {})",
						TC::NAME,
						SC::NAME,
						SC::NAME
					),
				)
				.map(Some)
			})
			.unwrap_or(Ok(None))?,
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[async_std::test]
	async fn target_to_source_conversion_rate_works() {
		assert_eq!(
			StandaloneMessagesMetrics::<relay_rococo_client::Rococo, relay_wococo_client::Wococo>::compute_target_to_source_conversion_rate(Some(183.15), Some(12.32)),
			Some(12.32 / 183.15),
		);
	}
}
