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

use crate::{
	client::{ChainRuntimeVersion, SimpleRuntimeVersion},
	error::{Error, Result},
	new_client::Client,
	rpc::{
		SubstrateAuthorClient, SubstrateBeefyClient, SubstrateChainClient,
		SubstrateFrameSystemClient, SubstrateGrandpaClient, SubstrateStateClient,
		SubstrateSystemClient,
	},
	transaction_stall_timeout, AccountIdOf, AccountKeyPairOf, BlockNumberOf, Chain,
	ChainWithGrandpa, ChainWithTransactions, ConnectionParams, HashOf, HeaderIdOf, HeaderOf,
	IndexOf, SignParam, SignedBlockOf, Subscription, TransactionTracker, UnsignedTransaction,
};

use async_std::sync::{Arc, Mutex, RwLock};
use async_trait::async_trait;
use bp_runtime::HeaderIdProvider;
use codec::{Decode, Encode};
use frame_support::weights::Weight;
use jsonrpsee::ws_client::{WsClient, WsClientBuilder};
use num_traits::Zero;
use pallet_transaction_payment::RuntimeDispatchInfo;
use relay_utils::{relay_loop::RECONNECT_DELAY, STALL_TIMEOUT};
use sp_core::{
	storage::{StorageData, StorageKey},
	Bytes, Hasher, Pair,
};
use sp_runtime::transaction_validity::{TransactionSource, TransactionValidity};
use sp_trie::StorageProof;
use sp_version::RuntimeVersion;
use std::{future::Future, marker::PhantomData};

const MAX_SUBSCRIPTION_CAPACITY: usize = 4096;

const SUB_API_TXPOOL_VALIDATE_TRANSACTION: &str = "TaggedTransactionQueue_validate_transaction";
const SUB_API_TX_PAYMENT_QUERY_INFO: &str = "TransactionPaymentApi_query_info";

pub struct RpcClient<C: Chain> {
	// Lock order: `submit_signed_extrinsic_lock`, `data`
	/// Client connection params.
	params: Arc<ConnectionParams>,
	/// If several tasks are submitting their transactions simultaneously using
	/// `submit_signed_extrinsic` method, they may get the same transaction nonce. So one of
	/// transactions will be rejected from the pool. This lock is here to prevent situations like
	/// that.
	submit_signed_extrinsic_lock: Arc<Mutex<()>>,
	/// Genesis block hash.
	genesis_hash: HashOf<C>,
	/// Shared dynamic data.
	data: Arc<RwLock<ClientData>>,
	/// Generic arguments dump.
	_phantom: PhantomData<C>,
}

/// Client data, shared by all `Client` clones.
struct ClientData {
	/// Tokio runtime handle.
	tokio: Arc<tokio::runtime::Runtime>,
	/// Substrate RPC client.
	client: Arc<WsClient>,
}

impl<C: Chain> RpcClient<C> {
	/// Returns client that is able to call RPCs on Substrate node over websocket connection.
	///
	/// This function will keep connecting to given Substrate node until connection is established
	/// and is functional. If attempt fail, it will wait for `RECONNECT_DELAY` and retry again.
	pub async fn new(params: ConnectionParams) -> Self {
		let params = Arc::new(params);
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
	async fn try_connect(params: Arc<ConnectionParams>) -> Result<Self> {
		let (tokio, client) = Self::build_client(&params).await?;

		let genesis_hash_client = client.clone();
		let genesis_hash = tokio
			.spawn(async move {
				SubstrateChainClient::<C>::block_hash(&*genesis_hash_client, Some(Zero::zero()))
					.await
			})
			.await??;

		Ok(Self {
			params,
			submit_signed_extrinsic_lock: Arc::new(Mutex::new(())),
			genesis_hash,
			data: Arc::new(RwLock::new(ClientData { tokio, client })),
			_phantom: PhantomData::default(),
		})
	}

	/// Build client to use in connection.
	async fn build_client(
		params: &ConnectionParams,
	) -> Result<(Arc<tokio::runtime::Runtime>, Arc<WsClient>)> {
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
				WsClientBuilder::default()
					.max_buffer_capacity_per_subscription(MAX_SUBSCRIPTION_CAPACITY)
					.build(&uri)
					.await
			})
			.await??;

