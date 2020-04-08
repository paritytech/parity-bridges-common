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

use crate::ethereum_client;
use crate::ethereum_types::{Address, U256};
use crate::substrate_client;
use crate::substrate_types::{
	Header, Hash, Number, Justification, QueuedSubstrateHeader,
	SubstrateHeaderId, SubstrateHeadersSyncPipeline,
};
use crate::sync::HeadersSyncParams;
use crate::sync_loop::{SourceClient, TargetClient};
use crate::sync_types::SourceHeader;
use futures::future::{FutureExt, Ready, ready};
use parity_crypto::publickey::KeyPair;
use std::{future::Future, pin::Pin};

/// Interval (in ms) at which we check new Substrate headers when we are synced/almost synced.
const SUBSTRATE_TICK_INTERVAL_MS: u64 = 10_000;
/// Interval (in ms) at which we check new Ethereum blocks.
const ETHEREUM_TICK_INTERVAL_MS: u64 = 5_000;

/// Substrate synchronization parameters.
#[derive(Debug)]
pub struct SubstrateSyncParams {
	/// Ethereum RPC host.
	pub eth_host: String,
	/// Ethereum RPC port.
	pub eth_port: u16,
	/// Ethereum chain id.
	pub eth_chain_id: u64,
	/// Ethereum bridge contract address.
	pub eth_contract_address: Address,
	/// Ethereum transactions signer.
	pub eth_signer: KeyPair,
	/// Gas price we agree to pay.
	pub eth_gas_price: U256,
	/// Substrate RPC host.
	pub sub_host: String,
	/// Substrate RPC port.
	pub sub_port: u16,
	/// Synchronization parameters.
	pub sync_params: HeadersSyncParams,
}

impl Default for SubstrateSyncParams {
	fn default() -> Self {
		SubstrateSyncParams {
			eth_host: "localhost".into(),
			eth_port: 8545,
			eth_chain_id: 1, // Ethereum mainnet
			eth_contract_address: Default::default(),
			// that the account that has a lot of ether when we run instant seal engine
			// address: 0x00a329c0648769a73afac7f9381e08fb43dbea72
			// secret: 0x4d5db4107d237df6a3d58ee5f70ae63d73d7658d4026f2eefd2f204c81682cb7
			eth_signer: KeyPair::from_secret_slice(
				&[0x4d, 0x5d, 0xb4, 0x10, 0x7d, 0x23, 0x7d, 0xf6, 0xa3, 0xd5, 0x8e, 0xe5, 0xf7, 0x0a,
				0xe6, 0x3d, 0x73, 0xd7, 0x65, 0x8d, 0x40, 0x26, 0xf2, 0xee, 0xfd, 0x2f, 0x20, 0x4c,
				0x81, 0x68, 0x2c, 0xb7]
			).expect("secret is hardcoded, thus valid; qed"),
			eth_gas_price: 8_000_000_000u64.into(), // 8 Gwei
			sub_host: "localhost".into(),
			sub_port: 9933,
			sync_params: Default::default(),
		}
	}
}

/// Substrate client as headers source.
struct SubstrateHeadersSource {
	/// Substrate node client.
	client: substrate_client::Client,
}

impl SourceClient<SubstrateHeadersSyncPipeline> for SubstrateHeadersSource {
	type Error = substrate_client::Error;
	type BestBlockNumberFuture = Pin<Box<dyn Future<Output = (Self, Result<Number, Self::Error>)>>>;
	type HeaderByHashFuture = Pin<Box<dyn Future<Output = (Self, Result<Header, Self::Error>)>>>;
	type HeaderByNumberFuture = Pin<Box<dyn Future<Output = (Self, Result<Header, Self::Error>)>>>;
	type HeaderExtraFuture = Pin<Box<dyn Future<Output = (Self, Result<(SubstrateHeaderId, Option<Justification>), Self::Error>)>>>;

	fn best_block_number(self) -> Self::BestBlockNumberFuture {
		substrate_client::best_header(self.client)
			.map(|(client, result)| (
				SubstrateHeadersSource { client },
				result.map(|header| header.number)
			))
			.boxed()
	}

	fn header_by_hash(self, hash: Hash) -> Self::HeaderByHashFuture {
		substrate_client::header_by_hash(self.client, hash)
			.map(|(client, result)| (SubstrateHeadersSource { client }, result))
			.boxed()
	}

