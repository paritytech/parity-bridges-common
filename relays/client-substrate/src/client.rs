// Copyright 2019-2021 Parity Technologies (UK) Ltd.
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

//! Substrate node client.

use crate::{
	chain::{Chain, ChainWithTransactions},
	new_client::{Client as _, RpcWithCachingClient as NewRpcWithCachingClient},
	rpc::{
		SubstrateChainClient, SubstrateFinalityClient, SubstrateStateClient, SubstrateSystemClient,
	},
	AccountKeyPairOf, ConnectionParams, Error, HashOf, HeaderIdOf, Result, TransactionTracker,
	UnsignedTransaction,
};

use async_std::sync::{Arc, Mutex, RwLock};
use async_trait::async_trait;
use bp_runtime::{StorageDoubleMapKeyProvider, StorageMapKeyProvider};
use codec::{Decode, Encode};
use frame_support::weights::Weight;
use futures::{SinkExt, StreamExt};
use jsonrpsee::{
	core::DeserializeOwned,
	ws_client::{WsClient as RpcClient, WsClientBuilder as RpcClientBuilder},
};
use num_traits::{Saturating, Zero};
use relay_utils::relay_loop::RECONNECT_DELAY;
use sp_core::{
	storage::{StorageData, StorageKey},
	Bytes, Pair,
};
use sp_runtime::{traits::Header as _, transaction_validity::TransactionValidity};
use sp_trie::StorageProof;
use sp_version::RuntimeVersion;
use std::future::Future;

const SUB_API_GRANDPA_AUTHORITIES: &str = "GrandpaApi_grandpa_authorities";
const MAX_SUBSCRIPTION_CAPACITY: usize = 4096;

/// The difference between best block number and number of its ancestor, that is enough
/// for us to consider that ancestor an "ancient" block with dropped state.
///
/// The relay does not assume that it is connected to the archive node, so it always tries
/// to use the best available chain state. But sometimes it still may use state of some
/// old block. If the state of that block is already dropped, relay will see errors when
/// e.g. it tries to prove something.
///
/// By default Substrate-based nodes are storing state for last 256 blocks. We'll use
/// half of this value.
pub const ANCIENT_BLOCK_THRESHOLD: u32 = 128;

/// Returns `true` if we think that the state is already discarded for given block.
pub fn is_ancient_block<N: From<u32> + PartialOrd + Saturating>(block: N, best: N) -> bool {
	best.saturating_sub(block) >= N::from(ANCIENT_BLOCK_THRESHOLD)
}

/// Opaque justifications subscription type.
pub struct Subscription<T>(pub(crate) Mutex<futures::channel::mpsc::Receiver<Option<T>>>);

/// Opaque GRANDPA authorities set.
pub type OpaqueGrandpaAuthoritiesSet = Vec<u8>;

/// A simple runtime version. It only includes the `spec_version` and `transaction_version`.
#[derive(Copy, Clone, Debug)]
pub struct SimpleRuntimeVersion {
	/// Version of the runtime specification.
	pub spec_version: u32,
	/// All existing dispatches are fully compatible when this number doesn't change.
	pub transaction_version: u32,
}

impl SimpleRuntimeVersion {
	/// Create a new instance of `SimpleRuntimeVersion` from a `RuntimeVersion`.
	pub const fn from_runtime_version(runtime_version: &RuntimeVersion) -> Self {
		Self {
			spec_version: runtime_version.spec_version,
			transaction_version: runtime_version.transaction_version,
		}
	}
}

/// Chain runtime version in client
#[derive(Clone, Debug)]
pub enum ChainRuntimeVersion {
	/// Auto query from chain.
	Auto,
	/// Custom runtime version, defined by user.
	Custom(SimpleRuntimeVersion),
}

