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
	Address, Bytes, CallRequest, EthereumHeaderId, Header, HeaderWithTransactions, Receipt, SignedRawTx, Transaction,
	TransactionHash, H256, U256,
};
use crate::rpc::{Ethereum, EthereumRpc};
use crate::rpc_errors::{EthereumNodeError, RpcError};
use crate::substrate_types::{GrandpaJustification, Hash as SubstrateHash, QueuedSubstrateHeader, SubstrateHeaderId};
use crate::sync_types::{HeaderId, SubmittedHeaders};
use crate::utils::MaybeConnectionError;

use async_trait::async_trait;
use codec::{Decode, Encode};
use ethabi::FunctionOutputDecoder;
use jsonrpsee::raw::RawClient;
use jsonrpsee::transport::http::HttpTransportClient;
use jsonrpsee::Client;
use parity_crypto::publickey::KeyPair;

use std::collections::{HashSet, VecDeque};

// to encode/decode contract calls
ethabi_contract::use_contract!(bridge_contract, "res/substrate-bridge-abi.json");

type Result<T> = std::result::Result<T, RpcError>;

/// Ethereum connection params.
#[derive(Debug, Clone)]
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

/// The client used to interact with an Ethereum node through RPC.
pub struct EthereumRpcClient {
	client: Client,
}

impl EthereumRpcClient {
	/// Create a new Ethereum RPC Client.
	pub fn new(params: EthereumConnectionParams) -> Self {
		let uri = format!("http://{}:{}", params.host, params.port);
		let transport = HttpTransportClient::new(&uri);
		let raw_client = RawClient::new(transport);
		let client: Client = raw_client.into();

		Self { client }
	}
}

#[async_trait]
impl EthereumRpc for EthereumRpcClient {
	async fn estimate_gas(&self, call_request: CallRequest) -> Result<U256> {
		Ok(Ethereum::estimate_gas(&self.client, call_request).await?)
	}

	async fn best_block_number(&self) -> Result<u64> {
		Ok(Ethereum::block_number(&self.client).await?.as_u64())
	}

	async fn header_by_number(&self, block_number: u64) -> Result<Header> {
		let get_full_tx_objects = false;
		let header = Ethereum::get_block_by_number(&self.client, block_number, get_full_tx_objects).await?;
		match header.number.is_some() && header.hash.is_some() && header.logs_bloom.is_some() {
			true => Ok(header),
			false => Err(RpcError::Ethereum(EthereumNodeError::IncompleteHeader)),
		}
	}

	async fn header_by_hash(&self, hash: H256) -> Result<Header> {
		let get_full_tx_objects = false;
		let header = Ethereum::get_block_by_hash(&self.client, hash, get_full_tx_objects).await?;
		match header.number.is_some() && header.hash.is_some() && header.logs_bloom.is_some() {
			true => Ok(header),
			false => Err(RpcError::Ethereum(EthereumNodeError::IncompleteHeader)),
		}
	}

	async fn header_by_number_with_transactions(&self, number: u64) -> Result<HeaderWithTransactions> {
		let get_full_tx_objects = true;
		let header = Ethereum::get_block_by_number_with_transactions(&self.client, number, get_full_tx_objects).await?;

		let is_complete_header = header.number.is_some() && header.hash.is_some() && header.logs_bloom.is_some();
		if !is_complete_header {
			return Err(RpcError::Ethereum(EthereumNodeError::IncompleteHeader));
		}

		let is_complete_transactions = header.transactions.iter().all(|tx| tx.raw.is_some());
		if !is_complete_transactions {
			return Err(RpcError::Ethereum(EthereumNodeError::IncompleteTransaction));
		}

		Ok(header)
	}

	async fn header_by_hash_with_transactions(&self, hash: H256) -> Result<HeaderWithTransactions> {
		let get_full_tx_objects = true;
		let header = Ethereum::get_block_by_hash_with_transactions(&self.client, hash, get_full_tx_objects).await?;

		let is_complete_header = header.number.is_some() && header.hash.is_some() && header.logs_bloom.is_some();
		if !is_complete_header {
			return Err(RpcError::Ethereum(EthereumNodeError::IncompleteHeader));
		}

		let is_complete_transactions = header.transactions.iter().all(|tx| tx.raw.is_some());
		if !is_complete_transactions {
			return Err(RpcError::Ethereum(EthereumNodeError::IncompleteTransaction));
		}

		Ok(header)
	}

	async fn transaction_by_hash(&self, hash: H256) -> Result<Option<Transaction>> {
		Ok(Ethereum::transaction_by_hash(&self.client, hash).await?)
	}

