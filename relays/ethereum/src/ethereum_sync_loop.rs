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

//! Ethereum PoA -> Substrate synchronization.

use crate::ethereum_client::{self, EthereumConnectionParams, EthereumRpcClient, HigherLevelCalls};
use crate::ethereum_types::{EthereumHeaderId, EthereumHeadersSyncPipeline, Header, QueuedEthereumHeader, Receipt};
use crate::rpc::{EthereumRpc, SubstrateRpc};
use crate::substrate_client::{
	self, AlsoHigherLevelCalls, SubstrateConnectionParams, SubstrateRpcClient, SubstrateSigningParams,
};
use crate::sync::{HeadersSyncParams, TargetTransactionMode};
use crate::sync_loop::{OwnedSourceFutureOutput, OwnedTargetFutureOutput, SourceClient, TargetClient};

use async_trait::async_trait;
use std::{collections::HashSet, time::Duration};
use web3::types::H256;

/// Interval at which we check new Ethereum headers when we are synced/almost synced.
const ETHEREUM_TICK_INTERVAL: Duration = Duration::from_secs(10);
/// Interval at which we check new Substrate blocks.
const SUBSTRATE_TICK_INTERVAL: Duration = Duration::from_secs(5);
/// Max number of headers in single submit transaction.
const MAX_HEADERS_IN_SINGLE_SUBMIT: usize = 32;
/// Max total size of headers in single submit transaction. This only affects signed
/// submissions, when several headers are submitted at once. 4096 is the maximal **expected**
/// size of the Ethereum header + transactions receipts (if they're required).
const MAX_HEADERS_SIZE_IN_SINGLE_SUBMIT: usize = MAX_HEADERS_IN_SINGLE_SUBMIT * 4096;
/// Max Ethereum headers we want to have in all 'before-submitted' states.
const MAX_FUTURE_HEADERS_TO_DOWNLOAD: usize = 128;
/// Max Ethereum headers count we want to have in 'submitted' state.
const MAX_SUBMITTED_HEADERS: usize = 128;
/// Max depth of in-memory headers in all states. Past this depth they will be forgotten (pruned).
const PRUNE_DEPTH: u32 = 4096;

/// Ethereum synchronization parameters.
pub struct EthereumSyncParams {
	/// Ethereum connection params.
	pub eth: EthereumConnectionParams,
	/// Substrate connection params.
	pub sub: SubstrateConnectionParams,
	/// Substrate signing params.
	pub sub_sign: SubstrateSigningParams,
	/// Synchronization parameters.
	pub sync_params: HeadersSyncParams,
}

impl Default for EthereumSyncParams {
	fn default() -> Self {
		EthereumSyncParams {
			eth: Default::default(),
			sub: Default::default(),
			sub_sign: Default::default(),
			sync_params: HeadersSyncParams {
				max_future_headers_to_download: MAX_FUTURE_HEADERS_TO_DOWNLOAD,
				max_headers_in_submitted_status: MAX_SUBMITTED_HEADERS,
				max_headers_in_single_submit: MAX_HEADERS_IN_SINGLE_SUBMIT,
				max_headers_size_in_single_submit: MAX_HEADERS_SIZE_IN_SINGLE_SUBMIT,
				prune_depth: PRUNE_DEPTH,
				target_tx_mode: TargetTransactionMode::Signed,
			},
		}
	}
}

/// Ethereum client as headers source.
struct EthereumHeadersSource {
	/// Ethereum node client.
	client: EthereumRpcClient,
}

// pub type OwnedSourceFutureOutput<Client, P, T> = (Client, Result<T, <Client as SourceClient<P>>::Error>);
type EthereumFutureOutput<T> = OwnedSourceFutureOutput<EthereumHeadersSource, EthereumHeadersSyncPipeline, T>;
// type EthereumFutureOutput<T> = OwnedSourceFutureOutput<EthereumHeadersSyncPipeline, T>;

#[async_trait]
impl SourceClient<EthereumHeadersSyncPipeline> for EthereumHeadersSource {
	type Error = ethereum_client::Error;

	// TODO: Fix error
	async fn best_block_number(&mut self) -> Result<u64, Self::Error> {
		self.client
			.best_block_number()
			.await
			.map_err(|_| ethereum_client::Error::RequestNotFound)
	}

	async fn header_by_hash(&mut self, hash: H256) -> Result<Header, Self::Error> {
		self.client
			.header_by_hash(hash)
			.await
			.map_err(|_| ethereum_client::Error::RequestNotFound)
	}