		Ok((Arc::new(tokio), Arc::new(client)))
	}

	/// Execute jsonrpsee future in tokio context.
	pub async fn jsonrpsee_execute<MF, F, T>(&self, make_jsonrpsee_future: MF) -> Result<T>
	// TODO: make me private
	where
		MF: FnOnce(Arc<WsClient>) -> F + Send + 'static,
		F: Future<Output = Result<T>> + Send + 'static,
		T: Send + 'static,
	{
		let data = self.data.read().await;
		let client = data.client.clone();
		data.tokio.spawn(make_jsonrpsee_future(client)).await?
	}

	/// Prepare parameters used to sign chain transactions.
	async fn build_sign_params(&self, signer: AccountKeyPairOf<C>) -> Result<SignParam<C>>
	where
		C: ChainWithTransactions,
	{
		let runtime_version = self.simple_runtime_version().await?;
		Ok(SignParam::<C> {
			spec_version: runtime_version.spec_version,
			transaction_version: runtime_version.transaction_version,
			genesis_hash: self.genesis_hash,
			signer,
		})
	}

	/// Return runtime specification and transaction versions, to use when signing transactions.
	pub async fn simple_runtime_version(&self) -> Result<SimpleRuntimeVersion> {
		Ok(match self.params.chain_runtime_version {
			ChainRuntimeVersion::Auto => {
				let runtime_version = self.runtime_version().await?;
				SimpleRuntimeVersion::from_runtime_version(&runtime_version)
			},
			ChainRuntimeVersion::Custom(ref version) => *version,
		})
	}

	/// Get the nonce of the given Substrate account.
	pub async fn next_account_index(&self, account: AccountIdOf<C>) -> Result<IndexOf<C>> {
		self.jsonrpsee_execute(move |client| async move {
			Ok(SubstrateFrameSystemClient::<C>::account_next_index(&*client, account).await?)
		})
		.await
	}
}

impl<C: Chain> Clone for RpcClient<C> {
	fn clone(&self) -> Self {
		RpcClient {
			params: self.params.clone(),
			submit_signed_extrinsic_lock: self.submit_signed_extrinsic_lock.clone(),
			genesis_hash: self.genesis_hash,
			data: self.data.clone(),
			_phantom: PhantomData::default(),
		}
	}
}

impl<C: Chain> std::fmt::Debug for RpcClient<C> {
	fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
		fmt.write_fmt(format_args!("Client<{}>", C::NAME))
	}
}

#[async_trait]
impl<C: Chain> Client<C> for RpcClient<C> {
	async fn ensure_synced(&self) -> Result<()> {
		let health = self
			.jsonrpsee_execute(|client| async move {
				Ok(SubstrateSystemClient::<C>::health(&*client).await?)
			})
			.await
			.map_err(|e| Error::failed_to_get_system_health::<C>(e.into()))?;

		let is_synced = !health.is_syncing && (!health.should_have_peers || health.peers > 0);
		if is_synced {
			Ok(())
		} else {
			Err(Error::ClientNotSynced(health))
		}
	}

	async fn reconnect(&self) -> Result<()> {
		let mut data = self.data.write().await;
		let (tokio, client) = Self::build_client(&self.params).await?;
		data.tokio = tokio;
		data.client = client;
		Ok(())
	}

	async fn header_hash_by_number(&self, number: BlockNumberOf<C>) -> Result<HashOf<C>> {
		self.jsonrpsee_execute(move |client| async move {
			Ok(SubstrateChainClient::<C>::block_hash(&*client, Some(number)).await?)
		})
		.await
		.map_err(|e| Error::failed_to_read_header_hash_by_number::<C>(number, e))
	}