	async fn transaction_receipt(&self, transaction_hash: H256) -> Result<Receipt> {
		let receipt = Ethereum::get_transaction_receipt(&self.client, transaction_hash).await?;

		match receipt.gas_used {
			Some(_) => Ok(receipt),
			None => Err(RpcError::Ethereum(EthereumNodeError::IncompleteReceipt)),
		}
	}

	async fn account_nonce(&self, address: Address) -> Result<U256> {
		Ok(Ethereum::get_transaction_count(&self.client, address).await?)
	}

	async fn submit_transaction(&self, signed_raw_tx: SignedRawTx) -> Result<TransactionHash> {
		let transaction = Bytes(signed_raw_tx);
		let tx_hash = Ethereum::submit_transaction(&self.client, transaction).await?;
		log::trace!(target: "bridge", "Sent transaction to Ethereum node: {:?}", tx_hash);
		Ok(tx_hash)
	}

	async fn eth_call(&self, call_transaction: CallRequest) -> Result<Bytes> {
		Ok(Ethereum::call(&self.client, call_transaction).await?)
	}
}

/// A trait which contains methods that work by using multiple low-level RPCs, or more complicated
/// interactions involving, for example, an Ethereum contract.
#[async_trait]
pub trait EthereumHighLevelRpc: EthereumRpc {
	/// Returns best Substrate block that PoA chain knows of.
	async fn best_substrate_block(&self, contract_address: Address) -> Result<SubstrateHeaderId>;

	/// Returns true if Substrate header is known to Ethereum node.
	async fn substrate_header_known(
		&self,
		contract_address: Address,
		id: SubstrateHeaderId,
	) -> Result<(SubstrateHeaderId, bool)>;

	/// Submits Substrate headers to Ethereum contract.
	async fn submit_substrate_headers(
		&self,
		params: EthereumSigningParams,
		contract_address: Address,
		headers: Vec<QueuedSubstrateHeader>,
	) -> SubmittedHeaders<SubstrateHeaderId, RpcError>;

	/// Returns ids of incomplete Substrate headers.
	async fn incomplete_substrate_headers(&self, contract_address: Address) -> Result<HashSet<SubstrateHeaderId>>;

	/// Complete Substrate header.
	async fn complete_substrate_header(
		&self,
		params: EthereumSigningParams,
		contract_address: Address,
		id: SubstrateHeaderId,
		justification: GrandpaJustification,
	) -> Result<SubstrateHeaderId>;

	/// Submit ethereum transaction.
	async fn submit_ethereum_transaction(
		&self,
		params: &EthereumSigningParams,
		contract_address: Option<Address>,
		nonce: Option<U256>,
		double_gas: bool,
		encoded_call: Vec<u8>,
	) -> Result<()>;

	/// Retrieve transactions receipts for given block.
	async fn transaction_receipts(
		&self,
		id: EthereumHeaderId,
		transactions: Vec<H256>,
	) -> Result<(EthereumHeaderId, Vec<Receipt>)>;
}

#[async_trait]
impl EthereumHighLevelRpc for EthereumRpcClient {
	async fn best_substrate_block(&self, contract_address: Address) -> Result<SubstrateHeaderId> {
		let (encoded_call, call_decoder) = bridge_contract::functions::best_known_header::call();
		let call_request = CallRequest {
			to: Some(contract_address),
			data: Some(encoded_call.into()),
			..Default::default()
		};

		let call_result = self.eth_call(call_request).await?;
		let (number, raw_hash) = call_decoder.decode(&call_result.0)?;
		let hash = SubstrateHash::decode(&mut &raw_hash[..])?;

		if number != number.low_u32().into() {
			return Err(RpcError::Ethereum(EthereumNodeError::InvalidSubstrateBlockNumber));
		}

		Ok(HeaderId(number.low_u32(), hash))
	}

	async fn substrate_header_known(
		&self,
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
		let is_known_block = call_decoder.decode(&call_result.0)?;

		Ok((id, is_known_block))
	}

	async fn submit_substrate_headers(
		&self,
		params: EthereumSigningParams,
		contract_address: Address,
		headers: Vec<QueuedSubstrateHeader>,
	) -> SubmittedHeaders<SubstrateHeaderId, RpcError> {
		// read nonce of signer
		let address: Address = params.signer.address().as_fixed_bytes().into();
		let nonce = match self.account_nonce(address).await {
			Ok(nonce) => nonce,
			Err(error) => {
				return SubmittedHeaders {
					submitted: Vec::new(),
					incomplete: Vec::new(),
					rejected: headers.iter().rev().map(|header| header.id()).collect(),
					fatal_error: Some(error),
				}
			}
		};

		// submit headers. Note that we're cloning self here. It is ok, because
		// cloning `jsonrpsee::Client` only clones reference to background threads
		submit_substrate_headers(
			EthereumHeadersSubmitter {
				client: EthereumRpcClient {
					client: self.client.clone(),
				},
				params,
				contract_address,
				nonce,
			},
			headers,
		)
		.await
	}

