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

//! Substrate -> Ethereum synchronization.

use crate::ethereum_client::{
	self, EthereumConnectionParams, EthereumRpcClient, EthereumSigningParams, HigherLevelCalls,
};
use crate::ethereum_types::Address;
use crate::rpc::{EthereumRpc, SubstrateRpc};
use crate::substrate_client::{self, AlsoHigherLevelCalls, SubstrateConnectionParams, SubstrateRpcClient};
use crate::substrate_types::{
	GrandpaJustification, Hash, Header, Number, QueuedSubstrateHeader, SubstrateHeaderId, SubstrateHeadersSyncPipeline,
};
use crate::sync::{HeadersSyncParams, TargetTransactionMode};
use crate::sync_loop::{OwnedSourceFutureOutput, OwnedTargetFutureOutput, SourceClient, TargetClient};
use crate::sync_types::SourceHeader;

use async_trait::async_trait;
use futures::future::FutureExt;
use std::{collections::HashSet, time::Duration};

/// Interval at which we check new Substrate headers when we are synced/almost synced.
const SUBSTRATE_TICK_INTERVAL: Duration = Duration::from_secs(10);
/// Interval at which we check new Ethereum blocks.
const ETHEREUM_TICK_INTERVAL: Duration = Duration::from_secs(5);
/// Max Ethereum headers we want to have in all 'before-submitted' states.
const MAX_FUTURE_HEADERS_TO_DOWNLOAD: usize = 8;
/// Max Ethereum headers count we want to have in 'submitted' state.
const MAX_SUBMITTED_HEADERS: usize = 4;
/// Max depth of in-memory headers in all states. Past this depth they will be forgotten (pruned).
const PRUNE_DEPTH: u32 = 256;

/// Substrate synchronization parameters.
#[derive(Debug)]
pub struct SubstrateSyncParams {
	/// Ethereum connection params.
	pub eth: EthereumConnectionParams,
	/// Ethereum signing params.
	pub eth_sign: EthereumSigningParams,
	/// Ethereum bridge contract address.
	pub eth_contract_address: Address,
	/// Substrate connection params.
	pub sub: SubstrateConnectionParams,
	/// Synchronization parameters.
	pub sync_params: HeadersSyncParams,
}

impl Default for SubstrateSyncParams {
	fn default() -> Self {
		SubstrateSyncParams {
			eth: Default::default(),
			eth_sign: Default::default(),
			// the address 0x731a10897d267e19b34503ad902d0a29173ba4b1 is the address
			// of the contract that is deployed by default signer and 0 nonce
			eth_contract_address: "731a10897d267e19b34503ad902d0a29173ba4b1"
				.parse()
				.expect("address is hardcoded, thus valid; qed"),
			sub: Default::default(),
			sync_params: HeadersSyncParams {
				max_future_headers_to_download: MAX_FUTURE_HEADERS_TO_DOWNLOAD,
				max_headers_in_submitted_status: MAX_SUBMITTED_HEADERS,
				// since we always have single Substrate header in separate Ethereum transaction,
				// all max_**_in_single_submit aren't important here
				max_headers_in_single_submit: 4,
				max_headers_size_in_single_submit: std::usize::MAX,
				prune_depth: PRUNE_DEPTH,
				target_tx_mode: TargetTransactionMode::Signed,
			},
		}
	}
}

/// Substrate client as headers source.
struct SubstrateHeadersSource {
	/// Substrate node client.
	client: SubstrateRpcClient, // substrate_client::Client,
}

type SubstrateFutureOutput<T> = OwnedSourceFutureOutput<SubstrateHeadersSource, SubstrateHeadersSyncPipeline, T>;

#[async_trait]
impl SourceClient<SubstrateHeadersSyncPipeline> for SubstrateHeadersSource {
	type Error = substrate_client::Error;

	// TODO: Fix error
	async fn best_block_number(&mut self) -> Result<Number, Self::Error> {
		match self.client.best_header().await {
			Ok(h) => Ok(h.number),
			Err(e) => Err(substrate_client::Error::RequestNotFound),
		}
	}

	async fn header_by_hash(&mut self, hash: Hash) -> Result<Header, Self::Error> {
		self.client
			.header_by_hash(hash)
			.await
			.map_err(|_| substrate_client::Error::RequestNotFound)
	}

	async fn header_by_number(&mut self, number: Number) -> Result<Header, Self::Error> {
		self.client
			.header_by_number(number)
			.await
			.map_err(|_| substrate_client::Error::RequestNotFound)
	}