	async fn header_by_hash(&self, hash: HashOf<C>) -> Result<HeaderOf<C>> {
		self.jsonrpsee_execute(move |client| async move {
			Ok(SubstrateChainClient::<C>::header(&*client, Some(hash)).await?)
		})
		.await
		.map_err(|e| Error::failed_to_read_header_by_hash::<C>(hash, e))
	}

	async fn block_by_hash(&self, hash: HashOf<C>) -> Result<SignedBlockOf<C>> {
		self.jsonrpsee_execute(move |client| async move {
			Ok(SubstrateChainClient::<C>::block(&*client, Some(hash)).await?)
		})
		.await
		.map_err(|e| Error::failed_to_read_block_by_hash::<C>(hash, e))
	}

	async fn best_finalized_header_hash(&self) -> Result<HashOf<C>> {
		self.jsonrpsee_execute(|client| async move {
			Ok(SubstrateChainClient::<C>::finalized_head(&*client).await?)
		})
		.await
		.map_err(|e| Error::failed_to_read_best_finalized_header_hash::<C>(e))
	}

	async fn best_header(&self) -> Result<HeaderOf<C>> {
		self.jsonrpsee_execute(|client| async move {
			Ok(SubstrateChainClient::<C>::header(&*client, None).await?)
		})
		.await
		.map_err(|e| Error::failed_to_read_best_header::<C>(e))
	}

	async fn subscribe_grandpa_justifications(&self) -> Result<Subscription<Bytes>>
	where
		C: ChainWithGrandpa,
	{
		let subscription = self
			.jsonrpsee_execute(move |client| async move {
				Ok(SubstrateGrandpaClient::<C>::subscribe_justifications(&*client).await?)
			})
			.await
			.map_err(|e| Error::failed_to_subscribe_grandpa_justification::<C>(e))?;
		let (sender, receiver) = futures::channel::mpsc::channel(MAX_SUBSCRIPTION_CAPACITY);
		self.data.read().await.tokio.spawn(Subscription::background_worker(
			C::NAME.into(),
			"grandpa_justification".into(),
			subscription,
			sender,
		));
		Ok(Subscription(Mutex::new(receiver)))
	}

	async fn subscribe_beefy_justifications(&self) -> Result<Subscription<Bytes>> {
		let subscription = self
			.jsonrpsee_execute(move |client| async move {
				Ok(SubstrateBeefyClient::<C>::subscribe_justifications(&*client).await?)
			})
			.await
			.map_err(|e| Error::failed_to_subscribe_beefy_justification::<C>(e))?;
		let (sender, receiver) = futures::channel::mpsc::channel(MAX_SUBSCRIPTION_CAPACITY);
		self.data.read().await.tokio.spawn(Subscription::background_worker(
			C::NAME.into(),
			"beefy_justification".into(),
			subscription,
			sender,
		));
		Ok(Subscription(Mutex::new(receiver)))
	}

	async fn runtime_version(&self) -> Result<RuntimeVersion> {
		self.jsonrpsee_execute(move |client| async move {
			Ok(SubstrateStateClient::<C>::runtime_version(&*client).await?)
		})
		.await
		.map_err(|e| Error::failed_to_read_runtime_version::<C>(e))
	}

	async fn raw_storage_value(
		&self,
		at: HashOf<C>,
		storage_key: StorageKey,
	) -> Result<Option<StorageData>> {
		let cloned_storage_key = storage_key.clone();
		self.jsonrpsee_execute(move |client| async move {
			Ok(SubstrateStateClient::<C>::storage(&*client, storage_key.clone(), Some(at)).await?)
		})
		.await
		.map_err(|e| Error::failed_to_read_storage_value::<C>(at, cloned_storage_key, e))
	}

	async fn pending_extrinsics(&self) -> Result<Vec<Bytes>> {
		self.jsonrpsee_execute(move |client| async move {
			Ok(SubstrateAuthorClient::<C>::pending_extrinsics(&*client).await?)
		})
		.await
		.map_err(|e| Error::failed_to_get_pending_extrinsics::<C>(e))
	}

