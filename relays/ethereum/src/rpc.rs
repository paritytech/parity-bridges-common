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

#![allow(dead_code)]
#![allow(unused_variables)]
#[warn(missing_docs)]
use std::result;

use crate::ethereum_client::{CallRequest, EthereumConnectionParams};
use crate::ethereum_types::{
	Address as EthAddress, Bytes, EthereumHeaderId, Header as EthereumHeader, Receipt, SignedRawTx,
	TransactionHash as EthereumTxHash, H256, U256, U64,
};
use crate::rpc_errors::{EthereumNodeError, RpcError};
use crate::substrate_client::SubstrateConnectionParams;
use crate::substrate_types::{Hash as SubstrateHash, Header as SubstrateHeader, Number as SubBlockNumber};
use crate::sync_types::HeaderId;

use async_trait::async_trait;
use bridge_node_runtime::UncheckedExtrinsic;
use codec::{Decode, Encode};
use jsonrpsee::common::Params;
use jsonrpsee::raw::client::RawClient;
use jsonrpsee::transport::http::HttpTransportClient;
use serde_json;
use sp_bridge_eth_poa::{EthereumHeadersApiCalls, Header as SubstrateEthereumHeader, RuntimeApiCalls};

type Result<T> = result::Result<T, RpcError>;
type GrandpaAuthorityList = Vec<u8>;

jsonrpsee::rpc_api! {
	Ethereum {
		#[rpc(method = "eth_estimateGas")]
		fn estimate_gas(call_request: Params) -> U256;
		#[rpc(method = "eth_blockNumber")]
		fn block_number() -> U64;
		#[rpc(method = "eth_getBlockByNumber")]
		fn get_block_by_number(block_number: Params) -> EthereumHeader;
		#[rpc(method = "eth_getBlockByHash")]
		fn get_block_by_hash(hash: Params) -> EthereumHeader;
		#[rpc(method = "eth_getTransactionReceipt")]
		fn get_transaction_receipt(transaction_hash: Params) -> Receipt;
		#[rpc(method = "eth_getTransactionCount")]
		fn get_transaction_count(address: Params) -> U256;
		#[rpc(method = "eth_submitTransaction")]
		fn submit_transaction(transaction: Params) -> EthereumTxHash;
		#[rpc(method = "eth_call")]
		fn call(transaction_call: Params) -> Bytes;
	}

	Substrate {
		#[rpc(method = "chain_getHeader")]
		fn chain_get_header(params: Params) -> SubstrateHeader;
		#[rpc(method = "chain_getBlockHash")]
		fn chain_get_block_hash(block_number: Params) -> SubstrateHash;
		#[rpc(method = "system_accountNextIndex")]
		fn system_account_next_index(account_id: Params) -> node_primitives::Index;
		#[rpc(method = "author_submitExtrinsic")]
		fn author_submit_extrinsic(extrinsic: Params) -> SubstrateHash;
		#[rpc(method = "state_call")]
		fn state_call(method: Params) -> Bytes;
	}
}

/// The API for the supported Ethereum RPC methods.
#[async_trait]
pub trait EthereumRpc {
	/// Estimate gas usage for the given call.
	async fn estimate_gas(&mut self, call_request: CallRequest) -> Result<U256>;
	/// Retrieve number of the best known block from the Ethereum node.
	async fn best_block_number(&mut self) -> Result<u64>;
	/// Retrieve block header by its number from Ethereum node.
	async fn header_by_number(&mut self, block_number: u64) -> Result<EthereumHeader>;
	/// Retrieve block header by its hash from Ethereum node.
	async fn header_by_hash(&mut self, hash: H256) -> Result<EthereumHeader>;
	/// Retrieve transaction receipt by transaction hash.
	async fn transaction_receipt(&mut self, transaction_hash: H256) -> Result<Receipt>;
	/// Get the nonce of the given account.
	async fn account_nonce(&mut self, address: EthAddress) -> Result<U256>;
	/// Submit an Ethereum transaction.
	///
	/// The transaction must already be signed before sending it through this method.
	async fn submit_transaction(&mut self, signed_raw_tx: SignedRawTx) -> Result<EthereumTxHash>;
	/// Submit a call to an Ethereum smart contract.
	async fn eth_call(&mut self, call_transaction: CallRequest) -> Result<Bytes>;
}

/// The client used to interact with an Ethereum node through RPC.
pub struct EthereumRpcClient {
	client: RawClient<HttpTransportClient>,
}

impl EthereumRpcClient {
	/// Create a new Ethereum RPC Client.
	pub fn new(params: EthereumConnectionParams) -> Self {
		let uri = format!("http://{}:{}", params.host, params.port);
		let transport = HttpTransportClient::new(&uri);
		let client = RawClient::new(transport);

		Self { client }
	}
}

