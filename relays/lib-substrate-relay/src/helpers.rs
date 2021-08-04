use relay_utils::metrics::{FloatJsonValueMetric, PrometheusError, Registry};

/// Creates standalone token price metric.
pub fn token_price_metric(
	registry: &Registry,
	prefix: Option<&str>,
	token_id: &str,
) -> Result<FloatJsonValueMetric, PrometheusError> {
	FloatJsonValueMetric::new(
		registry,
		prefix,
		format!(
			"https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies=btc",
			token_id
		),
		format!("$.{}.btc", token_id),
		format!("{}_to_base_conversion_rate", token_id.replace("-", "_")),
		format!(
			"Rate used to convert from {} to some BASE tokens",
			token_id.to_uppercase()
		),
	)
}