	async fn submit_unsigned_extrinsic(&self, transaction: Bytes) -> Result<HashOf<C>> {
		self.jsonrpsee_execute(move |client| async move {
			let tx_hash = SubstrateAuthorClient::<C>::submit_extrinsic(&*client, transaction)
				.await
				.map_err(|e| {
					log::error!(target: "bridge", "Failed to send transaction to {} node: {:?}", C::NAME, e);
					e
				})?;
			log::trace!(target: "bridge", "Sent transaction to {} node: {:?}", C::NAME, tx_hash);
			Ok(tx_hash)
		})
		.await
		.map_err(|e| Error::failed_to_submit_transaction::<C>(e.into()))
	}

	async fn submit_signed_extrinsic(
		&self,
		signer: &AccountKeyPairOf<C>,
		prepare_extrinsic: impl FnOnce(HeaderIdOf<C>, IndexOf<C>) -> Result<UnsignedTransaction<C>>
			+ Send
			+ 'static,
	) -> Result<HashOf<C>>
	where
		C: ChainWithTransactions,
		AccountIdOf<C>: From<<AccountKeyPairOf<C> as Pair>::Public>,
	{
		let _guard = self.submit_signed_extrinsic_lock.lock().await;
		let transaction_nonce = self.next_account_index(signer.public().into()).await?;
		let best_header = self.best_header().await?;
		let signing_data = self.build_sign_params(signer.clone()).await?;

		// By using parent of best block here, we are protecting again best-block reorganizations.
		// E.g. transaction may have been submitted when the best block was `A[num=100]`. Then it
		// has been changed to `B[num=100]`. Hash of `A` has been included into transaction
		// signature payload. So when signature will be checked, the check will fail and transaction
		// will be dropped from the pool.
		let best_header_id = best_header.parent_id().unwrap_or_else(|| best_header.id());

		self.jsonrpsee_execute(move |client| async move {
			let extrinsic = prepare_extrinsic(best_header_id, transaction_nonce)?;
			let signed_extrinsic = C::sign_transaction(signing_data, extrinsic)?.encode();
			let tx_hash =
				SubstrateAuthorClient::<C>::submit_extrinsic(&*client, Bytes(signed_extrinsic))
					.await
					.map_err(|e| {
						log::error!(target: "bridge", "Failed to send transaction to {} node: {:?}", C::NAME, e);
						e
					})?;
			log::trace!(target: "bridge", "Sent transaction to {} node: {:?}", C::NAME, tx_hash);
			Ok(tx_hash)
		})
		.await
		.map_err(|e| Error::failed_to_submit_transaction::<C>(e.into()))
	}

	async fn submit_and_watch_signed_extrinsic(
		&self,
		signer: &AccountKeyPairOf<C>,
		prepare_extrinsic: impl FnOnce(HeaderIdOf<C>, IndexOf<C>) -> Result<UnsignedTransaction<C>>
			+ Send
			+ 'static,
	) -> Result<TransactionTracker<C, Self>>
	where
		C: ChainWithTransactions,
		AccountIdOf<C>: From<<AccountKeyPairOf<C> as Pair>::Public>,
	{
		let self_clone = self.clone();
		let signing_data = self.build_sign_params(signer.clone()).await?;
		let _guard = self.submit_signed_extrinsic_lock.lock().await;
		let transaction_nonce = self.next_account_index(signer.public().into()).await?;
		let best_header = self.best_header().await?;
		let best_header_id = best_header.id();
		let (sender, receiver) = futures::channel::mpsc::channel(MAX_SUBSCRIPTION_CAPACITY);
		let (tracker, subscription) = self
			.jsonrpsee_execute(move |client| async move {
				let extrinsic = prepare_extrinsic(best_header_id, transaction_nonce)?;
				let stall_timeout = transaction_stall_timeout(
					extrinsic.era.mortality_period(),
					C::AVERAGE_BLOCK_INTERVAL,
					STALL_TIMEOUT,
				);
				let signed_extrinsic = C::sign_transaction(signing_data, extrinsic)?.encode();
				let tx_hash = C::Hasher::hash(&signed_extrinsic);
				let subscription = SubstrateAuthorClient::<C>::submit_and_watch_extrinsic(
					&*client,
					Bytes(signed_extrinsic),
				)
				.await
				.map_err(|e| {
					log::error!(target: "bridge", "Failed to send transaction to {} node: {:?}", C::NAME, e);
					e
				})?;
				log::trace!(target: "bridge", "Sent transaction to {} node: {:?}", C::NAME, tx_hash);
				let tracker = TransactionTracker::new(
					self_clone,
					stall_timeout,
					tx_hash,
					Subscription(Mutex::new(receiver)),
				);
				Ok((tracker, subscription))
			})
			.await
			.map_err(|e| Error::failed_to_submit_transaction::<C>(e.into()))?;
		self.data.read().await.tokio.spawn(Subscription::background_worker(
			C::NAME.into(),
			"extrinsic".into(),
			subscription,
			sender,
		));
		Ok(tracker)
	}