#[async_trait]
impl EthereumRpc for EthereumRpcClient {
	async fn estimate_gas(&mut self, call_request: CallRequest) -> Result<U256> {
		let params = Params::Array(vec![serde_json::to_value(call_request)?]);
		Ok(Ethereum::estimate_gas(&mut self.client, params).await?)
	}

	async fn best_block_number(&mut self) -> Result<u64> {
		Ok(Ethereum::block_number(&mut self.client).await?.as_u64())
	}

	async fn header_by_number(&mut self, block_number: u64) -> Result<EthereumHeader> {
		// Only want to get hashes back from the RPC
		let return_full_tx_obj = false;

		let params = Params::Array(vec![
			serde_json::to_value(U64::from(block_number))?,
			serde_json::to_value(return_full_tx_obj)?,
		]);

		let header = Ethereum::get_block_by_number(&mut self.client, params).await?;
		match header.number.is_some() && header.hash.is_some() {
			true => Ok(header),
			false => Err(RpcError::Ethereum(EthereumNodeError::IncompleteHeader)),
		}
	}

	async fn header_by_hash(&mut self, hash: H256) -> Result<EthereumHeader> {
		// Only want to get hashes back from the RPC
		let return_full_tx_obj = false;

		let params = Params::Array(vec![
			serde_json::to_value(hash)?,
			serde_json::to_value(return_full_tx_obj)?,
		]);

		let header = Ethereum::get_block_by_hash(&mut self.client, params).await?;
		match header.number.is_some() && header.hash.is_some() {
			true => Ok(header),
			false => Err(RpcError::Ethereum(EthereumNodeError::IncompleteHeader)),
		}
	}

	async fn transaction_receipt(&mut self, transaction_hash: H256) -> Result<Receipt> {
		let params = Params::Array(vec![
			serde_json::to_value(transaction_hash)?
		]);
		let receipt = Ethereum::get_transaction_receipt(&mut self.client, params).await?;

		match receipt.gas_used {
			Some(_) => Ok(receipt),
			None => Err(RpcError::Ethereum(EthereumNodeError::IncompleteReceipt)),
		}
	}

	async fn account_nonce(&mut self, address: EthAddress) -> Result<U256> {
		let params = Params::Array(vec![serde_json::to_value(address)?]);
		Ok(Ethereum::get_transaction_count(&mut self.client, params).await?)
	}

	async fn submit_transaction(&mut self, signed_raw_tx: SignedRawTx) -> Result<EthereumTxHash> {
		let transaction = serde_json::to_value(Bytes(signed_raw_tx))?;
		let params = Params::Array(vec![transaction]);

		Ok(Ethereum::submit_transaction(&mut self.client, params).await?)
	}

	async fn eth_call(&mut self, call_transaction: CallRequest) -> Result<Bytes> {
		let params = Params::Array(vec![serde_json::to_value(call_transaction)?]);
		Ok(Ethereum::call(&mut self.client, params).await?)
	}
}

/// The API for the supported Substrate RPC methods.
#[async_trait]
pub trait SubstrateRpc {
	/// Returns the best Substrate header.
	async fn best_header(&mut self) -> Result<SubstrateHeader>;
	/// Get a Substrate header by its hash.
	async fn header_by_hash(&mut self, hash: SubstrateHash) -> Result<SubstrateHeader>;
	/// Get a Substrate block hash by its number.
	async fn block_hash_by_number(&mut self, number: SubBlockNumber) -> Result<SubstrateHash>;
	/// Get a Substrate header by its number.
	async fn header_by_number(&mut self, block_number: SubBlockNumber) -> Result<SubstrateHeader>;
	/// Get the nonce of the given Substrate account.
	async fn next_account_index(&mut self, account: node_primitives::AccountId) -> Result<node_primitives::Index>;
	/// Returns best Ethereum block that Substrate runtime knows of.
	async fn best_ethereum_block(&mut self) -> Result<EthereumHeaderId>;
	/// Returns whether or not transactions receipts are required for Ethereum header submission.
	async fn ethereum_receipts_required(&mut self, header: SubstrateEthereumHeader) -> Result<bool>;
	/// Returns whether or not the given Ethereum header is known to the Substrate runtime.
	async fn ethereum_header_known(&mut self, header_id: EthereumHeaderId) -> Result<bool>;
	/// Submit an extrinsic for inclusion in a block.
	async fn submit_extrinsic(&mut self, transaction: UncheckedExtrinsic) -> Result<SubstrateHash>;
	/// Get the GRANDPA authority set at given block.
	async fn grandpa_authorities_set(&mut self, block: SubstrateHash) -> Result<GrandpaAuthorityList>;
}

/// The client used to interact with a Substrate node through RPC.
pub struct SubstrateRpcClient {
	client: RawClient<HttpTransportClient>,
}

