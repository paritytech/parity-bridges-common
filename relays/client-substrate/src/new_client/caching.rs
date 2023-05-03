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

use crate::{error::Result, new_client::Client, Chain, HashOf, HeaderOf, SignedBlockOf};

use async_std::sync::Arc;
use async_trait::async_trait;
use quick_cache::sync::Cache;

#[derive(Clone)]
pub struct CachingClient<C: Chain, B: Client<C>> {
	backend: B,
	header_by_hash_cache: Arc<Cache<HashOf<C>, HeaderOf<C>>>,
	block_by_hash_cache: Arc<Cache<HashOf<C>, SignedBlockOf<C>>>,
}

impl<C: Chain, B: Client<C>> CachingClient<C, B> {
	pub fn new(backend: B) -> Self {
		CachingClient {
			backend,
			header_by_hash_cache: Arc::new(Cache::new(
				crate::client::ANCIENT_BLOCK_THRESHOLD as usize,
			)),
			block_by_hash_cache: Arc::new(Cache::new(
				crate::client::ANCIENT_BLOCK_THRESHOLD as usize,
			)),
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
}
