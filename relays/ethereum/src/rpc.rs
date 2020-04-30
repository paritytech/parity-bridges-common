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

//! RPC Module

// #[warn(missing_docs)]

use crate::ethereum_client::{CallRequest, Error as EthError, EthereumConnectionParams};
use crate::ethereum_types::{
	Address as EthAddress, Bytes, Header as EthereumHeader, Receipt, SignedRawTx, TransactionHash as EthereumTxHash,
	H256, U256, U64,
};
use crate::substrate_client::{Error as SubError, SubstrateConnectionParams};
use crate::substrate_types::{Hash, Header as SubstrateHeader};

use async_trait::async_trait;
use ethereum_tx_sign::RawTransaction;
use jsonrpsee::common::Params;
use jsonrpsee::transport::http::{HttpTransportClient, RequestError};
use jsonrpsee::Client;
use jsonrpsee::{
	raw::client::{RawClient, RawClientError},
	transport::TransportClient,
};
use serde_json;

type RpcHttpError = RawClientError<RequestError>;

jsonrpsee::rpc_api! {
	Ethereum {
		fn eth_estimateGas(call_request: Params) -> U256;
		fn eth_blockNumber() -> U64;
		fn eth_getBlockByNumber(block_number: Params) -> EthereumHeader;
		fn eth_getBlockByHash(hash: Params) -> EthereumHeader;
		fn eth_getTransactionReceipt(transaction_hash: Params) -> Receipt;
		fn eth_call(transaction_call: Params) -> Bytes;
		fn eth_getTransactionCount(address: Params) -> U256;
		fn eth_submitTransaction(transaction: Params) -> EthereumTxHash;
	}

	Substrate {
		fn chain_getHeader(params: Params) -> SubstrateHeader;
	}
}

#[async_trait]
pub trait EthereumRpc {
	async fn estimate_gas(&mut self, call_request: CallRequest) -> Result<U256, RpcHttpError>;
	async fn best_block_number(&mut self) -> Result<u64, RpcHttpError>;
	async fn header_by_number(&mut self, block_number: u64) -> Result<EthereumHeader, RpcHttpError>;
	async fn header_by_hash(&mut self, hash: H256) -> Result<EthereumHeader, RpcHttpError>;
	async fn transaction_receipt(&mut self, transaction_hash: H256) -> Result<Receipt, RpcHttpError>;
	async fn account_nonce(&mut self, address: EthAddress) -> Result<U256, RpcHttpError>;
	async fn submit_transaction(&mut self, signed_raw_tx: SignedRawTx) -> Result<EthereumTxHash, RpcHttpError>;
}

pub struct EthereumRpcClient {
	client: RawClient<HttpTransportClient>,
}

impl EthereumRpcClient {
	pub fn new(params: EthereumConnectionParams) -> Self {
		let uri = format!("http://{}:{}", params.host, params.port);
		let transport = HttpTransportClient::new(&uri);
		let client = RawClient::new(transport);

		Self { client }
	}
}

#[async_trait]
impl EthereumRpc for EthereumRpcClient {
	// Not sure if I should use EthError, or jsonrpc::client::RequestError
	async fn estimate_gas(&mut self, call_request: CallRequest) -> Result<U256, RpcHttpError> {
		let params = Params::Array(vec![serde_json::to_value(call_request).unwrap()]);
		Ok(Ethereum::eth_estimateGas(&mut self.client, params).await?)
	}

	async fn best_block_number(&mut self) -> Result<u64, RpcHttpError> {
		Ok(Ethereum::eth_blockNumber(&mut self.client).await?.as_u64())
	}

	async fn header_by_number(&mut self, block_number: u64) -> Result<EthereumHeader, RpcHttpError> {
		// Only want to get hashes back from the RPC
		let return_full_tx_obj = false;

		let params = Params::Array(vec![
			serde_json::to_value(U64::from(block_number)).unwrap(),
			serde_json::to_value(return_full_tx_obj).unwrap(),
		]);

		let header = Ethereum::eth_getBlockByNumber(&mut self.client, params).await?;
		match header.number.is_some() && header.hash.is_some() {
			true => Ok(header),
			false => todo!(),
		}
	}

	async fn header_by_hash(&mut self, hash: H256) -> Result<EthereumHeader, RpcHttpError> {
		// Only want to get hashes back from the RPC
		let return_full_tx_obj = false;

		let params = Params::Array(vec![
			serde_json::to_value(hash).unwrap(),
			serde_json::to_value(return_full_tx_obj).unwrap(),
		]);

		let header = Ethereum::eth_getBlockByHash(&mut self.client, params).await?;
		// Q: Slava, why are we checking `is_none()` here?
		match header.number.is_none() && header.hash.is_none() {
			true => Ok(header),
			false => todo!(),
		}
	}

	async fn transaction_receipt(&mut self, transaction_hash: H256) -> Result<Receipt, RpcHttpError> {
		let params = Params::Array(vec![serde_json::to_value(transaction_hash).unwrap()]);
		let receipt = Ethereum::eth_getTransactionReceipt(&mut self.client, params).await?;

		match receipt.gas_used {
			Some(_) => Ok(receipt),
			None => todo!(),
		}
	}

	async fn account_nonce(&mut self, address: EthAddress) -> Result<U256, RpcHttpError> {
		let params = Params::Array(vec![serde_json::to_value(address).unwrap()]);
		Ok(Ethereum::eth_getTransactionCount(&mut self.client, params).await?)
	}

	async fn submit_transaction(&mut self, signed_raw_tx: SignedRawTx) -> Result<EthereumTxHash, RpcHttpError> {
		let transaction = serde_json::to_value(Bytes(signed_raw_tx)).unwrap();
		let params = Params::Array(vec![transaction]);

		Ok(Ethereum::eth_submitTransaction(&mut self.client, params).await?)
	}
}

#[async_trait]
pub trait SubstrateRpc {
	async fn best_header(&mut self) -> Result<SubstrateHeader, ()>;
	async fn header_by_hash(&mut self, hash: Hash) -> Result<SubstrateHeader, ()>;
}

pub struct SubstrateRpcClient {
	client: RawClient<HttpTransportClient>,
}

impl SubstrateRpcClient {
	pub fn new(params: SubstrateConnectionParams) -> Self {
		let uri = format!("http://{}:{}", params.host, params.port);
		let transport = HttpTransportClient::new(&uri);
		let client = RawClient::new(transport);

		Self { client }
	}
}

#[async_trait]
impl SubstrateRpc for SubstrateRpcClient {
	async fn best_header(&mut self) -> Result<SubstrateHeader, ()> {
		Ok(Substrate::chain_getHeader(&mut self.client, Params::None)
			.await
			.unwrap())
	}

	async fn header_by_hash(&mut self, hash: Hash) -> Result<SubstrateHeader, ()> {
		let hash = serde_json::to_value(hash).unwrap();
		let params = Params::Array(vec![hash]);
		let best_header = Substrate::chain_getHeader(&mut self.client, params).await.unwrap();

		Ok(best_header)
	}
}
