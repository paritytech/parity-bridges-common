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

use crate::ethereum_types::{Address, Bytes, EthereumHeaderId, Header, Receipt, H256, U256, U64};
use crate::substrate_types::{SubstrateHeaderId, Hash as SubstrateHash, Number as SubstrateNumber, QueuedSubstrateHeader};
use crate::sync_types::{HeaderId, MaybeConnectionError};
use codec::{Encode, Decode};
use ethabi::FunctionOutputDecoder;
use jsonrpsee::common::Params;
use jsonrpsee::raw::{RawClient, RawClientError};
use jsonrpsee::transport::http::{HttpTransportClient, RequestError};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{from_value, to_value};

// to encode/decode contract calls
ethabi_contract::use_contract!(bridge_contract, "res/substrate-bridge.json");

/// Proof of hash serialization success.
const HASH_SERIALIZATION_PROOF: &'static str = "hash serialization never fails; qed";
/// Proof of integer serialization success.
const INT_SERIALIZATION_PROOF: &'static str = "integer serialization never fails; qed";
/// Proof of bool serialization success.
const BOOL_SERIALIZATION_PROOF: &'static str = "bool serialization never fails; qed";

/// Ethereum client type.
pub type Client = RawClient<HttpTransportClient>;

/// Ethereum contract call request.
#[derive(Debug, Default, PartialEq, Serialize)]
pub struct CallRequest {
	/// Contract address.
	pub to: Option<Address>,
	/// Call data.
	pub data: Option<Bytes>,
}

/// All possible errors that can occur during interacting with Ethereum node.
#[derive(Debug)]
pub enum Error {
	/// Request start failed.
	StartRequestFailed(RequestError),
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
	/// Justification for last Substrate header is missing.
	MissingJustification,
}

impl MaybeConnectionError for Error {
	fn is_connection_error(&self) -> bool {
		match *self {
			Error::StartRequestFailed(_) | Error::ResponseRetrievalFailed(_) => true,
			_ => false,
		}
	}
}

/// Returns client that is able to call RPCs on Ethereum node.
pub fn client(uri: &str) -> Client {
	let transport = HttpTransportClient::new(uri);
	RawClient::new(transport)
}

/// Retrieve best known block number from Ethereum node.
pub async fn best_block_number(client: Client) -> (Client, Result<u64, Error>) {
	let (client, result) = call_rpc::<U64>(client, "eth_blockNumber", Params::None).await;
	(client, result.map(|x| x.as_u64()))
}

/// Retrieve block header by its number from Ethereum node.
pub async fn header_by_number(client: Client, number: u64) -> (Client, Result<Header, Error>) {
	let (client, header) = call_rpc(
		client,
		"eth_getBlockByNumber",
		Params::Array(vec![
			to_value(U64::from(number)).expect(INT_SERIALIZATION_PROOF),
			to_value(false).expect(BOOL_SERIALIZATION_PROOF),
		]),
	)
	.await;
	(
		client,
		header.and_then(
			|header: Header| match header.number.is_some() && header.hash.is_some() {
				true => Ok(header),
				false => Err(Error::IncompleteHeader),
			},
		),
	)
}

/// Retrieve block header by its hash from Ethereum node.
pub async fn header_by_hash(client: Client, hash: H256) -> (Client, Result<Header, Error>) {
	let (client, header) = call_rpc(
		client,
		"eth_getBlockByHash",
		Params::Array(vec![
			to_value(hash).expect(HASH_SERIALIZATION_PROOF),
			to_value(false).expect(BOOL_SERIALIZATION_PROOF),
		]),
	)
	.await;
	(
		client,
		header.and_then(
			|header: Header| match header.number.is_none() && header.hash.is_none() {
				true => Ok(header),
				false => Err(Error::IncompleteHeader),
			},
		),
	)
}

/// Retrieve transactions receipts for given block.
pub async fn transactions_receipts(
	mut client: Client,
	id: EthereumHeaderId,
	transacactions: Vec<H256>,
) -> (Client, Result<(EthereumHeaderId, Vec<Receipt>), Error>) {
	let mut transactions_receipts = Vec::with_capacity(transacactions.len());
	for transacaction in transacactions {
		let (next_client, transaction_receipt) = transaction_receipt(client, transacaction).await;
		let transaction_receipt = match transaction_receipt {
			Ok(transaction_receipt) => transaction_receipt,
			Err(error) => return (next_client, Err(error)),
		};
		transactions_receipts.push(transaction_receipt);
		client = next_client;
	}
	(client, Ok((id, transactions_receipts)))
}