/// Substrate client type.
///
/// Cloning `Client` is a cheap operation that only clones internal references. Different
/// clones of the same client are guaranteed to use the same references.
pub struct Client<C: Chain> {
	// Lock order: `submit_signed_extrinsic_lock`, `data`
	/// New client implementation.
	new: NewRpcWithCachingClient<C>,
	/// Client connection params.
	params: ConnectionParams,
	/// Saved chain runtime version.
	chain_runtime_version: ChainRuntimeVersion,
	/// If several tasks are submitting their transactions simultaneously using
	/// `submit_signed_extrinsic` method, they may get the same transaction nonce. So one of
	/// transactions will be rejected from the pool. This lock is here to prevent situations like
	/// that.
	submit_signed_extrinsic_lock: Arc<Mutex<()>>,
	/// Genesis block hash.
	genesis_hash: HashOf<C>,
	/// Shared dynamic data.
	data: Arc<RwLock<ClientData>>,
}

/// Client data, shared by all `Client` clones.
struct ClientData {
	/// Tokio runtime handle.
	tokio: Arc<tokio::runtime::Runtime>,
	/// Substrate RPC client.
	client: Arc<RpcClient>,
}
/*
#[async_trait]
impl<C: Chain> relay_utils::relay_loop::Client for Client<C> {
	type Error = Error;

	async fn reconnect(&mut self) -> Result<()> {
		self.new.reconnect().await?;

		let mut data = self.data.write().await;
		let (tokio, client) = Self::build_client(&self.params).await?;
		data.tokio = tokio;
		data.client = client;
		Ok(())
	}
}
*/
impl<C: Chain> Clone for Client<C> {
	fn clone(&self) -> Self {
		Client {
			new: self.new.clone(),
			params: self.params.clone(),
			chain_runtime_version: self.chain_runtime_version.clone(),
			submit_signed_extrinsic_lock: self.submit_signed_extrinsic_lock.clone(),
			genesis_hash: self.genesis_hash,
			data: self.data.clone(),
		}
	}
}

impl<C: Chain> std::fmt::Debug for Client<C> {
	fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
		fmt.debug_struct("Client").field("genesis_hash", &self.genesis_hash).finish()
	}
}

impl<C: Chain> Client<C> {
	/// Returns client that is able to call RPCs on Substrate node over websocket connection.
	///
	/// This function will keep connecting to given Substrate node until connection is established
	/// and is functional. If attempt fail, it will wait for `RECONNECT_DELAY` and retry again.
	pub async fn new(params: ConnectionParams) -> Self {
		loop {
			match Self::try_connect(params.clone()).await {
				Ok(client) => return client,
				Err(error) => log::error!(
					target: "bridge",
					"Failed to connect to {} node: {:?}. Going to retry in {}s",
					C::NAME,
					error,
					RECONNECT_DELAY.as_secs(),
				),
			}

			async_std::task::sleep(RECONNECT_DELAY).await;
		}
	}

	/// Try to connect to Substrate node over websocket. Returns Substrate RPC client if connection
	/// has been established or error otherwise.
	pub async fn try_connect(params: ConnectionParams) -> Result<Self> {
		let (tokio, client) = Self::build_client(&params).await?;

		let number: C::BlockNumber = Zero::zero();
		let genesis_hash_client = client.clone();
		let genesis_hash = tokio
			.spawn(async move {
				SubstrateChainClient::<C>::block_hash(&*genesis_hash_client, Some(number)).await
			})
			.await??;

		let chain_runtime_version = params.chain_runtime_version.clone();
		Ok(Self {
			new: crate::new_client::rpc_with_caching(params.clone()).await,
			params,
			chain_runtime_version,
			submit_signed_extrinsic_lock: Arc::new(Mutex::new(())),
			genesis_hash,
			data: Arc::new(RwLock::new(ClientData { tokio, client })),
		})
	}

