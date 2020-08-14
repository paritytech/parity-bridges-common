// Copyright 2019-2020 Parity Technologies (UK) Ltd.
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

use backoff::{future::FutureOperation, ExponentialBackoff};

use crate::ethereum_client::{EthereumConnectionParams, EthereumRpcClient};
use crate::instances::SupportedInstance;
use crate::rpc_errors::RpcError;
use crate::substrate_client::{SubstrateConnectionParams, SubstrateRpcClient};

use std::time::Duration;

/// Max delay after connection-unrelated error happened before we'll try the
/// same request again.
const MAX_BACKOFF_INTERVAL: Duration = Duration::from_secs(60);

/// Error type that can signal connection errors.
pub trait MaybeConnectionError {
	/// Returns true if error (maybe) represents connection error.
	fn is_connection_error(&self) -> bool;
}

/// Stringified error that may be either connection-related or not.
#[derive(Debug)]
pub enum StringifiedMaybeConnectionError {
	/// The error is connection-related error.
	Connection(String),
	/// The error is connection-unrelated error.
	NonConnection(String),
}

impl StringifiedMaybeConnectionError {
	/// Create new stringified connection error.
	pub fn new(is_connection_error: bool, error: String) -> Self {
		if is_connection_error {
			StringifiedMaybeConnectionError::Connection(error)
		} else {
			StringifiedMaybeConnectionError::NonConnection(error)
		}
	}
}

impl MaybeConnectionError for StringifiedMaybeConnectionError {
	fn is_connection_error(&self) -> bool {
		match *self {
			StringifiedMaybeConnectionError::Connection(_) => true,
			StringifiedMaybeConnectionError::NonConnection(_) => false,
		}
	}
}

impl ToString for StringifiedMaybeConnectionError {
	fn to_string(&self) -> String {
		match *self {
			StringifiedMaybeConnectionError::Connection(ref err) => err.clone(),
			StringifiedMaybeConnectionError::NonConnection(ref err) => err.clone(),
		}
	}
}

/// Exponential backoff for connection-unrelated errors retries.
pub fn retry_backoff() -> ExponentialBackoff {
	let mut backoff = ExponentialBackoff::default();
	// we do not want relayer to stop
	backoff.max_elapsed_time = None;
	backoff.max_interval = MAX_BACKOFF_INTERVAL;
	backoff
}

/// Compact format of IDs vector.
pub fn format_ids<Id: std::fmt::Debug>(mut ids: impl ExactSizeIterator<Item = Id>) -> String {
	const NTH_PROOF: &str = "we have checked len; qed";
	match ids.len() {
		0 => "<nothing>".into(),
		1 => format!("{:?}", ids.next().expect(NTH_PROOF)),
		2 => {
			let id0 = ids.next().expect(NTH_PROOF);
			let id1 = ids.next().expect(NTH_PROOF);
			format!("[{:?}, {:?}]", id0, id1)
		}
		len => {
			let id0 = ids.next().expect(NTH_PROOF);
			let id_last = ids.last().expect(NTH_PROOF);
			format!("{}:[{:?} ... {:?}]", len, id0, id_last)
		}
	}
}

/// Try to connect to a Substrate client.
///
/// If unsuccessful this function will retry the connection in the future, waiting longer after each
/// unsuccessful attempt.
pub async fn try_connect_to_sub_client(
	params: SubstrateConnectionParams,
	instance: SupportedInstance,
) -> Result<SubstrateRpcClient, RpcError> {
	let wait = Duration::from_secs(1);
	(|| async {
		let sub_client_fut = SubstrateRpcClient::new(params.clone(), (&instance).into());
		async_std::future::timeout(wait, sub_client_fut)
			.await
			.map_err(backoff::Error::Transient)
	})
	.retry_notify(
		ExponentialBackoff::default(),
		|_, _| log::warn!(target: "bridge", "Failed to connect to Substrate client at {}, trying again...", &params),
	)
	.await?
}

/// Try to connect to an Ethereum client.
///
/// If unsuccessful this function will retry the connection in the future, waiting longer after each
/// unsuccessful attempt.
pub async fn try_connect_to_eth_client(params: EthereumConnectionParams) -> Result<EthereumRpcClient, RpcError> {
	use crate::rpc::EthereumRpc;
	let client = EthereumRpcClient::new(params.clone());

	// Try and get the nonce of the zero-address as a check to see if we're able to connect to
	// the Ethereum client.
	let _nonce = (|| async {
		let wait = Duration::from_secs(1);
		let eth_client_fut = client.account_nonce([0u8; 20].into());
		async_std::future::timeout(wait, eth_client_fut)
			.await
			.map_err(backoff::Error::Transient)
	})
	.retry_notify(
		ExponentialBackoff::default(),
		|_, _| log::warn!(target: "bridge", "Failed to connect to Ethereum client at {}, trying again...", &params),
	)
	.await?;

	Ok(client)
}
