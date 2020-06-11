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

use crate::ethereum_types::{
	Address, Bytes, CallRequest, EthereumHeaderId, Header, Receipt, SignedRawTx, TransactionHash, H256, U256,
};
use crate::rpc::{Ethereum, EthereumRpc};
use crate::rpc_errors::{EthereumNodeError, RpcError};
use crate::substrate_types::{GrandpaJustification, Hash as SubstrateHash, QueuedSubstrateHeader, SubstrateHeaderId};
use crate::sync_types::{HeaderId, MaybeConnectionError};

use async_trait::async_trait;
use codec::{Decode, Encode};
use ethabi::FunctionOutputDecoder;
use jsonrpsee::raw::{RawClient, RawClientError};
use jsonrpsee::transport::http::{HttpTransportClient, RequestError};
use parity_crypto::publickey::KeyPair;

use std::collections::HashSet;

// to encode/decode contract calls
ethabi_contract::use_contract!(bridge_contract, "res/substrate-bridge-abi.json");

/// Proof of hash serialization success.
const HASH_SERIALIZATION_PROOF: &'static str = "hash serialization never fails; qed";
/// Proof of integer serialization success.
const INT_SERIALIZATION_PROOF: &'static str = "integer serialization never fails; qed";
/// Proof of bool serialization success.
const BOOL_SERIALIZATION_PROOF: &'static str = "bool serialization never fails; qed";

type Result<T> = std::result::Result<T, RpcError>;

/// Ethereum connection params.
#[derive(Debug)]
pub struct EthereumConnectionParams {
	/// Ethereum RPC host.
	pub host: String,
	/// Ethereum RPC port.
	pub port: u16,
}

impl Default for EthereumConnectionParams {
	fn default() -> Self {
		EthereumConnectionParams {
			host: "localhost".into(),
			port: 8545,
		}
	}
}

/// Ethereum signing params.
#[derive(Clone, Debug)]
pub struct EthereumSigningParams {
	/// Ethereum chain id.
	pub chain_id: u64,
	/// Ethereum transactions signer.
	pub signer: KeyPair,
	/// Gas price we agree to pay.
	pub gas_price: U256,
}

impl Default for EthereumSigningParams {
	fn default() -> Self {
		EthereumSigningParams {
			chain_id: 0x11, // Parity dev chain
			// account that has a lot of ether when we run instant seal engine
			// address: 0x00a329c0648769a73afac7f9381e08fb43dbea72
			// secret: 0x4d5db4107d237df6a3d58ee5f70ae63d73d7658d4026f2eefd2f204c81682cb7
			signer: KeyPair::from_secret_slice(
				&hex::decode("4d5db4107d237df6a3d58ee5f70ae63d73d7658d4026f2eefd2f204c81682cb7")
					.expect("secret is hardcoded, thus valid; qed"),
			)
			.expect("secret is hardcoded, thus valid; qed"),
			gas_price: 8_000_000_000u64.into(), // 8 Gwei
		}
	}
}

/// Ethereum client type.
pub type Client = RawClient<HttpTransportClient>;

/// The client used to interact with an Ethereum node through RPC.
pub struct EthereumRpcClient {
	client: Client,
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

/// All possible errors that can occur during interacting with Ethereum node.
#[derive(Debug)]
pub enum Error {
	/// Request start failed.
	StartRequestFailed(RequestError),
	/// Error serializing request.
	RequestSerialization(serde_json::Error),
	/// Request not found (should never occur?).
	RequestNotFound,
	/// Failed to receive response.
	ResponseRetrievalFailed(RawClientError<RequestError>),
	/// Failed to parse response.
	ResponseParseFailed(String),
	/// We have received header with missing number and hash fields.
	IncompleteHeader,
	/// We have received receipt with missing gas_used field.
	IncompleteReceipt,
	/// Invalid Substrate block number received from Ethereum node.
	InvalidSubstrateBlockNumber,
}

impl MaybeConnectionError for Error {
	fn is_connection_error(&self) -> bool {
		match *self {
			Error::StartRequestFailed(_) | Error::ResponseRetrievalFailed(_) => true,
			_ => false,
		}
	}
}

#[async_trait]
impl EthereumRpc for EthereumRpcClient {
	async fn estimate_gas(&mut self, call_request: CallRequest) -> Result<U256> {
		Ok(Ethereum::estimate_gas(&mut self.client, call_request).await?)
	}

	async fn best_block_number(&mut self) -> Result<u64> {
		Ok(Ethereum::block_number(&mut self.client).await?.as_u64())
	}

	async fn header_by_number(&mut self, block_number: u64) -> Result<Header> {
		let header = Ethereum::get_block_by_number(&mut self.client, block_number).await?;
		match header.number.is_some() && header.hash.is_some() && header.logs_bloom.is_some() {
			true => Ok(header),
			false => Err(RpcError::Ethereum(EthereumNodeError::IncompleteHeader)),
		}
	}