	async fn incomplete_substrate_headers(&self, contract_address: Address) -> Result<HashSet<SubstrateHeaderId>> {
		let (encoded_call, call_decoder) = bridge_contract::functions::incomplete_headers::call();
		let call_request = CallRequest {
			to: Some(contract_address),
			data: Some(encoded_call.into()),
			..Default::default()
		};

		let call_result = self.eth_call(call_request).await?;

		// Q: Is is correct to call these "incomplete_ids"?
		let (incomplete_headers_numbers, incomplete_headers_hashes) = call_decoder.decode(&call_result.0)?;
		let incomplete_ids = incomplete_headers_numbers
			.into_iter()
			.zip(incomplete_headers_hashes)
			.filter_map(|(number, hash)| {
				if number != number.low_u32().into() {
					return None;
				}

				Some(HeaderId(number.low_u32(), hash))
			})
			.collect();

		Ok(incomplete_ids)
	}

	async fn complete_substrate_header(
		&self,
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
			.await?;

		Ok(id)
	}

	async fn submit_ethereum_transaction(
		&self,
		params: &EthereumSigningParams,
		contract_address: Option<Address>,
		nonce: Option<U256>,
		double_gas: bool,
		encoded_call: Vec<u8>,
	) -> Result<()> {
		let nonce = if let Some(n) = nonce {
			n
		} else {
			let address: Address = params.signer.address().as_fixed_bytes().into();
			self.account_nonce(address).await?
		};

		let call_request = CallRequest {
			to: contract_address,
			data: Some(encoded_call.clone().into()),
			..Default::default()
		};
		let gas = self.estimate_gas(call_request).await?;

		let raw_transaction = ethereum_tx_sign::RawTransaction {
			nonce,
			to: contract_address,
			value: U256::zero(),
			gas: if double_gas { gas.saturating_mul(2.into()) } else { gas },
			gas_price: params.gas_price,
			data: encoded_call,
		}
		.sign(&params.signer.secret().as_fixed_bytes().into(), &params.chain_id);

		let _ = self.submit_transaction(raw_transaction).await?;
		Ok(())
	}

	async fn transaction_receipts(
		&self,
		id: EthereumHeaderId,
		transactions: Vec<H256>,
	) -> Result<(EthereumHeaderId, Vec<Receipt>)> {
		let mut transaction_receipts = Vec::with_capacity(transactions.len());
		for transaction in transactions {
			let transaction_receipt = self.transaction_receipt(transaction).await?;
			transaction_receipts.push(transaction_receipt);
		}
		Ok((id, transaction_receipts))
	}
}

/// Substrate headers submitter API.
#[async_trait]
trait HeadersSubmitter {
	/// Returns Ok(0) if all given not-yet-imported headers are complete.
	/// Returns Ok(index != 0) where index is 1-based index of first header that is incomplete.
	///
	/// Returns Err(()) if contract has rejected headers. This means that the contract is
	/// unable to import first header (e.g. it may already be imported).
	async fn is_headers_incomplete(
		&self,
		header1: &QueuedSubstrateHeader,
		header2: Option<&QueuedSubstrateHeader>,
		header3: Option<&QueuedSubstrateHeader>,
		header4: Option<&QueuedSubstrateHeader>,
	) -> Result<usize>;

	/// Submit given headers to Ethereum node.
	async fn submit_headers(
		&mut self,
		header1: QueuedSubstrateHeader,
		header2: Option<&QueuedSubstrateHeader>,
		header3: Option<&QueuedSubstrateHeader>,
		header4: Option<&QueuedSubstrateHeader>,
	) -> Result<()>;
}

/// Implementation of Substrate headers submitter that sends headers to running Ethereum node.
struct EthereumHeadersSubmitter {
	client: EthereumRpcClient,
	params: EthereumSigningParams,
	contract_address: Address,
	nonce: U256,
}