	async fn validate_transaction<SignedTransaction: Encode + Send + 'static>(
		&self,
		at: HashOf<C>,
		transaction: SignedTransaction,
	) -> Result<TransactionValidity> {
		// TODO: replace with state_call
		self.jsonrpsee_execute(move |client| async move {
			let call = SUB_API_TXPOOL_VALIDATE_TRANSACTION.to_string();
			let data = Bytes((TransactionSource::External, transaction, at).encode());

			let encoded_response =
				SubstrateStateClient::<C>::call(&*client, call, data, Some(at)).await?;
			let validity = TransactionValidity::decode(&mut &encoded_response.0[..])
				.map_err(Error::ResponseParseFailed)?;

			Ok(validity)
		})
		.await
	}

	async fn estimate_extrinsic_weight<SignedTransaction: Encode + Send + 'static>(
		&self,
		transaction: SignedTransaction,
	) -> Result<Weight> {
		// TODO: replace with state_call
		self.jsonrpsee_execute(move |client| async move {
			let transaction_len = transaction.encoded_size() as u32;

			let call = SUB_API_TX_PAYMENT_QUERY_INFO.to_string();
			let data = Bytes((transaction, transaction_len).encode());

			let encoded_response =
				SubstrateStateClient::<C>::call(&*client, call, data, None).await?;
			let dispatch_info =
				RuntimeDispatchInfo::<C::Balance>::decode(&mut &encoded_response.0[..])
					.map_err(Error::ResponseParseFailed)?;

			Ok(dispatch_info.weight)
		})
		.await
	}

	async fn raw_state_call<Args: Encode + Send + 'static>(
		&self,
		at: HashOf<C>,
		method: String,
		arguments: Args,
	) -> Result<Bytes> {
		let arguments = Bytes(arguments.encode());
		let arguments_clone = arguments.clone();
		let method_clone = method.clone();
		self.jsonrpsee_execute(move |client| async move {
			SubstrateStateClient::<C>::call(&*client, method, arguments, Some(at))
				.await
				.map_err(Into::into)
		})
		.await
		.map_err(|e| Error::failed_state_call::<C>(at, method_clone, arguments_clone, e.into()))
	}

	async fn prove_storage(&self, at: HashOf<C>, keys: Vec<StorageKey>) -> Result<StorageProof> {
		let keys_clone = keys.clone();
		self.jsonrpsee_execute(move |client| async move {
			SubstrateStateClient::<C>::prove_storage(&*client, keys, Some(at))
				.await
				.map(|proof| {
					StorageProof::new(proof.proof.into_iter().map(|b| b.0).collect::<Vec<_>>())
				})
				.map_err(Into::into)
		})
		.await
		.map_err(|e| Error::failed_to_prove_storage::<C>(at, keys_clone, e.into()))
	}
}
