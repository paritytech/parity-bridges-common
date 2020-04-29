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

use crate::ethereum_types::U256;
use crate::substrate_types::Header as SubstrateHeader;

use jsonrpsee::Client;
use jsonrpsee::common::Params;
use jsonrpsee::{
	raw::client::{RawClient, RawClientError},
	transport::TransportClient,
};
use jsonrpsee::transport::http::{HttpTransportClient, RequestError};

use serde_json;

jsonrpsee::rpc_api! {
    Ethereum {
		fn eth_estimateGas(call_request: Params) -> U256;
    }

    Substrate {
		fn chain_getHeader() -> SubstrateHeader;
    }
}

pub async fn estimate_gas(client: &mut RawClient<HttpTransportClient>) -> U256 {
	let call_request = Params::Array(vec![
		serde_json::to_value(1).unwrap(),
		serde_json::to_value(2).unwrap(),
		serde_json::to_value(3).unwrap(),
	]);
	Ethereum::eth_estimateGas(client, call_request).await.unwrap()
}

pub async fn best_header(client: &mut RawClient<HttpTransportClient>) -> SubstrateHeader {
	Substrate::chain_getHeader(client).await.unwrap()
}

pub async fn test_rpc_calls() {
	let mut eth_transport = jsonrpsee::transport::http::HttpTransportClient::new("http://localhost:8545");
    let mut eth_client = jsonrpsee::raw::RawClient::new(eth_transport);

	let mut sub_transport = jsonrpsee::transport::http::HttpTransportClient::new("http://localhost:9933");
    let mut sub_client = jsonrpsee::raw::RawClient::new(sub_transport);

	estimate_gas(&mut eth_client);
	best_header(&mut sub_client);
}