#[async_trait]
impl HeadersSubmitter for EthereumHeadersSubmitter {
	async fn is_headers_incomplete(
		&self,
		header1: &QueuedSubstrateHeader,
		header2: Option<&QueuedSubstrateHeader>,
		header3: Option<&QueuedSubstrateHeader>,
		header4: Option<&QueuedSubstrateHeader>,
	) -> Result<usize> {
		let (encoded_call, call_decoder) = bridge_contract::functions::is_incomplete_headers::call(
			header1.header().encode(),
			header2.map(|header2| header2.header().encode()).unwrap_or_default(),
			header3.map(|header3| header3.header().encode()).unwrap_or_default(),
			header4.map(|header4| header4.header().encode()).unwrap_or_default(),
		);
		let call_request = CallRequest {
			to: Some(self.contract_address),
			data: Some(encoded_call.into()),
			..Default::default()
		};

		let call_result = self.client.eth_call(call_request).await?;
		let incomplete_index: U256 = call_decoder.decode(&call_result.0)?;
		if incomplete_index > 4.into() {
			return Err(RpcError::Ethereum(EthereumNodeError::InvalidIncompleteIndex));
		}

		Ok(incomplete_index.low_u32() as _)
	}

	async fn submit_headers(
		&mut self,
		header1: QueuedSubstrateHeader,
		header2: Option<&QueuedSubstrateHeader>,
		header3: Option<&QueuedSubstrateHeader>,
		header4: Option<&QueuedSubstrateHeader>,
	) -> Result<()> {
		let result = self
			.client
			.submit_ethereum_transaction(
				&self.params,
				Some(self.contract_address),
				Some(self.nonce),
				false,
				bridge_contract::functions::import_headers::encode_input(
					header1.header().encode(),
					header2.map(|header2| header2.header().encode()).unwrap_or_default(),
					header3.map(|header3| header3.header().encode()).unwrap_or_default(),
					header4.map(|header4| header4.header().encode()).unwrap_or_default(),
				),
			)
			.await;

		if result.is_ok() {
			self.nonce += U256::one();
		}

		result
	}
}

/// Submit multiple Substrate headers.
async fn submit_substrate_headers(
	mut header_submitter: impl HeadersSubmitter,
	mut headers: Vec<QueuedSubstrateHeader>,
) -> SubmittedHeaders<SubstrateHeaderId, RpcError> {
	let mut submitted_headers = SubmittedHeaders::default();

	let mut ids = headers.iter().map(|header| header.id()).collect::<VecDeque<_>>();
	headers.reverse();

	while !headers.is_empty() {
		let header1 = headers.pop().expect("headers is not empty; qed");
		let header2 = headers.pop();
		let header3 = headers.pop();
		let header4 = headers.pop();

		let mut submitting_ids = Vec::with_capacity(4);
		submitting_ids.push(ids.pop_front().expect("both collections have same size; qed"));
		submitting_ids.extend(ids.pop_front().iter());
		submitting_ids.extend(ids.pop_front().iter());
		submitting_ids.extend(ids.pop_front().iter());

		submitted_headers.fatal_error = submit_4_substrate_headers(
			&mut header_submitter,
			&mut submitted_headers,
			submitting_ids,
			header1,
			header2,
			header3,
			header4,
		)
		.await;

		if submitted_headers.fatal_error.is_some() {
			submitted_headers.rejected.extend(ids);
			break;
		}
	}

	submitted_headers
}

