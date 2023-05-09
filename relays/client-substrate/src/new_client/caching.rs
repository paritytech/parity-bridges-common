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

//! Client implementation that is caching (whenever possible) results of its backend
//! method calls.

use crate::{
	error::Result, new_client::Client, AccountIdOf, AccountKeyPairOf, BlockNumberOf, Chain,
	ChainWithTransactions, HashOf, HeaderIdOf, HeaderOf, IndexOf, SignedBlockOf,
	TransactionTracker, UnsignedTransaction,
};

use async_std::sync::Arc;
use async_trait::async_trait;
use quick_cache::sync::Cache;
use sp_core::{
	storage::{StorageData, StorageKey},
	Bytes, Pair,
};
use sp_version::RuntimeVersion;

#[derive(Clone)]
pub struct CachingClient<C: Chain, B: Client<C>> {
	backend: B,
	header_hash_by_number_cache: Arc<Cache<BlockNumberOf<C>, HashOf<C>>>,
	header_by_hash_cache: Arc<Cache<HashOf<C>, HeaderOf<C>>>,
	block_by_hash_cache: Arc<Cache<HashOf<C>, SignedBlockOf<C>>>,
	raw_storage_value_cache: Arc<Cache<(HashOf<C>, StorageKey), Option<StorageData>>>,
}

impl<C: Chain, B: Client<C>> CachingClient<C, B> {
	pub fn new(backend: B) -> Self {
		// most of relayer operations will never touch more than `ANCIENT_BLOCK_THRESHOLD`
		// headers, so we'll use this as a cache capacity for all chain-related caches
		let capacity = crate::client::ANCIENT_BLOCK_THRESHOLD as usize;
		CachingClient {
			backend,
			header_hash_by_number_cache: Arc::new(Cache::new(capacity)),
			header_by_hash_cache: Arc::new(Cache::new(capacity)),
			block_by_hash_cache: Arc::new(Cache::new(capacity)),
			raw_storage_value_cache: Arc::new(Cache::new(1_024)),
		}
	}
}

#[async_trait]
impl<C: Chain, B: Client<C>> Client<C> for CachingClient<C, B> {
	async fn reconnect(&self) -> Result<()> {
		// TODO: do we need to clear the cache here? IMO not, but think twice
		self.backend.reconnect().await?;
		Ok(())
	}

	async fn header_hash_by_number(&self, number: BlockNumberOf<C>) -> Result<HashOf<C>> {
		self.header_hash_by_number_cache
			.get_or_insert_async(&number, self.backend.header_hash_by_number(number))
			.await
	}

	async fn header_by_hash(&self, hash: HashOf<C>) -> Result<HeaderOf<C>> {
		self.header_by_hash_cache
			.get_or_insert_async(&hash, self.backend.header_by_hash(hash))
			.await
	}

	async fn block_by_hash(&self, hash: HashOf<C>) -> Result<SignedBlockOf<C>> {
		self.block_by_hash_cache
			.get_or_insert_async(&hash, self.backend.block_by_hash(hash))
			.await
	}

	async fn best_finalized_header_hash(&self) -> Result<HashOf<C>> {
		// TODO: after https://github.com/paritytech/parity-bridges-common/issues/2074 we may
		// use single-value-cache here, but for now let's just call the backend
		self.backend.best_finalized_header_hash().await
	}

	async fn best_header(&self) -> Result<HeaderOf<C>> {
		// TODO: if after https://github.com/paritytech/parity-bridges-common/issues/2074 we'll
		// be using subscriptions to get best blocks, we may use single-value-cache here, but for
		// now let's just call the backend
		self.backend.best_header().await
	}

	async fn runtime_version(&self) -> Result<RuntimeVersion> {
		self.backend.runtime_version().await
	}

	async fn raw_storage_value(
		&self,
		at: HashOf<C>,
		storage_key: StorageKey,
	) -> Result<Option<StorageData>> {
		self.raw_storage_value_cache
			.get_or_insert_async(
				&(at, storage_key.clone()),
				self.backend.raw_storage_value(at, storage_key),
			)
			.await
	}

	async fn pending_extrinsics(&self) -> Result<Vec<Bytes>> {
		self.backend.pending_extrinsics().await
	}

	async fn submit_unsigned_extrinsic(&self, transaction: Bytes) -> Result<HashOf<C>> {
		self.backend.submit_unsigned_extrinsic(transaction).await
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
		self.backend.submit_signed_extrinsic(signer, prepare_extrinsic).await
	}

	/// Does exactly the same as `submit_signed_extrinsic`, but keeps watching for extrinsic status
	/// after submission.
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
		self.backend
			.submit_and_watch_signed_extrinsic(signer, prepare_extrinsic)
			.await
			.map(|t| t.switch_environment(self.clone()))
	}
}