	/// Build client to use in connection.
	async fn build_client(
		params: &ConnectionParams,
	) -> Result<(Arc<tokio::runtime::Runtime>, Arc<RpcClient>)> {
		let tokio = tokio::runtime::Runtime::new()?;
		let uri = format!(
			"{}://{}:{}",
			if params.secure { "wss" } else { "ws" },
			params.host,
			params.port,
		);
		log::info!(target: "bridge", "Connecting to {} node at {}", C::NAME, uri);

		let client = tokio
			.spawn(async move {
				RpcClientBuilder::default()
					.max_buffer_capacity_per_subscription(MAX_SUBSCRIPTION_CAPACITY)
					.build(&uri)
					.await
			})
			.await??;

		Ok((Arc::new(tokio), Arc::new(client)))
	}
}

impl<C: Chain> Client<C> {
	/// Return simple runtime version, only include `spec_version` and `transaction_version`.
	pub async fn simple_runtime_version(&self) -> Result<SimpleRuntimeVersion> {
		Ok(match &self.chain_runtime_version {
			ChainRuntimeVersion::Auto => {
				let runtime_version = self.runtime_version().await?;
				SimpleRuntimeVersion::from_runtime_version(&runtime_version)
			},
			ChainRuntimeVersion::Custom(version) => *version,
		})
	}

	/// Returns true if client is connected to at least one peer and is in synced state.
	pub async fn ensure_synced(&self) -> Result<()> {
		self.new.ensure_synced().await
	}

	/// Return hash of the genesis block.
	pub fn genesis_hash(&self) -> &C::Hash {
		&self.genesis_hash
	}

	/// Return hash of the best finalized block.
	pub async fn best_finalized_header_hash(&self) -> Result<C::Hash> {
		self.new.best_finalized_header_hash().await
	}

	/// Return number of the best finalized block.
	pub async fn best_finalized_header_number(&self) -> Result<C::BlockNumber> {
		self.new.best_finalized_header_number().await
	}

	/// Return header of the best finalized block.
	pub async fn best_finalized_header(&self) -> Result<C::Header> {
		self.new.best_finalized_header().await
	}

	/// Returns the best Substrate header.
	pub async fn best_header(&self) -> Result<C::Header> {
		self.new.best_header().await
	}

	/// Get a Substrate block from its hash.
	pub async fn get_block(&self, block_hash: C::Hash) -> Result<C::SignedBlock> {
		self.new.block_by_hash(block_hash).await
	}

	/// Get a Substrate header by its hash.
	pub async fn header_by_hash(&self, block_hash: C::Hash) -> Result<C::Header> {
		self.new.header_by_hash(block_hash).await
	}

	/// Get a Substrate block hash by its number.
	pub async fn block_hash_by_number(&self, number: C::BlockNumber) -> Result<C::Hash> {
		self.new.header_hash_by_number(number).await
	}

	/// Get a Substrate header by its number.
	pub async fn header_by_number(&self, block_number: C::BlockNumber) -> Result<C::Header> {
		self.new.header_by_number(block_number).await
	}

	/// Return runtime version.
	pub async fn runtime_version(&self) -> Result<RuntimeVersion> {
		self.new.runtime_version().await
	}

