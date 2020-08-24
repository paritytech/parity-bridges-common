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

//! Module that provides RPC methods speific to message-lane pallet.

use crate::error::{Error, FutureResult};

use bp_message_lane::{LaneId, MessageNonce};
use futures::{FutureExt, TryFutureExt};
use jsonrpc_core::futures::Future as _;
use jsonrpc_derive::rpc;
use sc_client_api::Backend as BackendT;
use serde::{Deserialize, Serialize};
use sp_blockchain::{Error as BlockchainError, HeaderBackend};
use sp_core::{storage::StorageKey, Bytes};
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use sp_state_machine::prove_read;
use sp_trie::StorageProof;
use std::sync::Arc;

mod error;

/// Instance id.
pub type InstanceId = [u8; 4];

/// Trie-based storage proof that the message(s) with given key(s) are sent by the bridged chain.
pub type MessagesProof = SerializableStorageProof;

/// Trie-based storage proof that the message(s) with given key(s) are received by the bridged chain.
pub type MessagesRetrievalProof = SerializableStorageProof;

/// Trie-based storage proof that the message(s) with given key(s) are processed by the bridged chain.
pub type MessagesProcessingProof = SerializableStorageProof;

/// Serializable storage proof.
#[derive(Debug, Serialize, Deserialize)]
pub struct SerializableStorageProof {
	proof: Vec<Bytes>,
}

/// Runtime adapter.
pub trait Runtime: Send + Sync + 'static {
	/// Return runtime storage key for given message. May return None if instance is unknown.
	fn message_key(&self, instance: &InstanceId, lane: &LaneId, nonce: MessageNonce) -> Option<StorageKey>;
	/// Return runtime storage key for inbound lane state. May return None if instance is unknown.
	fn inbound_lane_data_key(&self, instance: &InstanceId, lane: &LaneId) -> Option<StorageKey>;
}

/// Provides RPC methods for interacting with message-lane pallet.
#[rpc]
pub trait MessageLaneApi<BlockHash> {
	/// Returns proof-of-message(s) in given inclusive range.
	#[rpc(name = "prove_messages")]
	fn prove_messages(
		&self,
		instance: InstanceId,
		lane: LaneId,
		begin: MessageNonce,
		end: MessageNonce,
		block: Option<BlockHash>,
	) -> FutureResult<MessagesProof>;

	/// Returns proof-of-message(s) retrieval.
	#[rpc(name = "prove_messagesRetrieval")]
	fn prove_messages_retrieval(
		&self,
		instance: InstanceId,
		lane: LaneId,
		block: Option<BlockHash>,
	) -> FutureResult<MessagesRetrievalProof>;

	/// Returns proof-of-message(s) processing.
	#[rpc(name = "prove_messagesProcessing")]
	fn prove_messages_processing(
		&self,
		instance: InstanceId,
		lane: LaneId,
		block: Option<BlockHash>,
	) -> FutureResult<MessagesProcessingProof>;
}

/// Implements the MessageLaneApi trait for interacting with message lanes.
pub struct MessageLaneRpcHandler<Block, Backend, R> {
	backend: Arc<Backend>,
	runtime: Arc<R>,
	_phantom: std::marker::PhantomData<Block>,
}

impl<Block, Backend, R> MessageLaneRpcHandler<Block, Backend, R> {
	/// Creates new mesage lane RPC handler.
	pub fn new(backend: Arc<Backend>, runtime: Arc<R>) -> Self {
		Self {
			backend,
			runtime,
			_phantom: Default::default(),
		}
	}
}

impl<Block, Backend, R> MessageLaneApi<Block::Hash> for MessageLaneRpcHandler<Block, Backend, R>
where
	Block: BlockT,
	Backend: BackendT<Block> + 'static,
	R: Runtime,
{
	fn prove_messages(
		&self,
		instance: InstanceId,
		lane: LaneId,
		begin: MessageNonce,
		end: MessageNonce,
		block: Option<Block::Hash>,
	) -> FutureResult<MessagesProof> {
		let runtime = self.runtime.clone();
		Box::new(
			prove_keys_read(
				self.backend.clone(),
				block,
				(begin..=end).map(move |nonce| runtime.message_key(&instance, &lane, nonce)),
			)
			.boxed()
			.compat()
			.map(Into::into)
			.map_err(Into::into),
		)
	}

	fn prove_messages_retrieval(
		&self,
		instance: InstanceId,
		lane: LaneId,
		block: Option<Block::Hash>,
	) -> FutureResult<MessagesRetrievalProof> {
		Box::new(
			prove_keys_read(
				self.backend.clone(),
				block,
				vec![self.runtime.inbound_lane_data_key(&instance, &lane)],
			)
			.boxed()
			.compat()
			.map(Into::into)
			.map_err(Into::into),
		)
	}

	fn prove_messages_processing(
		&self,
		instance: InstanceId,
		lane: LaneId,
		block: Option<Block::Hash>,
	) -> FutureResult<MessagesProcessingProof> {
		Box::new(
			prove_keys_read(
				self.backend.clone(),
				block,
				vec![self.runtime.inbound_lane_data_key(&instance, &lane)],
			)
			.boxed()
			.compat()
			.map(Into::into)
			.map_err(Into::into),
		)
	}
}

impl From<StorageProof> for SerializableStorageProof {
	fn from(proof: StorageProof) -> Self {
		SerializableStorageProof {
			proof: proof.iter_nodes().map(Into::into).collect(),
		}
	}
}

async fn prove_keys_read<Block, Backend>(
	backend: Arc<Backend>,
	block: Option<Block::Hash>,
	keys: impl IntoIterator<Item = Option<StorageKey>>,
) -> Result<StorageProof, Error>
where
	Block: BlockT,
	Backend: BackendT<Block> + 'static,
{
	let block = unwrap_or_best(&*backend, block);
	let state = backend.state_at(BlockId::Hash(block)).map_err(blockchain_err)?;
	let keys = keys
		.into_iter()
		.map(|key| key.ok_or(Error::UnknownInstance).map(|key| key.0))
		.collect::<Result<Vec<_>, _>>()?;
	let storage_proof = prove_read(state, keys)
		.map_err(BlockchainError::Execution)
		.map_err(blockchain_err)?;
	Ok(storage_proof)
}

fn unwrap_or_best<Block: BlockT>(backend: &impl BackendT<Block>, block: Option<Block::Hash>) -> Block::Hash {
	match block {
		Some(block) => block,
		None => backend.blockchain().info().best_hash,
	}
}

fn blockchain_err(err: BlockchainError) -> Error {
	Error::Client(Box::new(err))
}
