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

use crate::messages_lane::SubstrateMessageLane;

use codec::Decode;
use frame_system::AccountInfo;
use pallet_balances::AccountData;
use relay_substrate_client::{
	metrics::{
		FixedU128OrOne, FloatStorageValue, FloatStorageValueMetric, StorageProofOverheadMetric,
	},
	AccountIdOf, BalanceOf, Chain, ChainWithBalances, Client, Error as SubstrateError, IndexOf,
};
use relay_utils::metrics::{
	FloatJsonValueMetric, GlobalMetrics, MetricsParams, PrometheusError, StandaloneMetric,
};
use sp_core::storage::StorageData;
use sp_runtime::{FixedPointNumber, FixedU128};
use std::{fmt::Debug, marker::PhantomData};

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
	pub source_to_target_conversion_rate: Option<FloatStorageValueMetric<TC, FixedU128OrOne>>,
	/// Target tokens to source tokens conversion rate metric. This rate is stored by the source
	/// chain.
	pub target_to_source_conversion_rate: Option<FloatStorageValueMetric<SC, FixedU128OrOne>>,
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

/// Create symmetric standalone metrics for the message lane relay loop.
///
/// All metrics returned by this function are exposed by loops that are serving given lane (`P`)
/// and by loops that are serving reverse lane (`P` with swapped `TargetChain` and `SourceChain`).
/// We assume that either conversion rate parameters have values in the storage, or they are
/// initialized with 1:1.
pub fn standalone_metrics<P: SubstrateMessageLane>(
	source_client: Client<P::SourceChain>,
	target_client: Client<P::TargetChain>,
) -> anyhow::Result<StandaloneMessagesMetrics<P::SourceChain, P::TargetChain>> {
	Ok(StandaloneMessagesMetrics {
		global: GlobalMetrics::new()?,
		source_storage_proof_overhead: StorageProofOverheadMetric::new(
			source_client.clone(),
			format!("{}_storage_proof_overhead", P::SourceChain::NAME.to_lowercase()),
			format!("{} storage proof overhead", P::SourceChain::NAME),
		)?,
		target_storage_proof_overhead: StorageProofOverheadMetric::new(
			target_client.clone(),
			format!("{}_storage_proof_overhead", P::TargetChain::NAME.to_lowercase()),
			format!("{} storage proof overhead", P::TargetChain::NAME),
		)?,
		source_to_base_conversion_rate: P::SourceChain::TOKEN_ID
			.map(|source_chain_token_id| {
				crate::helpers::token_price_metric(source_chain_token_id).map(Some)
			})
			.unwrap_or(Ok(None))?,
		target_to_base_conversion_rate: P::TargetChain::TOKEN_ID
			.map(|target_chain_token_id| {
				crate::helpers::token_price_metric(target_chain_token_id).map(Some)
			})
			.unwrap_or(Ok(None))?,
		source_to_target_conversion_rate: P::SOURCE_TO_TARGET_CONVERSION_RATE_PARAMETER_NAME
			.map(bp_runtime::storage_parameter_key)
			.map(|key| {
				FloatStorageValueMetric::new(
					target_client,
					key,
					format!(
						"{}_{}_to_{}_conversion_rate",
						P::TargetChain::NAME,
						P::SourceChain::NAME,
						P::TargetChain::NAME
					),
					format!(
						"{} to {} tokens conversion rate (used by {})",
						P::SourceChain::NAME,
						P::TargetChain::NAME,
						P::TargetChain::NAME
					),
				)
				.map(Some)
			})
			.unwrap_or(Ok(None))?,
		target_to_source_conversion_rate: P::TARGET_TO_SOURCE_CONVERSION_RATE_PARAMETER_NAME
			.map(bp_runtime::storage_parameter_key)
			.map(|key| {
				FloatStorageValueMetric::new(
					source_client,
					key,
					format!(
						"{}_{}_to_{}_conversion_rate",
						P::SourceChain::NAME,
						P::TargetChain::NAME,
						P::SourceChain::NAME
					),
					format!(
						"{} to {} tokens conversion rate (used by {})",
						P::TargetChain::NAME,
						P::SourceChain::NAME,
						P::SourceChain::NAME
					),
				)
				.map(Some)
			})
			.unwrap_or(Ok(None))?,
	})
}

/// Add relay accounts balance metrics.
pub async fn add_relay_balances_metrics<C: ChainWithBalances>(
	client: Client<C>,
	metrics: MetricsParams,
	relay_account_id: Option<AccountIdOf<C>>,
	messages_pallet_owner_account_id: Option<AccountIdOf<C>>,
) -> anyhow::Result<MetricsParams>
where
	BalanceOf<C>: Into<u128> + std::fmt::Debug,
{
	if let Some(relay_account_id) = relay_account_id {
		let relay_account_balance_metric =
			FloatStorageValueMetric::<_, FreeAccountBalance<C>>::new(
				client.clone(),
				C::account_info_storage_key(&relay_account_id),
				format!("at_{}_relay_balance", C::NAME),
				format!("Balance of the relay account at the {}", C::NAME),
			)?;
		relay_account_balance_metric.register_and_spawn(&metrics.registry)?;
	}
	if let Some(messages_pallet_owner_account_id) = messages_pallet_owner_account_id {
		let pallet_owner_account_balance_metric =
			FloatStorageValueMetric::<_, FreeAccountBalance<C>>::new(
				client.clone(),
				C::account_info_storage_key(&messages_pallet_owner_account_id),
				format!("at_{}_messages_pallet_owner_balance", C::NAME),
				format!("Balance of the messages pallet owner at the {}", C::NAME),
			)?;
		pallet_owner_account_balance_metric.register_and_spawn(&metrics.registry)?;
	}
	Ok(metrics)
}

/// Adapter for `FloatStorageValueMetric` to decode account free balance.
#[derive(Clone, Debug)]
struct FreeAccountBalance<C> {
	_phantom: PhantomData<C>,
}

impl<C> FloatStorageValue for FreeAccountBalance<C>
where
	C: Chain,
	BalanceOf<C>: Into<u128>,
{
	type Value = FixedU128;

	fn decode(maybe_raw_value: Option<StorageData>) -> Result<Option<Self::Value>, SubstrateError> {
		maybe_raw_value
			.map(|raw_value| {
				AccountInfo::<IndexOf<C>, AccountData<BalanceOf<C>>>::decode(&mut &raw_value.0[..])
					.map_err(SubstrateError::ResponseParseFailed)
					.map(|account_data| {
						FixedU128::from_inner(
							account_data.data.free.into().saturating_mul(FixedU128::DIV),
						)
					})
			})
			.transpose()
	}
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