	async fn header_completion(
		&mut self,
		id: SubstrateHeaderId,
	) -> Result<(SubstrateHeaderId, Option<GrandpaJustification>), Self::Error> {
		self.client
			.grandpa_justification(id)
			.await
			.map_err(|_| substrate_client::Error::RequestNotFound)
	}

	async fn header_extra(
		self,
		id: SubstrateHeaderId,
		_header: QueuedSubstrateHeader,
	) -> Result<(SubstrateHeaderId, ()), Self::Error> {
		Ok((id, ()))
	}
}

/// Ethereum client as Substrate headers target.
struct EthereumHeadersTarget {
	/// Ethereum node client.
	client: EthereumRpcClient, // ethereum_client::Client,
	/// Bridge contract address.
	contract: Address,
	/// Ethereum signing params.
	sign_params: EthereumSigningParams,
}

type EthereumFutureOutput<T> = OwnedTargetFutureOutput<EthereumHeadersTarget, SubstrateHeadersSyncPipeline, T>;

#[async_trait]
impl TargetClient<SubstrateHeadersSyncPipeline> for EthereumHeadersTarget {
	type Error = ethereum_client::Error;

	async fn best_header_id(self) -> EthereumFutureOutput<SubstrateHeaderId> {
		let (contract, sign_params) = (self.contract, self.sign_params);
		self.client.best_substrate_block(contract).await.expect("TOD");
		// .map(move |(client, result)| {
		// 	(
		// 		EthereumHeadersTarget {
		// 			client,
		// 			contract,
		// 			sign_params,
		// 		},
		// 		result,
		// 	)
		// })
		// .await
	}

	async fn is_known_header(self, id: SubstrateHeaderId) -> EthereumFutureOutput<(SubstrateHeaderId, bool)> {
		let (contract, sign_params) = (self.contract, self.sign_params);
		self.client.substrate_header_known(contract, id).await.expect("D")
		// .map(move |(client, result)| {
		// 	(
		// 		EthereumHeadersTarget {
		// 			client,
		// 			contract,
		// 			sign_params,
		// 		},
		// 		result,
		// 	)
		// })
		// .await
	}

	async fn submit_headers(self, headers: Vec<QueuedSubstrateHeader>) -> EthereumFutureOutput<Vec<SubstrateHeaderId>> {
		let (contract, sign_params) = (self.contract, self.sign_params);
		self.client
			.submit_substrate_headers(sign_params.clone(), contract, headers)
			.await
			.expect("TO")
		// .map(move |(client, result)| {
		// 	(
		// 		EthereumHeadersTarget {
		// 			client,
		// 			contract,
		// 			sign_params,
		// 		},
		// 		result,
		// 	)
		// })
		// .await
	}

	async fn incomplete_headers_ids(self) -> EthereumFutureOutput<HashSet<SubstrateHeaderId>> {
		let (contract, sign_params) = (self.contract, self.sign_params);
		self.client.incomplete_substrate_headers(contract).await.expect("TOD")
		// .map(move |(client, result)| {
		// 	(
		// 		EthereumHeadersTarget {
		// 			client,
		// 			contract,
		// 			sign_params,
		// 		},
		// 		result,
		// 	)
		// })
		// .await
	}

	async fn complete_header(
		self,
		id: SubstrateHeaderId,
		completion: GrandpaJustification,
	) -> EthereumFutureOutput<SubstrateHeaderId> {
		let (contract, sign_params) = (self.contract, self.sign_params);
		self.client
			.complete_substrate_header(sign_params.clone(), contract, id, completion)
			.await
			.expect("TD")
		// .map(move |(client, result)| {
		// 	(
		// 		EthereumHeadersTarget {
		// 			client,
		// 			contract,
		// 			sign_params,
		// 		},
		// 		result,
		// 	)
		// })
		// .await
	}

	async fn requires_extra(self, header: QueuedSubstrateHeader) -> EthereumFutureOutput<(SubstrateHeaderId, bool)> {
		(self, Ok((header.header().id(), false)))
	}
}

/// Run Substrate headers synchronization.
pub fn run(params: SubstrateSyncParams) {
	let mut eth_client = EthereumRpcClient::new(params.eth);
	let mut sub_client = SubstrateRpcClient::new(params.sub);

	crate::sync_loop::run(
		SubstrateHeadersSource { client: sub_client },
		SUBSTRATE_TICK_INTERVAL,
		EthereumHeadersTarget {
			client: eth_client,
			contract: params.eth_contract_address,
			sign_params: params.eth_sign,
		},
		ETHEREUM_TICK_INTERVAL,
		params.sync_params,
	);
}
