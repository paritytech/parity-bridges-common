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

//! Substrate relay helpers

use relay_utils::metrics::{FloatJsonValueMetric, PrometheusError, StandaloneMetric};

/// Creates standalone token price metric.
pub fn token_price_metric(token_id: &str) -> Result<FloatJsonValueMetric, PrometheusError> {
	FloatJsonValueMetric::new(
		format!("https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies=btc", token_id),
		format!("$.{}.btc", token_id),
		format!("{}_to_base_conversion_rate", token_id.replace("-", "_")),
		format!("Rate used to convert from {} to some BASE tokens", token_id.to_uppercase()),
	)
}

/// Compute conversion rate between two tokens immediately, without spawning any metrics.
pub async fn target_to_source_conversion_rate(
	source_token_id: &str,
	target_token_id: &str,
) -> anyhow::Result<f64> {
	let source_token_metric = token_price_metric(source_token_id)?;
	source_token_metric.update().await;
	let target_token_metric = token_price_metric(target_token_id)?;
	target_token_metric.update().await;

	let source_token_value = *source_token_metric.shared_value_ref().read().await;
	let target_token_value = *target_token_metric.shared_value_ref().read().await;
	// `FloatJsonValueMetric` guarantees that the value is positive && normal, so no additional
	// checks required here
	match (source_token_value, target_token_value) {
		(Some(source_token_value), Some(target_token_value)) =>
			Ok(target_token_value / source_token_value),
		_ => Err(anyhow::format_err!(
			"Failed to compute conversion rate from {} to {}",
			target_token_id,
			source_token_id,
		)),
	}
}