	/// Read value from runtime storage.
	pub async fn storage_value<T: Send + Decode + 'static>(
		&self,
		storage_key: StorageKey,
		block_hash: Option<C::Hash>,
	) -> Result<Option<T>> {
		self.new.storage_value(self.given_or_best(block_hash).await?, storage_key).await
	}

	/// Read `MapStorage` value from runtime storage.
	pub async fn storage_map_value<T: StorageMapKeyProvider>(
		&self,
		pallet_prefix: &str,
		key: &T::Key,
		block_hash: Option<C::Hash>,
	) -> Result<Option<T::Value>> {
		self.new
			.storage_map_value::<T>(self.given_or_best(block_hash).await?, pallet_prefix, key)
			.await
	}

	/// Read `DoubleMapStorage` value from runtime storage.
	pub async fn storage_double_map_value<T: StorageDoubleMapKeyProvider>(
		&self,
		pallet_prefix: &str,
		key1: &T::Key1,
		key2: &T::Key2,
		block_hash: Option<C::Hash>,
	) -> Result<Option<T::Value>> {
		self.new
			.storage_double_map_value::<T>(
				self.given_or_best(block_hash).await?,
				pallet_prefix,
				key1,
				key2,
			)
			.await
	}

	/// Read raw value from runtime storage.
	pub async fn raw_storage_value(
		&self,
		storage_key: StorageKey,
		block_hash: Option<C::Hash>,
	) -> Result<Option<StorageData>> {
		self.new
			.raw_storage_value(self.given_or_best(block_hash).await?, storage_key)
			.await
	}

	/// Submit unsigned extrinsic for inclusion in a block.
	///
	/// Note: The given transaction needs to be SCALE encoded beforehand.
	pub async fn submit_unsigned_extrinsic(&self, transaction: Bytes) -> Result<C::Hash> {
		self.new.submit_unsigned_extrinsic(transaction).await
	}

	/// Submit an extrinsic signed by given account.
	///
	/// All calls of this method are synchronized, so there can't be more than one active
	/// `submit_signed_extrinsic()` call. This guarantees that no nonces collision may happen
	/// if all client instances are clones of the same initial `Client`.
	///
	/// Note: The given transaction needs to be SCALE encoded beforehand.
	pub async fn submit_signed_extrinsic(
		&self,
		signer: &AccountKeyPairOf<C>,
		prepare_extrinsic: impl FnOnce(HeaderIdOf<C>, C::Index) -> Result<UnsignedTransaction<C>>
			+ Send
			+ 'static,
	) -> Result<C::Hash>
	where
		C: ChainWithTransactions,
		C::AccountId: From<<C::AccountKeyPair as Pair>::Public>,
	{
		self.new.submit_signed_extrinsic(signer, prepare_extrinsic).await
	}

	/// Does exactly the same as `submit_signed_extrinsic`, but keeps watching for extrinsic status
	/// after submission.
	pub async fn submit_and_watch_signed_extrinsic(
		&self,
		signer: &AccountKeyPairOf<C>,
		prepare_extrinsic: impl FnOnce(HeaderIdOf<C>, C::Index) -> Result<UnsignedTransaction<C>>
			+ Send
			+ 'static,
	) -> Result<TransactionTracker<C, NewRpcWithCachingClient<C>>>
	where
		C: ChainWithTransactions,
		C::AccountId: From<<C::AccountKeyPair as Pair>::Public>,
	{
		self.new.submit_and_watch_signed_extrinsic(signer, prepare_extrinsic).await
	}

	/// Returns pending extrinsics from transaction pool.
	pub async fn pending_extrinsics(&self) -> Result<Vec<Bytes>> {
		self.new.pending_extrinsics().await
	}

	/// Validate transaction at given block state.
	pub async fn validate_transaction<SignedTransaction: Encode + Send + 'static>(
		&self,
		at_block: C::Hash,
		transaction: SignedTransaction,
	) -> Result<TransactionValidity> {
		self.new.validate_transaction(at_block, transaction).await
	}

	/// Returns weight of the given transaction.
	pub async fn estimate_extrinsic_weight<SignedTransaction: Encode + Send + 'static>(
		&self,
		transaction: SignedTransaction,
	) -> Result<Weight> {
		self.new
			.estimate_extrinsic_weight(self.given_or_best(None).await?, transaction)
			.await
	}

	/// Get the GRANDPA authority set at given block.
	pub async fn grandpa_authorities_set(
		&self,
		block: C::Hash,
	) -> Result<OpaqueGrandpaAuthoritiesSet> {
		self.jsonrpsee_execute(move |client| async move {
			let call = SUB_API_GRANDPA_AUTHORITIES.to_string();
			let data = Bytes(Vec::new());

			let encoded_response =
				SubstrateStateClient::<C>::call(&*client, call, data, Some(block)).await?;
			let authority_list = encoded_response.0;

			Ok(authority_list)
		})
		.await
	}

	/// Execute runtime call at given block, provided the input and output types.
	/// It also performs the input encode and output decode.
	pub async fn typed_state_call<Input: codec::Encode + Send + 'static, Output: codec::Decode>(
		&self,
		method_name: String,
		input: Input,
		at_block: Option<C::Hash>,
	) -> Result<Output> {
		self.new
			.state_call(self.given_or_best(at_block).await?, method_name, input)
			.await
	}

	/// Execute runtime call at given block.
	pub async fn state_call(
		&self,
		method: String,
		data: Bytes,
		at_block: Option<C::Hash>,
	) -> Result<Bytes> {
		self.new.raw_state_call(self.given_or_best(at_block).await?, method, data).await
	}

	/// Returns storage proof of given storage keys.
	pub async fn prove_storage(
		&self,
		keys: Vec<StorageKey>,
		at_block: C::Hash,
	) -> Result<StorageProof> {
		self.new.prove_storage(at_block, keys).await
	}

	/// Return `tokenDecimals` property from the set of chain properties.
	pub async fn token_decimals(&self) -> Result<Option<u64>> {
		self.jsonrpsee_execute(move |client| async move {
			let system_properties = SubstrateSystemClient::<C>::properties(&*client).await?;
			Ok(system_properties.get("tokenDecimals").and_then(|v| v.as_u64()))
		})
		.await
	}

	/// Execute jsonrpsee future in tokio context.
	async fn jsonrpsee_execute<MF, F, T>(&self, make_jsonrpsee_future: MF) -> Result<T>
	where
		MF: FnOnce(Arc<RpcClient>) -> F + Send + 'static,
		F: Future<Output = Result<T>> + Send + 'static,
		T: Send + 'static,
	{
		let data = self.data.read().await;
		let client = data.client.clone();
		data.tokio.spawn(make_jsonrpsee_future(client)).await?
	}

	/// Returns `true` if version guard can be started.
	///
	/// There's no reason to run version guard when version mode is set to `Auto`. It can
	/// lead to relay shutdown when chain is upgraded, even though we have explicitly
	/// said that we don't want to shutdown.
	pub fn can_start_version_guard(&self) -> bool {
		!matches!(self.chain_runtime_version, ChainRuntimeVersion::Auto)
	}

	async fn given_or_best(&self, at: Option<HashOf<C>>) -> Result<HashOf<C>> {
		Ok(match at {
			Some(at) => at,
			None => self.best_header().await?.hash(),
		})
	}
}

