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

//! Client implementation that returns some mock data without actually connecting
//! to any node.

use crate::{
	client::Client,
	error::{Error, Result},
	AccountIdOf, AccountKeyPairOf, BlockNumberOf, Chain, ChainWithGrandpa, ChainWithTransactions,
	HashOf, HeaderIdOf, HeaderOf, IndexOf, SignedBlockOf, SimpleRuntimeVersion, Subscription,
	TransactionTracker, UnsignedTransaction,
};

use async_trait::async_trait;
use codec::Encode;
use frame_support::weights::Weight;
use parking_lot::Mutex;
use sp_core::{
	storage::{StorageData, StorageKey},
	Bytes, Pair,
};
use sp_runtime::{traits::Header as HeaderT, transaction_validity::TransactionValidity};
use sp_trie::StorageProof;
use sp_version::RuntimeVersion;
use std::{collections::HashMap, sync::Arc};

/// Test client builder.
pub struct TestClientBuilder<C: Chain> {
	data: Arc<Mutex<ClientData<C>>>,
}

impl<C: Chain> TestClientBuilder<C> {
	/// Build client.
	pub fn build(self) -> TestClient<C> {
		TestClient { data: self.data }
	}

	/// Start building block.
	pub fn block(self, number: BlockNumberOf<C>) -> TestBlockBuilder<C> {
		TestBlockBuilder {
			header: HeaderOf::<C>::new(
				number,
				Default::default(),
				Default::default(),
				Default::default(),
				Default::default(),
			),
			is_finalized: false,
			data: self.data,
		}
	}
}

/// Test block builder.
pub struct TestBlockBuilder<C: Chain> {
	header: HeaderOf<C>,
	transactions: Vec<CallOf<C>>,
	is_finalized: bool,
	data: Arc<Mutex<ClientData<C>>>,
}

impl<C: Chain> TestBlockBuilder<C> {
	/// Push new transaction to the block.
	pub fn push_transaction(mut self, call: CallOf<C>) -> Self {
		self.transactions.push(call);
		self
	}

	/// Finalize block.
	pub fn finalize(mut self) -> Self {
		self.is_finalized = true;
		self
	}

	/// Build block.
	pub fn build(self) -> TestClientBuilder<C> {
		{
			let mut data = self.data.lock();
			if data
				.best_header
				.as_ref()
				.map(|bh| bh.number() <= self.header.number())
				.unwrap_or(true)
			{
				data.best_header = Some(self.header.clone());
			}
			if self.is_finalized {
				if data
					.best_finalized_header
					.as_ref()
					.map(|bh| bh.number() <= self.header.number())
					.unwrap_or(true)
				{
					data.best_finalized_header = Some(self.header.clone());
				}
			}
			data.headers.insert(self.header.hash(), self.header);
		}

		TestClientBuilder { data: self.data }
	}
}

/// Client implementation that returns some mock data without actually connecting
/// to any node.
#[derive(Clone, Default)]
pub struct TestClient<C: Chain> {
	data: Arc<Mutex<ClientData<C>>>,
}

/// Client data, shared by all `CachingClient` clones.
struct ClientData<C: Chain> {
	best_header: Option<HeaderOf<C>>,
	best_finalized_header: Option<HeaderOf<C>>,
	headers: HashMap<HashOf<C>, HeaderOf<C>>,
}

impl<C: Chain> Default for ClientData<C> {
	fn default() -> Self {
		ClientData { best_header: None, best_finalized_header: None, headers: HashMap::new() }
	}
}

impl<C: Chain> TestClient<C> {
	/// Start building client.
	pub fn builder() -> TestClientBuilder<C> {
		TestClientBuilder { data: Arc::new(Mutex::new(ClientData::default())) }
	}

	/// Start amending the client.
	pub fn amend(&self) -> TestClientBuilder<C> {
		TestClientBuilder { data: self.data.clone() }
	}
}

impl<C: Chain> std::fmt::Debug for TestClient<C> {
	fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
		fmt.write_fmt(format_args!("TestClient<{:?}>", C::NAME))
	}
}

#[async_trait]
impl<C: Chain> Client<C> for TestClient<C> {
	async fn ensure_synced(&self) -> Result<()> {
		unimplemented!("TODO")
	}

	async fn reconnect(&self) -> Result<()> {
		unimplemented!("TODO")
	}

	fn genesis_hash(&self) -> HashOf<C> {
		unimplemented!("TODO")
	}

