use thiserror::Error;
use crate::rpc_errors::RpcError;

/// Result type used by PoA relay.
pub type Result<T> = std::result::Result<T, Error>;

/// Ethereum PoA relay errors.
#[derive(Error, Debug)]
pub enum Error {
    /// Failed to decode initial header.
    #[error("Error decoding initial header: {0}")]
    DecodeInitialHeader(codec::Error),
    /// RPC error.
    #[error("{0}")]
    Rpc(#[from] RpcError),
    /// Failed to read genesis header.
    #[error("Error reading Substrate genesis header: {0:?}")]
    ReadGenesisHeader(relay_substrate_client::Error),
    /// Failed to read initial GRANDPA authorities.
    #[error("Error reading GRANDPA authorities set: {0:?}")]
    ReadAuthorities(relay_substrate_client::Error),
    /// Failed to deploy bridge contract to Ethereum chain.
    #[error("Error deploying contract: {0:?}")]
    DeployContract(RpcError),
}