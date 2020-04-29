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

use crate::ethereum_client::EthereumConnectionParams;
use crate::ethereum_types::U256;
use crate::substrate_client::SubstrateConnectionParams;
use crate::substrate_types::{Hash, Header as SubstrateHeader};

use async_trait::async_trait;
use jsonrpsee::common::Params;
use jsonrpsee::transport::http::{HttpTransportClient, RequestError};
use jsonrpsee::Client;
use jsonrpsee::{
	raw::client::{RawClient, RawClientError},
	transport::TransportClient,
};
use serde_json;

jsonrpsee::rpc_api! {
	Ethereum {
		fn eth_estimateGas(call_request: Params) -> U256;
	}

	Substrate {
		fn chain_getHeader(params: Params) -> SubstrateHeader;
	}
}

#[async_trait]
pub trait EthereumRpc {
	async fn estimate_gas(&mut self, _params: Vec<u32>) -> Result<U256, ()>;
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
	async fn estimate_gas(&mut self, _params: Vec<u32>) -> Result<U256, ()> {
		let call_request = Params::Array(vec![
			serde_json::to_value(1).unwrap(),
			serde_json::to_value(2).unwrap(),
			serde_json::to_value(3).unwrap(),
		]);

		let gas = Ethereum::eth_estimateGas(&mut self.client, call_request).await.unwrap();
		Ok(gas)
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
