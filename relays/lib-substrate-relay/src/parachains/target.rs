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

//! Parachain heads target.

use crate::{
	parachains::{
		ParachainsPipelineAdapter, SubmitParachainHeadsCallBuilder, SubstrateParachainsPipeline,
	},
	TransactionParams,
};

use async_trait::async_trait;
use bp_polkadot_core::parachains::{ParaHash, ParaHeadsProof, ParaId};
use bp_runtime::HeaderIdProvider;
use parachains_relay::parachains_loop::TargetClient;
use relay_substrate_client::{
	AccountIdOf, AccountKeyPairOf, Chain, Client, Error as SubstrateError, HeaderIdOf,
	ParachainBase, TransactionEra, TransactionTracker, UnsignedTransaction,
};
use relay_utils::relay_loop::Client as RelayClient;
use sp_core::Pair;

/// Substrate client as parachain heads source.
pub struct ParachainsTarget<P: SubstrateParachainsPipeline, TargetClnt> {
	client: TargetClnt,
	transaction_params: TransactionParams<AccountKeyPairOf<P::TargetChain>>,
}

impl<P: SubstrateParachainsPipeline, TargetClnt> ParachainsTarget<P, TargetClnt> {
	/// Creates new parachains target client.
	pub fn new(
		client: TargetClnt,
		transaction_params: TransactionParams<AccountKeyPairOf<P::TargetChain>>,
	) -> Self {
		ParachainsTarget { client, transaction_params }
	}

	/// Returns reference to the underlying RPC client.
	pub fn client(&self) -> &TargetClnt {
		&self.client
	}
}

impl<P: SubstrateParachainsPipeline, TargetClnt: Clone> Clone for ParachainsTarget<P, TargetClnt> {
	fn clone(&self) -> Self {
		ParachainsTarget {
			client: self.client.clone(),
			transaction_params: self.transaction_params.clone(),
		}
	}
}

#[async_trait]
impl<P: SubstrateParachainsPipeline, TargetClnt: Client<P::TargetChain>> RelayClient
	for ParachainsTarget<P, TargetClnt>
{
	type Error = SubstrateError;

	async fn reconnect(&mut self) -> Result<(), SubstrateError> {
		self.client.reconnect().await
	}
}

#[async_trait]
impl<P, TargetClnt> TargetClient<ParachainsPipelineAdapter<P>> for ParachainsTarget<P, TargetClnt>
where
	P: SubstrateParachainsPipeline,
	TargetClnt: Client<P::TargetChain>,
	AccountIdOf<P::TargetChain>: From<<AccountKeyPairOf<P::TargetChain> as Pair>::Public>,
{
	type TransactionTracker = TransactionTracker<P::TargetChain, TargetClnt>;

	async fn best_block(&self) -> Result<HeaderIdOf<P::TargetChain>, Self::Error> {
		let best_header = self.client.best_header().await?;
		let best_id = best_header.id();

		Ok(best_id)
	}

	async fn best_finalized_source_relay_chain_block(
		&self,
		at_block: &HeaderIdOf<P::TargetChain>,
	) -> Result<HeaderIdOf<P::SourceRelayChain>, Self::Error> {
		self.client
			.state_call::<_, Option<HeaderIdOf<P::SourceRelayChain>>>(
				at_block.hash(),
				P::SourceRelayChain::BEST_FINALIZED_HEADER_ID_METHOD.into(),
				(),
			)
			.await?
			.map(Ok)
			.unwrap_or(Err(SubstrateError::BridgePalletIsNotInitialized))
	}

	async fn parachain_head(
		&self,
		at_block: HeaderIdOf<P::TargetChain>,
	) -> Result<Option<HeaderIdOf<P::SourceParachain>>, Self::Error> {
		self.client
			.state_call(
				at_block.hash(),
				P::SourceParachain::BEST_FINALIZED_HEADER_ID_METHOD.into(),
				(),
			)
			.await
	}

	async fn submit_parachain_head_proof(
		&self,
		at_relay_block: HeaderIdOf<P::SourceRelayChain>,
		updated_head_hash: ParaHash,
		proof: ParaHeadsProof,
	) -> Result<Self::TransactionTracker, Self::Error> {
		let transaction_params = self.transaction_params.clone();
		let call = P::SubmitParachainHeadsCallBuilder::build_submit_parachain_heads_call(
			at_relay_block,
			vec![(ParaId(P::SourceParachain::PARACHAIN_ID), updated_head_hash)],
			proof,
		);
		self.client
			.submit_and_watch_signed_extrinsic(
				&transaction_params.signer,
				move |best_block_id, transaction_nonce| {
					Ok(UnsignedTransaction::new(call.into(), transaction_nonce)
						.era(TransactionEra::new(best_block_id, transaction_params.mortality)))
				},
			)
			.await
	}
}