/// Retrieve transaction receipt by transaction hash.
async fn transaction_receipt(client: Client, hash: H256) -> (Client, Result<Receipt, Error>) {
	let (client, receipt) = call_rpc::<Receipt>(
		client,
		"eth_getTransactionReceipt",
		Params::Array(vec![to_value(hash).expect(HASH_SERIALIZATION_PROOF)]),
	)
	.await;
	(
		client,
		receipt.and_then(|receipt| match receipt.gas_used.is_some() {
			true => Ok(receipt),
			false => Err(Error::IncompleteReceipt),
		}),
	)
}

/// Returns best Substrate block that PoA chain knows of.
pub async fn best_substrate_block(
	client: Client,
	contract_address: Address,
) -> (Client, Result<SubstrateHeaderId, Error>) {
	let (encoded_call, call_decoder) = bridge_contract::functions::best_known_header::call();
	let (client, result) = call_rpc::<Bytes>(
		client,
		"eth_call",
		Params::Array(vec![
			to_value(CallRequest {
				to: Some(contract_address),
				data: Some(encoded_call.into()),
			}).unwrap(),
		]),
	)
	.await;
	let call_result = match result {
		Ok(result) => result,
		Err(error) => return (client, Err(error)),
	};
	let (raw_number, raw_hash) = match call_decoder.decode(&call_result.0) {
		Ok((raw_number, raw_hash)) => (raw_number, raw_hash),
		Err(error) => return (client, Err(Error::ResponseParseFailed(format!("{}", error)))),
	};
	let number = match SubstrateNumber::decode(&mut &raw_number[..]) {
		Ok(number) => number,
		Err(error) => return (client, Err(Error::ResponseParseFailed(format!("{}", error)))),
	};
	let hash = match SubstrateHash::decode(&mut &raw_hash[..]) {
		Ok(hash) => hash,
		Err(error) => return (client, Err(Error::ResponseParseFailed(format!("{}", error)))),
	};

	(
		client,
		Ok(HeaderId(number, hash)),
	)
}

/// Returns true if Substrate header is known to Ethereum node.
pub async fn substrate_header_known(
	client: Client,
	contract_address: Address,
	id: SubstrateHeaderId,
) -> (Client, Result<(SubstrateHeaderId, bool), Error>) {
	// Ethereum contract could prune old headers. So this fn could return false even
	// if header is synced. And we'll mark corresponding Ethereum header as Orphan.
	//
	// But when we'll read best header from Ethereum next time, we will know that
	// there's a better header => this Orphan will either be marked as synced, or
	// eventually pruned.
	let (encoded_call, call_decoder) = bridge_contract::functions::is_known_header::call(id.1.encode());
	let (client, result) = call_rpc::<Bytes>(
		client,
		"eth_call",
		Params::Array(vec![
			to_value(CallRequest {
				to: Some(contract_address),
				data: Some(encoded_call.into()),
			}).unwrap(),
		]),
	)
	.await;
	let call_result = match result {
		Ok(result) => result,
		Err(error) => return (client, Err(error)),
	};
	match call_decoder.decode(&call_result.0) {
		Ok(is_known_block) => (client, Ok((id, is_known_block))),
		Err(error) => (client, Err(Error::ResponseParseFailed(format!("{}", error)))),
	}
}

