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
	error::Result,
	BlockNumberOf, Chain, HashOf, HeaderOf, SignedBlockOf,
};

use async_trait::async_trait;
use sp_runtime::traits::Header as _;

#[async_trait]
pub trait Client<C: Chain>: 'static + Send + Sync + Clone {
	/// Reconnects the client.
	async fn reconnect(&self) -> Result<()>;

	/// Get header by hash.
	async fn header_by_hash(&self, hash: HashOf<C>) -> Result<HeaderOf<C>>;
	/// Get block by hash.
	async fn block_by_hash(&self, hash: HashOf<C>) -> Result<SignedBlockOf<C>>;

	/// Get best finalized header hash.
	async fn best_finalized_header_hash(&self) -> Result<HashOf<C>>;
	/// Get best finalized header number.
	async fn best_finalized_header_number(&self) -> Result<BlockNumberOf<C>> {
		Ok(*self.best_finalized_header().await?.number())
	}
	/// Get best finalized header.
	async fn best_finalized_header(&self) -> Result<HeaderOf<C>> {
		self.header_by_hash(self.best_finalized_header_hash().await?).await
	}

	/// Get best header.
	async fn best_header(&self) -> Result<HeaderOf<C>>;
}
