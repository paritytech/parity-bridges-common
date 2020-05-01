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

use std::result;

use crate::ethereum_client::{CallRequest, Error as EthError, EthereumConnectionParams};
use crate::ethereum_types::{
	Address as EthAddress, Bytes, EthereumHeaderId, Header as EthereumHeader, Receipt, SignedRawTx,
	TransactionHash as EthereumTxHash, H256, U256, U64,
};
use crate::rpc_errors::{EthereumNodeError, RpcError, SubstrateNodeError};
use crate::substrate_client::{Error as SubError, SubstrateConnectionParams};
use crate::substrate_types::{Hash as SubstrateHash, Header as SubstrateHeader, Number as SubBlockNumber};

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
use sp_bridge_eth_poa::EthereumHeadersApiCalls;

/// Proof of hash serialization success.
const HASH_SERIALIZATION_PROOF: &'static str = "hash serialization never fails; qed";
/// Proof of integer serialization success.
const INT_SERIALIZATION_PROOF: &'static str = "integer serialization never fails; qed";
/// Proof of bool serialization success.
const BOOL_SERIALIZATION_PROOF: &'static str = "bool serialization never fails; qed";

type Result<T> = result::Result<T, RpcError>;

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
		fn state_call(method: Params) -> Bytes;
		fn author_submitExtrinsic(extrinsic: Params) ->SubstrateHash;
		fn chain_getBlockHash(block_number: Params) -> SubstrateHash;
		fn system_accountNextIndex(account_id: Params) -> node_primitives::Index;
	}
}

#[async_trait]
pub trait EthereumRpc {
	async fn estimate_gas(&mut self, call_request: CallRequest) -> Result<U256>;
	async fn best_block_number(&mut self) -> Result<u64>;
	async fn header_by_number(&mut self, block_number: u64) -> Result<EthereumHeader>;
	async fn header_by_hash(&mut self, hash: H256) -> Result<EthereumHeader>;
	async fn transaction_receipt(&mut self, transaction_hash: H256) -> Result<Receipt>;
	async fn account_nonce(&mut self, address: EthAddress) -> Result<U256>;
	async fn submit_transaction(&mut self, signed_raw_tx: SignedRawTx) -> Result<EthereumTxHash>;
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
	async fn estimate_gas(&mut self, call_request: CallRequest) -> Result<U256> {
		let params = Params::Array(vec![serde_json::to_value(call_request)?]);
		Ok(Ethereum::eth_estimateGas(&mut self.client, params).await?)
	}

	async fn best_block_number(&mut self) -> Result<u64> {
		Ok(Ethereum::eth_blockNumber(&mut self.client).await?.as_u64())
	}

	async fn header_by_number(&mut self, block_number: u64) -> Result<EthereumHeader> {
		// Only want to get hashes back from the RPC
		let return_full_tx_obj = false;

		let params = Params::Array(vec![
			serde_json::to_value(U64::from(block_number)).expect(INT_SERIALIZATION_PROOF),
			serde_json::to_value(return_full_tx_obj).expect(BOOL_SERIALIZATION_PROOF),
		]);

		let header = Ethereum::eth_getBlockByNumber(&mut self.client, params).await?;
		match header.number.is_some() && header.hash.is_some() {
			true => Ok(header),
			false => todo!(),
		}
	}

	async fn header_by_hash(&mut self, hash: H256) -> Result<EthereumHeader> {
		// Only want to get hashes back from the RPC
		let return_full_tx_obj = false;

		let params = Params::Array(vec![
			serde_json::to_value(hash).expect(HASH_SERIALIZATION_PROOF),
			serde_json::to_value(return_full_tx_obj).expect(BOOL_SERIALIZATION_PROOF),
		]);

		let header = Ethereum::eth_getBlockByHash(&mut self.client, params).await?;
		// Q: Slava, why are we checking `is_none()` here?
		match header.number.is_none() && header.hash.is_none() {
			true => Ok(header),
			false => Err(RpcError::Ethereum(EthereumNodeError::IncompleteHeader)),
		}
	}

	async fn transaction_receipt(&mut self, transaction_hash: H256) -> Result<Receipt> {
		let params = Params::Array(vec![
			serde_json::to_value(transaction_hash).expect(HASH_SERIALIZATION_PROOF)
		]);
		let receipt = Ethereum::eth_getTransactionReceipt(&mut self.client, params).await?;

		match receipt.gas_used {
			Some(_) => Ok(receipt),
			None => Err(RpcError::Ethereum(EthereumNodeError::IncompleteReceipt)),
		}
	}