/// Submits Substrate headers to Ethereum contract.
pub async fn submit_substrate_headers(
	client: Client,
	signer: parity_crypto::publickey::KeyPair,
	chain_id: u64,
	contract_address: Address,
	gas_price: U256,
	headers: Vec<QueuedSubstrateHeader>,
) -> (Client, Result<(H256, Vec<SubstrateHeaderId>), Error>) {
	let ids = headers.iter().map(|header| header.id()).collect();
	let num_headers = headers.len();
	let (headers, justification) = headers
		.into_iter()
		.fold((Vec::with_capacity(num_headers), None), |(mut headers, _), header| {
			let (header, justification) = header.extract();
			headers.push(header.encode());
			(headers, justification)
		});
	let justification = match justification {
		Some(justification) => justification.encode(),
		None => return (client, Err(Error::MissingJustification)),
	};
	let encoded_call = bridge_contract::functions::import_headers::encode_input(
		headers.encode(),
		justification,
	);
	let (client, nonce) = account_nonce(client, signer.address().as_fixed_bytes().into()).await;
	let nonce = match nonce {
		Ok(nonce) => nonce,
		Err(error) => return (client, Err(error)),
	};
	let (client, gas) = estimate_gas(client, CallRequest {
		to: Some(contract_address),
		data: Some(encoded_call.clone().into()),
	}).await;
	let gas = match gas {
		Ok(gas) => gas,
		Err(error) => return (client, Err(error)),
	};
	let raw_transaction = ethereum_tx_sign::RawTransaction {
		nonce,
		to: Some(contract_address),
		value: U256::zero(),
		gas,
		gas_price,
		data: encoded_call,
	}.sign(&signer.secret().as_fixed_bytes().into(), &chain_id);
	let (client, result) = call_rpc(
		client,
		"eth_submitTransaction",
		Params::Array(vec![
			to_value(raw_transaction).unwrap(),
		]),
	)
	.await;
	(client, result.map(|tx_hash| (tx_hash, ids)))
}

/// Deploy bridge contract.
pub async fn deploy_bridge_contract(
	client: Client,
	signer: parity_crypto::publickey::KeyPair,
	chain_id: u64,
	gas_price: U256,
	contract_code: Vec<u8>,
	initial_header: Vec<u8>,
	initial_set_id: u64,
	initial_authorities: Vec<u8>,
) -> (Client, Result<H256, Error>) {
	let encoded_call = bridge_contract::constructor(
		contract_code,
		vec![initial_header].encode(),
		initial_set_id,
		initial_authorities,
	);
	let (client, nonce) = account_nonce(client, signer.address().as_fixed_bytes().into()).await;
	let nonce = match nonce {
		Ok(nonce) => nonce,
		Err(error) => return (client, Err(error)),
	};
	let (client, gas) = estimate_gas(client, CallRequest {
		data: Some(encoded_call.clone().into()),
		..Default::default()
	}).await;
	let gas = match gas {
		Ok(gas) => gas,
		Err(error) => return (client, Err(error)),
	};
	let raw_transaction = ethereum_tx_sign::RawTransaction {
		nonce: nonce,
		to: None,
		value: U256::zero(),
		gas,
		gas_price,
		data: encoded_call,
	}.sign(&signer.secret().as_fixed_bytes().into(), &chain_id);
	call_rpc(
		client,
		"eth_submitTransaction",
		Params::Array(vec![
			to_value(Bytes(raw_transaction)).unwrap(),
		]),
	)
	.await
}

/// Get account nonce.
async fn account_nonce(
	client: Client,
	caller_address: Address,
) -> (Client, Result<U256, Error>) {
	call_rpc(client, "eth_getTransactionCount", Params::Array(vec![
		to_value(caller_address).unwrap(),
	])).await
}

/// Estimate gas usage for call.
async fn estimate_gas(
	client: Client,
	call_request: CallRequest,
) -> (Client, Result<U256, Error>) {
	call_rpc(client, "eth_estimateGas", Params::Array(vec![
		to_value(call_request).unwrap(),
	])).await
}

/// Calls RPC on Ethereum node.
async fn call_rpc<T: DeserializeOwned>(
	mut client: Client,
	method: &'static str,
	params: Params,
) -> (Client, Result<T, Error>) {
	async fn do_call_rpc<T: DeserializeOwned>(
		client: &mut Client,
		method: &'static str,
		params: Params,
	) -> Result<T, Error> {
		let request_id = client
			.start_request(method, params)
			.await
			.map_err(Error::StartRequestFailed)?;
		// WARN: if there'll be need for executing >1 request at a time, we should avoid
		// calling request_by_id
		let response = client
			.request_by_id(request_id)
			.ok_or(Error::RequestNotFound)?
			.await
			.map_err(Error::ResponseRetrievalFailed)?;
		from_value(response).map_err(|e| Error::ResponseParseFailed(format!("{}", e)))
	}

	let result = do_call_rpc(&mut client, method, params).await;
	(client, result)
}