	async fn header_hash_by_number(&self, _number: BlockNumberOf<C>) -> Result<HashOf<C>> {
		unimplemented!("TODO")
	}

	async fn header_by_hash(&self, hash: HashOf<C>) -> Result<HeaderOf<C>> {
		self.data.lock().headers.get(&hash).cloned().map(Ok).unwrap_or_else(|| {
			Err(Error::Custom(format!("TestClient::header_by_hash({:?}): not found", hash)))
		})
	}

	async fn block_by_hash(&self, _hash: HashOf<C>) -> Result<SignedBlockOf<C>> {
		unimplemented!("TODO")
	}

	async fn best_finalized_header_hash(&self) -> Result<HashOf<C>> {
		self.data
			.lock()
			.best_finalized_header
			.as_ref()
			.map(|h| Ok(h.hash()))
			.unwrap_or_else(|| {
				Err(Error::Custom(format!("TestClient::best_finalized_header_hash: not found")))
			})
	}

	async fn best_header(&self) -> Result<HeaderOf<C>> {
		self.data
			.lock()
			.best_header
			.clone()
			.map(Ok)
			.unwrap_or_else(|| Err(Error::Custom(format!("TestClient::best_header: not found"))))
	}

	async fn subscribe_grandpa_finality_justifications(&self) -> Result<Subscription<Bytes>>
	where
		C: ChainWithGrandpa,
	{
		unimplemented!("TODO")
	}

	async fn subscribe_beefy_finality_justifications(&self) -> Result<Subscription<Bytes>> {
		unimplemented!("TODO")
	}

	async fn token_decimals(&self) -> Result<Option<u64>> {
		unimplemented!("TODO")
	}

	async fn runtime_version(&self) -> Result<RuntimeVersion> {
		unimplemented!("TODO")
	}

	async fn simple_runtime_version(&self) -> Result<SimpleRuntimeVersion> {
		unimplemented!("TODO")
	}

	fn can_start_version_guard(&self) -> bool {
		unimplemented!("TODO")
	}

	async fn raw_storage_value(
		&self,
		_at: HashOf<C>,
		_storage_key: StorageKey,
	) -> Result<Option<StorageData>> {
		unimplemented!("TODO")
	}

	async fn pending_extrinsics(&self) -> Result<Vec<Bytes>> {
		unimplemented!("TODO")
	}

	async fn submit_unsigned_extrinsic(&self, _transaction: Bytes) -> Result<HashOf<C>> {
		unimplemented!("TODO")
	}

	async fn submit_signed_extrinsic(
		&self,
		_signer: &AccountKeyPairOf<C>,
		_prepare_extrinsic: impl FnOnce(HeaderIdOf<C>, IndexOf<C>) -> Result<UnsignedTransaction<C>>
			+ Send
			+ 'static,
	) -> Result<HashOf<C>>
	where
		C: ChainWithTransactions,
		AccountIdOf<C>: From<<AccountKeyPairOf<C> as Pair>::Public>,
	{
		unimplemented!("TODO")
	}

	async fn submit_and_watch_signed_extrinsic(
		&self,
		_signer: &AccountKeyPairOf<C>,
		_prepare_extrinsic: impl FnOnce(HeaderIdOf<C>, IndexOf<C>) -> Result<UnsignedTransaction<C>>
			+ Send
			+ 'static,
	) -> Result<TransactionTracker<C, Self>>
	where
		C: ChainWithTransactions,
		AccountIdOf<C>: From<<AccountKeyPairOf<C> as Pair>::Public>,
	{
		unimplemented!("TODO")
	}

	async fn validate_transaction<SignedTransaction: Encode + Send + 'static>(
		&self,
		_at: HashOf<C>,
		_transaction: SignedTransaction,
	) -> Result<TransactionValidity> {
		unimplemented!("TODO")
	}

	async fn estimate_extrinsic_weight<SignedTransaction: Encode + Send + 'static>(
		&self,
		_at: HashOf<C>,
		_transaction: SignedTransaction,
	) -> Result<Weight> {
		unimplemented!("TODO")
	}

	async fn raw_state_call<Args: Encode + Send>(
		&self,
		_at: HashOf<C>,
		_method: String,
		_arguments: Args,
	) -> Result<Bytes> {
		unimplemented!("TODO")
	}

	async fn prove_storage(&self, _at: HashOf<C>, _keys: Vec<StorageKey>) -> Result<StorageProof> {
		unimplemented!("TODO")
	}
}