	async fn header_by_number(&mut self, number: u64) -> Result<Header, Self::Error> {
		self.client
			.header_by_number(number)
			.await
			.map_err(|_| ethereum_client::Error::RequestNotFound)
	}

	async fn header_completion(&mut self, id: EthereumHeaderId) -> Result<(EthereumHeaderId, Option<()>), Self::Error> {
		Ok((id, None))
	}

	async fn header_extra(
		&mut self,
		id: EthereumHeaderId,
		header: QueuedEthereumHeader,
	) -> Result<(EthereumHeaderId, Vec<Receipt>), Self::Error> {
		self.client
			.transactions_receipts(id, header.header().transactions.clone())
			.await
			.map_err(|_| ethereum_client::Error::RequestNotFound)
	}
}

/// Substrate client as Ethereum headers target.
struct SubstrateHeadersTarget {
	/// Substrate node client.
	client: SubstrateRpcClient, // substrate_client::Client,
	/// Whether we want to submit signed (true), or unsigned (false) transactions.
	sign_transactions: bool,
	/// Substrate signing params.
	sign_params: SubstrateSigningParams,
}

type SubstrateFutureOutput<T> = OwnedTargetFutureOutput<SubstrateHeadersTarget, EthereumHeadersSyncPipeline, T>;

#[async_trait]
impl TargetClient<EthereumHeadersSyncPipeline> for SubstrateHeadersTarget {
	type Error = substrate_client::Error;

	async fn best_header_id(self) -> Result<EthereumHeaderId, Self::Error> {
		let (sign_transactions, sign_params) = (self.sign_transactions, self.sign_params);
		self.client
			.best_ethereum_block()
			.await
			.map_err(|_| substrate_client::Error::RequestNotFound)
	}

	async fn is_known_header(self, id: EthereumHeaderId) -> Result<(EthereumHeaderId, bool), Self::Error> {
		let (sign_transactions, sign_params) = (self.sign_transactions, self.sign_params);
		// TODO: Fix naming
		self.client
			.ethereum_header_known_high(id)
			.await
			.map_err(|_| substrate_client::Error::RequestNotFound)
	}

	async fn submit_headers(self, headers: Vec<QueuedEthereumHeader>) -> Result<Vec<EthereumHeaderId>, Self::Error> {
		let (sign_transactions, sign_params) = (self.sign_transactions, self.sign_params);
		self.client
			.submit_ethereum_headers(sign_params.clone(), headers, sign_transactions)
			.await
			.map_err(|e| substrate_client::Error::RequestNotFound)
	}

	async fn incomplete_headers_ids(self) -> Result<HashSet<EthereumHeaderId>, Self::Error> {
		Ok(HashSet::new())
	}

	async fn complete_header(self, id: EthereumHeaderId, _completion: ()) -> Result<EthereumHeaderId, Self::Error> {
		Ok(id)
	}

	async fn requires_extra(self, header: QueuedEthereumHeader) -> Result<(EthereumHeaderId, bool), Self::Error> {
		// we can minimize number of receipts_check calls by checking header
		// logs bloom here, but it may give us false positives (when authorities
		// source is contract, we never need any logs)
		let (sign_transactions, sign_params) = (self.sign_transactions, self.sign_params);
		// TODO: Fix name
		self.client
			.ethereum_receipts_required_high(header)
			.await
			.map_err(|e| substrate_client::Error::RequestNotFound)
	}
}

/// Run Ethereum headers synchronization.
pub fn run(params: EthereumSyncParams) {
	let mut eth_client = EthereumRpcClient::new(params.eth); // ethereum_client::client(params.eth);
	let mut sub_client = SubstrateRpcClient::new(params.sub); // substrate_client::client(params.sub);

	let sign_sub_transactions = match params.sync_params.target_tx_mode {
		TargetTransactionMode::Signed | TargetTransactionMode::Backup => true,
		TargetTransactionMode::Unsigned => false,
	};

	crate::sync_loop::run(
		EthereumHeadersSource { client: eth_client },
		ETHEREUM_TICK_INTERVAL,
		SubstrateHeadersTarget {
			client: sub_client,
			sign_transactions: sign_sub_transactions,
			sign_params: params.sub_sign,
		},
		SUBSTRATE_TICK_INTERVAL,
		params.sync_params,
	);
}