	async fn header_by_hash(&mut self, hash: H256) -> Result<Header> {
		let header = Ethereum::get_block_by_hash(&mut self.client, hash).await?;
		match header.number.is_some() && header.hash.is_some() && header.logs_bloom.is_some() {
			true => Ok(header),
			false => Err(RpcError::Ethereum(EthereumNodeError::IncompleteHeader)),
		}
	}

	async fn transaction_receipt(&mut self, transaction_hash: H256) -> Result<Receipt> {
		let receipt = Ethereum::get_transaction_receipt(&mut self.client, transaction_hash).await?;

		match receipt.gas_used {
			Some(_) => Ok(receipt),
			None => Err(RpcError::Ethereum(EthereumNodeError::IncompleteReceipt)),
		}
	}

	async fn account_nonce(&mut self, address: Address) -> Result<U256> {
		Ok(Ethereum::get_transaction_count(&mut self.client, address).await?)
	}

	async fn submit_transaction(&mut self, signed_raw_tx: SignedRawTx) -> Result<TransactionHash> {
		let transaction = Bytes(signed_raw_tx);
		Ok(Ethereum::submit_transaction(&mut self.client, transaction).await?)
	}

	async fn eth_call(&mut self, call_transaction: CallRequest) -> Result<Bytes> {
		Ok(Ethereum::call(&mut self.client, call_transaction).await?)
	}
}

#[async_trait]
pub trait HigherLevelCalls: EthereumRpc {
	/// Returns best Substrate block that PoA chain knows of.
	async fn best_substrate_block(&mut self, contract_address: Address) -> Result<SubstrateHeaderId>;

	/// Returns true if Substrate header is known to Ethereum node.
	async fn substrate_header_known(
		&mut self,
		contract_address: Address,
		id: SubstrateHeaderId,
	) -> Result<(SubstrateHeaderId, bool)>;

	/// Submits Substrate headers to Ethereum contract.
	async fn submit_substrate_headers(
		&mut self,
		params: EthereumSigningParams,
		contract_address: Address,
		headers: Vec<QueuedSubstrateHeader>,
	) -> Result<Vec<SubstrateHeaderId>>;

	/// Returns ids of incomplete Substrate headers.
	async fn incomplete_substrate_headers(&mut self, contract_address: Address) -> Result<HashSet<SubstrateHeaderId>>;

	/// Complete Substrate header.
	async fn complete_substrate_header(
		&mut self,
		params: EthereumSigningParams,
		contract_address: Address,
		id: SubstrateHeaderId,
		justification: GrandpaJustification,
	) -> Result<SubstrateHeaderId>;

	/// Submit ethereum transaction.
	async fn submit_ethereum_transaction(
		&mut self,
		params: &EthereumSigningParams,
		contract_address: Option<Address>,
		nonce: Option<U256>,
		double_gas: bool,
		encoded_call: Vec<u8>,
	) -> Result<()>;

	/// Retrieve transactions receipts for given block.
	async fn transactions_receipts(
		&mut self,
		id: EthereumHeaderId,
		transactions: Vec<H256>,
	) -> Result<(EthereumHeaderId, Vec<Receipt>)>;
}

#[async_trait]
impl HigherLevelCalls for EthereumRpcClient {
	async fn best_substrate_block(&mut self, contract_address: Address) -> Result<SubstrateHeaderId> {
		let (encoded_call, call_decoder) = bridge_contract::functions::best_known_header::call();
		let call_request = CallRequest {
			to: Some(contract_address),
			data: Some(encoded_call.into()),
			..Default::default()
		};

		let call_result = self.eth_call(call_request).await?;

		let (number, raw_hash) = match call_decoder.decode(&call_result.0) {
			Ok((raw_number, raw_hash)) => (raw_number, raw_hash),
			Err(error) => return todo!(), // Err(Error::ResponseParseFailed(format!("{}", error))),
		};

		let hash = match SubstrateHash::decode(&mut &raw_hash[..]) {
			Ok(hash) => hash,
			Err(error) => return todo!(), // Err(Error::ResponseParseFailed(format!("{}", error))),
		};

		if number != number.low_u32().into() {
			return todo!(); // Err(Error::InvalidSubstrateBlockNumber));
		}

		Ok(HeaderId(number.low_u32(), hash))
	}

	async fn substrate_header_known(
		&mut self,
		contract_address: Address,
		id: SubstrateHeaderId,
	) -> Result<(SubstrateHeaderId, bool)> {
		let (encoded_call, call_decoder) = bridge_contract::functions::is_known_header::call(id.1);
		let call_request = CallRequest {
			to: Some(contract_address),
			data: Some(encoded_call.into()),
			..Default::default()
		};

		let call_result = self.eth_call(call_request).await?;
		match call_decoder.decode(&call_result.0) {
			Ok(is_known_block) => Ok((id, is_known_block)),
			Err(error) => todo!(), //Err(Error::ResponseParseFailed(format!("{}", error))),
		}
	}