	fn header_by_number(self, number: Number) -> Self::HeaderByNumberFuture {
		substrate_client::header_by_number(self.client, number)
			.map(|(client, result)| (SubstrateHeadersSource { client }, result))
			.boxed()
	}

	fn header_extra(self, id: SubstrateHeaderId, _header: &Header) -> Self::HeaderExtraFuture {
		substrate_client::justification(self.client, id.1)
			.map(move |(client, result)| (
				SubstrateHeadersSource { client },
				result.map(|justification| (id, justification)),
			))
			.boxed()
	}
}

/// Ethereum client as Substrate headers target.
struct EthereumHeadersTarget {
	/// Ethereum node client.
	client: ethereum_client::Client,
	/// Ethereum transactions signer.
	signer: parity_crypto::publickey::KeyPair,
	/// Ethereum chain id.
	chain_id: u64,
	/// Bridge contract address.
	contract: Address,
	/// Gas price we are paying for transactions.
	gas_price: U256,
}

impl TargetClient<SubstrateHeadersSyncPipeline> for EthereumHeadersTarget {
	type Error = ethereum_client::Error;
	type BestHeaderIdFuture = Pin<Box<dyn Future<Output = (Self, Result<SubstrateHeaderId, Self::Error>)>>>;
	type IsKnownHeaderFuture = Pin<Box<dyn Future<Output = (Self, Result<(SubstrateHeaderId, bool), Self::Error>)>>>;
	type RequiresExtraFuture = Ready<(Self, Result<(SubstrateHeaderId, bool), Self::Error>)>;
	type SubmitHeadersFuture = Pin<Box<dyn Future<Output = (Self, Result<Vec<SubstrateHeaderId>, Self::Error>)>>>;

	fn best_header_id(self) -> Self::BestHeaderIdFuture {
		let (signer, chain_id, contract, gas_price)
			= (self.signer, self.chain_id, self.contract, self.gas_price);
		ethereum_client::best_substrate_block(self.client, contract)
			.map(move |(client, result)| {
				(
					EthereumHeadersTarget {
						client,
						signer,
						chain_id,
						contract,
						gas_price,
					},
					result,
				)
			})
			.boxed()
	}

	fn is_known_header(self, id: SubstrateHeaderId) -> Self::IsKnownHeaderFuture {
		let (signer, chain_id, contract, gas_price)
			= (self.signer, self.chain_id, self.contract, self.gas_price);
		ethereum_client::substrate_header_known(self.client, contract, id)
			.map(move |(client, result)| {
				(
					EthereumHeadersTarget {
						client,
						signer,
						chain_id,
						contract,
						gas_price,
					},
					result,
				)
			})
			.boxed()
	}

	fn requires_extra(self, header: &QueuedSubstrateHeader) -> Self::RequiresExtraFuture {
		// we would require justification for every header
		ready((self, Ok((header.header().id(), true))))
	}

	fn submit_headers(self, headers: Vec<QueuedSubstrateHeader>) -> Self::SubmitHeadersFuture {
		let (signer, chain_id, contract, gas_price)
			= (self.signer, self.chain_id, self.contract, self.gas_price);
		ethereum_client::submit_substrate_headers(
			self.client,
			signer.clone(),
			chain_id,
			contract,
			gas_price,
			headers,
		).map(move |(client, result)| {
			(
				EthereumHeadersTarget {
					client,
					signer,
					chain_id,
					contract,
					gas_price,
				},
				result.map(|(_, submitted_headers)| submitted_headers),
			)
		})
		.boxed()
	}
}

/// Run Substrate headers synchronization.
pub fn run(params: SubstrateSyncParams) {
	let eth_uri = format!("http://{}:{}", params.eth_host, params.eth_port);
	let eth_client = ethereum_client::client(&eth_uri);

	let sub_uri = format!("http://{}:{}", params.sub_host, params.sub_port);
	let sub_client = substrate_client::client(&sub_uri);

	crate::sync_loop::run(
		SubstrateHeadersSource { client: sub_client },
		SUBSTRATE_TICK_INTERVAL_MS,
		EthereumHeadersTarget {
			client: eth_client,
			signer: params.eth_signer,
			chain_id: params.eth_chain_id,
			contract: params.eth_contract_address,
			gas_price: params.eth_gas_price,
		},
		ETHEREUM_TICK_INTERVAL_MS,
		params.sync_params,
	);
}