impl<T: DeserializeOwned> Subscription<T> {
	/// Consumes subscription and returns future statuses stream.
	pub fn into_stream(self) -> impl futures::Stream<Item = T> {
		futures::stream::unfold(self, |this| async {
			let item = this.0.lock().await.next().await.unwrap_or(None);
			item.map(|i| (i, this))
		})
	}

	/// Return next item from the subscription.
	pub async fn next(&self) -> Result<Option<T>> {
		let mut receiver = self.0.lock().await;
		let item = receiver.next().await;
		Ok(item.unwrap_or(None))
	}

	/// Background worker that is executed in tokio context as `jsonrpsee` requires.
	pub async fn background_worker(
		// TODO: remove pub
		chain_name: String,
		item_type: String,
		mut subscription: jsonrpsee::core::client::Subscription<T>,
		mut sender: futures::channel::mpsc::Sender<Option<T>>,
	) {
		loop {
			match subscription.next().await {
				Some(Ok(item)) =>
					if sender.send(Some(item)).await.is_err() {
						break
					},
				Some(Err(e)) => {
					log::trace!(
						target: "bridge",
						"{} {} subscription stream has returned '{:?}'. Stream needs to be restarted.",
						chain_name,
						item_type,
						e,
					);
					let _ = sender.send(None).await;
					break
				},
				None => {
					log::trace!(
						target: "bridge",
						"{} {} subscription stream has returned None. Stream needs to be restarted.",
						chain_name,
						item_type,
					);
					let _ = sender.send(None).await;
					break
				},
			}
		}
	}
}