	async fn submit_substrate_headers(
		&mut self,
		params: EthereumSigningParams,
		contract_address: Address,
		headers: Vec<QueuedSubstrateHeader>,
	) -> Result<Vec<SubstrateHeaderId>> {
		let address: Address = params.signer.address().as_fixed_bytes().into();
		let mut nonce = self.account_nonce(address).await?;

		let ids = headers.iter().map(|header| header.id()).collect();
		for header in headers {
			self.submit_ethereum_transaction(
				&params,
				Some(contract_address),
				Some(nonce),
				false,
				bridge_contract::functions::import_header::encode_input(header.header().encode()),
			)
			.await
			.expect("TODO");

			nonce += 1.into();
		}

		Ok(ids)
	}

	async fn incomplete_substrate_headers(&mut self, contract_address: Address) -> Result<HashSet<SubstrateHeaderId>> {
		let (encoded_call, call_decoder) = bridge_contract::functions::incomplete_headers::call();
		let call_request = CallRequest {
			to: Some(contract_address),
			data: Some(encoded_call.into()),
			..Default::default()
		};

		let call_result = self.eth_call(call_request).await.expect("TODO");
		match call_decoder.decode(&call_result.0) {
			Ok((incomplete_headers_numbers, incomplete_headers_hashes)) => Ok(incomplete_headers_numbers
				.into_iter()
				.zip(incomplete_headers_hashes)
				.filter_map(|(number, hash)| {
					if number != number.low_u32().into() {
						return None;
					}

					Some(HeaderId(number.low_u32(), hash))
				})
				.collect()),
			Err(error) => todo!(), // Err(Error::ResponseParseFailed(format!("{}", error))),
		}
	}

	async fn complete_substrate_header(
		&mut self,
		params: EthereumSigningParams,
		contract_address: Address,
		id: SubstrateHeaderId,
		justification: GrandpaJustification,
	) -> Result<SubstrateHeaderId> {
		let _ = self
			.submit_ethereum_transaction(
				&params,
				Some(contract_address),
				None,
				false,
				bridge_contract::functions::import_finality_proof::encode_input(id.0, id.1, justification),
			)
			.await
			.expect("TDOO");

		Ok(id)
	}

	async fn submit_ethereum_transaction(
		&mut self,
		params: &EthereumSigningParams,
		contract_address: Option<Address>,
		nonce: Option<U256>,
		double_gas: bool,
		encoded_call: Vec<u8>,
	) -> Result<()> {
		let address: Address = params.signer.address().as_fixed_bytes().into();
		let nonce = self.account_nonce(address).await.expect("TODO");

		let call_request = CallRequest {
			to: contract_address,
			data: Some(encoded_call.clone().into()),
			..Default::default()
		};
		let gas = self.estimate_gas(call_request).await.expect("TODO");

		let raw_transaction = ethereum_tx_sign::RawTransaction {
			nonce,
			to: contract_address,
			value: U256::zero(),
			gas: if double_gas { gas.saturating_mul(2.into()) } else { gas },
			gas_price: params.gas_price,
			data: encoded_call,
		}
		.sign(&params.signer.secret().as_fixed_bytes().into(), &params.chain_id);

		let _ = self.submit_transaction(raw_transaction).await.expect("TODO");
		Ok(())
	}

	async fn transactions_receipts(
		&mut self,
		id: EthereumHeaderId,
		transactions: Vec<H256>,
	) -> Result<(EthereumHeaderId, Vec<Receipt>)> {
		let mut transactions_receipts = Vec::with_capacity(transactions.len());
		for transaction in transactions {
			let transaction_receipt = self.transaction_receipt(transaction).await.expect("TODO");
			transactions_receipts.push(transaction_receipt);
		}
		Ok((id, transactions_receipts))
	}
}

#[async_trait]
pub trait DeployContract: EthereumRpc {
	/// Deploy bridge contract.
	async fn deploy_bridge_contract(
		&mut self,
		params: &EthereumSigningParams,
		contract_code: Vec<u8>,
		initial_header: Vec<u8>,
		initial_set_id: u64,
		initial_authorities: Vec<u8>,
	) -> Result<()>;
}

#[async_trait]
impl DeployContract for EthereumRpcClient {
	async fn deploy_bridge_contract(
		&mut self,
		params: &EthereumSigningParams,
		contract_code: Vec<u8>,
		initial_header: Vec<u8>,
		initial_set_id: u64,
		initial_authorities: Vec<u8>,
	) -> Result<()> {
		self.submit_ethereum_transaction(
			params,
			None,
			None,
			false,
			bridge_contract::constructor(contract_code, initial_header, initial_set_id, initial_authorities),
		)
		.await
	}
}
