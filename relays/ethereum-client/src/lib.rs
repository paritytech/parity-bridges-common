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

//! Tools to interact with (Open) Ethereum node using RPC methods.

#![warn(missing_docs)]

use crate::types::{
	Address, Bytes, CallRequest, Header, HeaderWithTransactions, Receipt, SignedRawTx, Transaction, TransactionHash,
	H256, U256,
};

use async_trait::async_trait;

mod client;
mod error;
mod rpc;
mod sign;

pub use crate::error::{Error, Result};
pub use crate::sign::{sign_and_submit_transaction, SigningParams};

pub mod types;

/// Ethereum connection params.
#[derive(Debug, Clone)]
pub struct ConnectionParams {
	/// Ethereum RPC host.
	pub host: String,
	/// Ethereum RPC port.
	pub port: u16,
}

impl Default for ConnectionParams {
	fn default() -> Self {
		ConnectionParams {
			host: "localhost".into(),
			port: 8545,
		}
	}
}

/// The API for the supported Ethereum RPC methods.
///
/// Cloning client is a lightweight operation that only clones internal references.
#[async_trait]
pub trait Client: 'static + Send + Sync + Clone {
	/// Estimate gas usage for the given call.
	async fn estimate_gas(&self, call_request: CallRequest) -> Result<U256>;
	/// Retrieve number of the best known block from the Ethereum node.
	async fn best_block_number(&self) -> Result<u64>;
	/// Retrieve block header by its number from Ethereum node.
	async fn header_by_number(&self, block_number: u64) -> Result<Header>;
	/// Retrieve block header by its hash from Ethereum node.
	async fn header_by_hash(&self, hash: H256) -> Result<Header>;
	/// Retrieve block header and its transactions by its number from Ethereum node.
	async fn header_by_number_with_transactions(&self, block_number: u64) -> Result<HeaderWithTransactions>;
	/// Retrieve block header and its transactions by its hash from Ethereum node.
	async fn header_by_hash_with_transactions(&self, hash: H256) -> Result<HeaderWithTransactions>;
	/// Retrieve transaction by its hash from Ethereum node.
	async fn transaction_by_hash(&self, hash: H256) -> Result<Option<Transaction>>;
	/// Retrieve transaction receipt by transaction hash.
	async fn transaction_receipt(&self, transaction_hash: H256) -> Result<Receipt>;
	/// Get the nonce of the given account.
	async fn account_nonce(&self, address: Address) -> Result<U256>;
	/// Submit an Ethereum transaction.
	///
	/// The transaction must already be signed before sending it through this method.
	async fn submit_transaction(&self, signed_raw_tx: SignedRawTx) -> Result<TransactionHash>;
	/// Submit a call to an Ethereum smart contract.
	async fn eth_call(&self, call_transaction: CallRequest) -> Result<Bytes>;
}

/// Create new Ethereum RPC client.
pub fn new(params: ConnectionParams) -> impl Client {
	crate::client::Client::new(params)
}