	async fn account_nonce(&mut self, address: EthAddress) -> Result<U256> {
		let params = Params::Array(vec![serde_json::to_value(address).unwrap()]);
		Ok(Ethereum::eth_getTransactionCount(&mut self.client, params).await?)
	}

	async fn submit_transaction(&mut self, signed_raw_tx: SignedRawTx) -> Result<EthereumTxHash> {
		let transaction = serde_json::to_value(Bytes(signed_raw_tx))?;
		let params = Params::Array(vec![transaction]);

		Ok(Ethereum::eth_submitTransaction(&mut self.client, params).await?)
	}
}

#[async_trait]
pub trait SubstrateRpc {
	async fn best_header(&mut self) -> Result<SubstrateHeader>;
	async fn header_by_hash(&mut self, hash: SubstrateHash) -> Result<SubstrateHeader>;
	async fn block_hash_by_number(&mut self, number: SubBlockNumber) -> Result<SubstrateHash>;
	async fn header_by_number(&mut self, block_number: SubBlockNumber) -> Result<SubstrateHeader>;
	async fn next_account_index(
		&mut self,
		account: node_primitives::AccountId,
	) -> Result<node_primitives::Index>;
	async fn best_ethereum_block(&mut self) -> Result<EthereumHeaderId>;
	async fn ethereum_receipts_required(&mut self) -> Result<(bool, EthereumHeaderId)>;
	async fn ethereum_header_known(
		&mut self,
		header_id: EthereumHeaderId,
	) -> Result<(bool, EthereumHeaderId)>;
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
	async fn best_header(&mut self) -> Result<SubstrateHeader> {
		Ok(Substrate::chain_getHeader(&mut self.client, Params::None).await?)
	}

	async fn header_by_hash(&mut self, hash: SubstrateHash) -> Result<SubstrateHeader> {
		let hash = serde_json::to_value(hash)?;
		let params = Params::Array(vec![hash]);

		Ok(Substrate::chain_getHeader(&mut self.client, params).await?)
	}

	async fn block_hash_by_number(&mut self, number: SubBlockNumber) -> Result<SubstrateHash> {
		let params = Params::Array(vec![serde_json::to_value(number)?]);

		Ok(Substrate::chain_getBlockHash(&mut self.client, params).await?)
	}

	async fn header_by_number(&mut self, block_number: SubBlockNumber) -> Result<SubstrateHeader> {
		// let block_hash = Self::block_hash_by_number(self, block_number).await?;
		// Self::header_by_hash(self, block_hash).await?
		todo!()
	}

	async fn next_account_index(
		&mut self,
		account: node_primitives::AccountId,
	) -> Result<node_primitives::Index> {
		// Q: Should this belong here, or be left to the caller?
		use sp_core::crypto::Ss58Codec;
		let account = serde_json::to_value(account.to_ss58check())?;
		let params = Params::Array(vec![account]);

		Ok(Substrate::system_accountNextIndex(&mut self.client, params).await?)
	}

	async fn best_ethereum_block(&mut self) -> Result<EthereumHeaderId> {
		let call = EthereumHeadersApiCalls::BestBlock.to_string();
		let data = "0x".to_string();
		let params = Params::Array(vec![serde_json::Value::String(call), serde_json::Value::String(data)]);

		let _encoded_result = Substrate::state_call(&mut self.client, params).await?;
		todo!("Decode result")
	}

	// Should I work with a QueuedEthereumHeader or a SubstrateEthereumHeader since that's what'll
	// actually get encoded and sent to the RPC?
	// I'm leaning towards the SubstrateEthereumHeader since that's a bit "lower level"
	async fn ethereum_receipts_required(&mut self) -> Result<(bool, EthereumHeaderId)> {
		let call = EthereumHeadersApiCalls::IsImportRequiresReceipts.to_string();
		let data = "todo_put_header_in_here".to_string();
		let params = Params::Array(vec![serde_json::Value::String(call), serde_json::Value::String(data)]);

		let _encoded_result = Substrate::state_call(&mut self.client, params).await?;
		todo!("Decode result")
	}

	async fn ethereum_header_known(
		&mut self,
		header_id: EthereumHeaderId,
	) -> Result<(bool, EthereumHeaderId)> {
		let call = EthereumHeadersApiCalls::IsKnownBlock.to_string();
		let data = "todo_put_header_id".to_string();
		let params = Params::Array(vec![serde_json::Value::String(call), serde_json::Value::String(data)]);

		let _encoded_result = Substrate::state_call(&mut self.client, params).await?;
		todo!("Decode result")
	}
}
