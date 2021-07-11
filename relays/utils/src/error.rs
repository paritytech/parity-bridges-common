use std::net::AddrParseError;
use thiserror::Error;

/// Result type used by relay utils.
pub type Result<T> = std::result::Result<T, Error>;

/// Relay utils errors.
#[derive(Error, Debug)]
pub enum Error {
    /// Failed to request a float value from HTTP service.
    #[error("Failed to fetch token price from remote server: {0}")]
    FetchTokenPrice(#[source] anyhow::Error),
    /// Failed to parse the response from HTTP service.
    #[error("Failed to parse HTTP service response: {0:?}. Response: {1:?}")]
    ParseHttp(serde_json::Error, String),
    /// Failed to select response value from the JSON response.
    #[error("Failed to select value from response: {0:?}. Response: {1:?}")]
    SelectResponseValue(jsonpath_lib::JsonPathError, String),
    /// Failed to parse float value from the selected value.
    #[error("Failed to parse float value {0:?} from response. It is assumed to be positive and normal")]
    ParseFloat(f64),
    /// Couldn't found value in the JSON response.
    #[error("Missing required value from response: {0:?}")]
    MissingResponseValue(String),
    /// Invalid host address was used for exposing Prometheus metrics.
    #[error("Invalid host {0} is used to expose Prometheus metrics: {1}")]
    ExposingMetricsInvalidHost(String, AddrParseError),
    /// Prometheus error.
    #[error("{0}")]
    Prometheus(#[from] substrate_prometheus_endpoint::prometheus::Error),
}