/// Submit 4 Substrate headers in single PoA transaction.
async fn submit_4_substrate_headers(
	header_submitter: &mut impl HeadersSubmitter,
	submitted_headers: &mut SubmittedHeaders<SubstrateHeaderId, RpcError>,
	mut ids: Vec<SubstrateHeaderId>,
	header1: QueuedSubstrateHeader,
	header2: Option<QueuedSubstrateHeader>,
	header3: Option<QueuedSubstrateHeader>,
	header4: Option<QueuedSubstrateHeader>,
) -> Option<RpcError> {
	debug_assert_eq!(
		ids.len(),
		1 + header2.as_ref().map(|_| 1).unwrap_or(0)
			+ header3.as_ref().map(|_| 1).unwrap_or(0)
			+ header4.as_ref().map(|_| 1).unwrap_or(0),
	);

	// if parent of first header is either incomplete, or rejected, we assume that contract
	// will reject this header as well
	let parent_id = header1.parent_id();
	if submitted_headers.rejected.contains(&parent_id) || submitted_headers.incomplete.contains(&parent_id) {
		submitted_headers.rejected.extend(ids);
		return None;
	}

	// check if headers are incomplete
	let incomplete_header_index = match header_submitter
		.is_headers_incomplete(&header1, header2.as_ref(), header3.as_ref(), header4.as_ref())
		.await
	{
		Ok(incomplete_header_index) => incomplete_header_index,
		Err(error) => {
			// contract has rejected all headers => we do not want to submit it
			submitted_headers.rejected.extend(ids);
			if error.is_connection_error() {
				return Some(error);
			} else {
				return None;
			}
		}
	};

	// submit headers that contract will accept
	let rejected = if incomplete_header_index != 0 {
		ids.split_off(std::cmp::min(incomplete_header_index, ids.len()))
	} else {
		Vec::new()
	};
	let submitted = ids;
	let submit_result = header_submitter
		.submit_headers(
			header1,
			if incomplete_header_index == 0 || incomplete_header_index >= 2 {
				header2.as_ref()
			} else {
				None
			},
			if incomplete_header_index == 0 || incomplete_header_index >= 3 {
				header3.as_ref()
			} else {
				None
			},
			if incomplete_header_index == 0 || incomplete_header_index >= 4 {
				header4.as_ref()
			} else {
				None
			},
		)
		.await;
	match submit_result {
		Ok(_) => {
			if incomplete_header_index != 0 {
				submitted_headers.incomplete.extend(submitted.iter().last().cloned());
			}
			submitted_headers.submitted.extend(submitted);
			submitted_headers.rejected.extend(rejected);
			None
		}
		Err(error) => {
			submitted_headers.rejected.extend(submitted);
			submitted_headers.rejected.extend(rejected);
			Some(error)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::substrate_types::{Header as SubstrateHeader, Number as SubstrateBlockNumber};
	use sp_runtime::traits::Header;

	struct TestHeadersSubmitter {
		incomplete: Vec<SubstrateHeaderId>,
		failed: Vec<SubstrateHeaderId>,
	}

	#[async_trait]
	impl HeadersSubmitter for TestHeadersSubmitter {
		async fn is_headers_incomplete(
			&self,
			header1: &QueuedSubstrateHeader,
			_header2: Option<&QueuedSubstrateHeader>,
			_header3: Option<&QueuedSubstrateHeader>,
			_header4: Option<&QueuedSubstrateHeader>,
		) -> Result<usize> {
			if self.incomplete.iter().any(|i| i.0 == header1.id().0) {
				Ok(1)
			} else {
				Ok(0)
			}
		}

		async fn submit_headers(
			&mut self,
			header1: QueuedSubstrateHeader,
			_header2: Option<&QueuedSubstrateHeader>,
			_header3: Option<&QueuedSubstrateHeader>,
			_header4: Option<&QueuedSubstrateHeader>,
		) -> Result<()> {
			if self.failed.iter().any(|i| i.0 == header1.id().0) {
				Err(RpcError::Ethereum(EthereumNodeError::InvalidSubstrateBlockNumber))
			} else {
				Ok(())
			}
		}
	}

	fn header(number: SubstrateBlockNumber) -> QueuedSubstrateHeader {
		QueuedSubstrateHeader::new(SubstrateHeader::new(
			number,
			Default::default(),
			Default::default(),
			if number == 0 {
				Default::default()
			} else {
				header(number - 1).id().1
			},
			Default::default(),
		))
	}

	#[test]
	fn descendants_of_incomplete_headers_are_not_submitted() {
		let submitted_headers = async_std::task::block_on(submit_substrate_headers(
			TestHeadersSubmitter {
				incomplete: vec![header(5).id()],
				failed: vec![],
			},
			vec![header(5), header(6)],
		));
		assert_eq!(submitted_headers.submitted, vec![header(5).id()]);
		assert_eq!(submitted_headers.incomplete, vec![header(5).id()]);
		assert_eq!(submitted_headers.rejected, vec![header(6).id()]);
		assert!(submitted_headers.fatal_error.is_none());
	}

	#[test]
	fn headers_after_fatal_error_are_not_submitted() {
		let submitted_headers = async_std::task::block_on(submit_substrate_headers(
			TestHeadersSubmitter {
				incomplete: vec![],
				failed: vec![header(9).id()],
			},
			vec![
				header(5),
				header(6),
				header(7),
				header(8),
				header(9),
				header(10),
				header(11),
				header(12),
				header(13),
			],
		));
		assert_eq!(
			submitted_headers.submitted,
			vec![header(5).id(), header(6).id(), header(7).id(), header(8).id()]
		);
		assert_eq!(submitted_headers.incomplete, vec![]);
		assert_eq!(
			submitted_headers.rejected,
			vec![
				header(9).id(),
				header(10).id(),
				header(11).id(),
				header(12).id(),
				header(13).id()
			]
		);
		assert!(submitted_headers.fatal_error.is_some());
	}
}