impl SubstrateRpcClient {
	/// Create a new Substrate RPC Client.
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
		Ok(Substrate::chain_get_header(&mut self.client, Params::None).await?)
	}

	async fn header_by_hash(&mut self, hash: SubstrateHash) -> Result<SubstrateHeader> {
		let hash = serde_json::to_value(hash)?;
		let params = Params::Array(vec![hash]);

		Ok(Substrate::chain_get_header(&mut self.client, params).await?)
	}

	async fn block_hash_by_number(&mut self, number: SubBlockNumber) -> Result<SubstrateHash> {
		let params = Params::Array(vec![serde_json::to_value(number)?]);

		Ok(Substrate::chain_get_block_hash(&mut self.client, params).await?)
	}

	async fn header_by_number(&mut self, block_number: SubBlockNumber) -> Result<SubstrateHeader> {
		let block_hash = Self::block_hash_by_number(self, block_number).await?;
		Ok(Self::header_by_hash(self, block_hash).await?)
	}

	async fn next_account_index(&mut self, account: node_primitives::AccountId) -> Result<node_primitives::Index> {
		// Q: Should this belong here, or be left to the caller?
		use sp_core::crypto::Ss58Codec;
		let account = serde_json::to_value(account.to_ss58check())?;
		let params = Params::Array(vec![account]);

		Ok(Substrate::system_account_next_index(&mut self.client, params).await?)
	}

	async fn best_ethereum_block(&mut self) -> Result<EthereumHeaderId> {
		let call = EthereumHeadersApiCalls::BestBlock.to_string();
		let data = "0x".to_string();
		let params = Params::Array(vec![serde_json::Value::String(call), serde_json::Value::String(data)]);

		let encoded_response = Substrate::state_call(&mut self.client, params).await?;
		let decoded_response: (u64, sp_bridge_eth_poa::H256) = Decode::decode(&mut &encoded_response.0[..])?;

		let best_header_id = HeaderId(decoded_response.0, decoded_response.1);
		Ok(best_header_id)
	}

	// Should I work with a QueuedEthereumHeader or a SubstrateEthereumHeader since that's what'll
	// actually get encoded and sent to the RPC?
	//
	// I'm leaning towards the SubstrateEthereumHeader since that's a bit "lower level"
	async fn ethereum_receipts_required(&mut self, header: SubstrateEthereumHeader) -> Result<bool> {
		let call = EthereumHeadersApiCalls::IsImportRequiresReceipts.to_string();
		let data = Bytes(header.encode());
		let params = Params::Array(vec![serde_json::Value::String(call), serde_json::to_value(data)?]);

		let encoded_response = Substrate::state_call(&mut self.client, params).await?;
		let receipts_required: bool = Decode::decode(&mut &encoded_response.0[..])?;

		// Gonna make it the responsibility of the caller to return (receipts_required, id)
		Ok(receipts_required)
	}

	// The Substrate module could prune old headers. So this function could return false even
	// if header is synced. And we'll mark corresponding Ethereum header as Orphan.
	//
	// But when we read the best header from Substrate next time, we will know that
	// there's a better header. This Orphan will either be marked as synced, or
	// eventually pruned.
	async fn ethereum_header_known(&mut self, header_id: EthereumHeaderId) -> Result<bool> {
		let call = EthereumHeadersApiCalls::IsKnownBlock.to_string();
		let data = Bytes(header_id.1.encode());
		let params = Params::Array(vec![serde_json::Value::String(call), serde_json::to_value(data)?]);

		let encoded_response = Substrate::state_call(&mut self.client, params).await?;
		let is_known_block: bool = Decode::decode(&mut &encoded_response.0[..])?;

		// Gonna make it the responsibility of the caller to return (is_known_block, id)
		Ok(is_known_block)
	}

	// Q: Should I move the UncheckedExtrinsic type elsewhere so I don't have to pull it in from
	// the runtime?
	async fn submit_extrinsic(&mut self, transaction: UncheckedExtrinsic) -> Result<SubstrateHash> {
		let encoded_transaction = Bytes(transaction.encode());
		let params = Params::Array(vec![serde_json::to_value(encoded_transaction)?]);

		Ok(Substrate::author_submit_extrinsic(&mut self.client, params).await?)
	}

	async fn grandpa_authorities_set(&mut self, block: SubstrateHash) -> Result<GrandpaAuthorityList> {
		let call = RuntimeApiCalls::GrandpaAuthorities.to_string();
		let data = block;
		let params = Params::Array(vec![serde_json::Value::String(call), serde_json::to_value(data)?]);

		let encoded_response = Substrate::state_call(&mut self.client, params).await?;
		let authority_list = encoded_response.0;

		Ok(authority_list)
	}
}
