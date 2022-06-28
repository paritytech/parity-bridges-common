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
use bp_parachains::{
	best_parachain_head_hash_storage_key_at_target, imported_parachain_head_storage_key_at_target,
	BestParaHeadHash,
};
use bp_polkadot_core::parachains::{ParaHeadsProof, ParaId};
use codec::{Decode, Encode};
use parachains_relay::{
	parachains_loop::TargetClient, parachains_loop_metrics::ParachainsLoopMetrics,
};
use relay_substrate_client::{
	AccountIdOf, AccountKeyPairOf, BlockNumberOf, Chain, Client, Error as SubstrateError, HashOf,
	HeaderIdOf, HeaderOf, RelayChain, SignParam, TransactionEra, TransactionSignScheme,
	UnsignedTransaction,
};
use relay_utils::{relay_loop::Client as RelayClient, HeaderId};
use sp_core::{Bytes, Pair};
use sp_runtime::traits::Header as HeaderT;

/// Substrate client as parachain heads source.
pub struct ParachainsTarget<P: SubstrateParachainsPipeline> {
	client: Client<P::TargetChain>,
	transaction_params: TransactionParams<AccountKeyPairOf<P::TransactionSignScheme>>,
}

impl<P: SubstrateParachainsPipeline> ParachainsTarget<P> {
	/// Creates new parachains target client.
	pub fn new(
		client: Client<P::TargetChain>,
		transaction_params: TransactionParams<AccountKeyPairOf<P::TransactionSignScheme>>,
	) -> Self {
		ParachainsTarget { client, transaction_params }
	}

	/// Returns reference to the underlying RPC client.
	pub fn client(&self) -> &Client<P::TargetChain> {
		&self.client
	}
}

impl<P: SubstrateParachainsPipeline> Clone for ParachainsTarget<P> {
	fn clone(&self) -> Self {
		ParachainsTarget {
			client: self.client.clone(),
			transaction_params: self.transaction_params.clone(),
		}
	}
}

#[async_trait]
impl<P: SubstrateParachainsPipeline> RelayClient for ParachainsTarget<P> {
	type Error = SubstrateError;

	async fn reconnect(&mut self) -> Result<(), SubstrateError> {
		self.client.reconnect().await
	}
}

#[async_trait]
impl<P> TargetClient<ParachainsPipelineAdapter<P>> for ParachainsTarget<P>
where
	P: SubstrateParachainsPipeline,
	P::TransactionSignScheme: TransactionSignScheme<Chain = P::TargetChain>,
	AccountIdOf<P::TargetChain>: From<<AccountKeyPairOf<P::TransactionSignScheme> as Pair>::Public>,
{
	async fn best_block(&self) -> Result<HeaderIdOf<P::TargetChain>, Self::Error> {
		let best_header = self.client.best_header().await?;
		let best_hash = best_header.hash();
		let best_id = HeaderId(*best_header.number(), best_hash);

		Ok(best_id)
	}

	async fn best_finalized_source_block(
		&self,
		at_block: &HeaderIdOf<P::TargetChain>,
	) -> Result<HeaderIdOf<P::SourceRelayChain>, Self::Error> {
		let encoded_best_finalized_source_block = self
			.client
			.state_call(
				P::SourceRelayChain::BEST_FINALIZED_HEADER_ID_METHOD.into(),
				Bytes(Vec::new()),
				Some(at_block.1),
			)
			.await?;
		let decoded_best_finalized_source_block =
			Option::<(BlockNumberOf<P::SourceRelayChain>, HashOf<P::SourceRelayChain>)>::decode(
				&mut &encoded_best_finalized_source_block.0[..],
			)
			.map_err(SubstrateError::ResponseParseFailed)?
			.map(Ok)
			.unwrap_or(Err(SubstrateError::BridgePalletIsNotInitialized))?;
		Ok(HeaderId(decoded_best_finalized_source_block.0, decoded_best_finalized_source_block.1))
	}

	async fn parachain_head(
		&self,
		at_block: HeaderIdOf<P::TargetChain>,
		metrics: Option<&ParachainsLoopMetrics>,
		para_id: ParaId,
	) -> Result<Option<BestParaHeadHash>, Self::Error> {
		let best_para_head_hash_key = best_parachain_head_hash_storage_key_at_target(
			P::SourceRelayChain::PARACHAINS_FINALITY_PALLET_NAME,
			para_id,
		);
		let best_para_head_hash: Option<BestParaHeadHash> =
			self.client.storage_value(best_para_head_hash_key, Some(at_block.1)).await?;

		if let (Some(metrics), &Some(ref best_para_head_hash)) = (metrics, &best_para_head_hash) {
			let imported_para_head_key = imported_parachain_head_storage_key_at_target(
				P::SourceRelayChain::PARACHAINS_FINALITY_PALLET_NAME,
				para_id,
				best_para_head_hash.head_hash,
			);
			let imported_para_head: Option<HeaderOf<P::SourceParachain>> =
				self.client.storage_value(imported_para_head_key, Some(at_block.1)).await?;
			if let Some(imported_para_head) = imported_para_head {
				metrics
					.update_best_parachain_block_at_target(para_id, *imported_para_head.number());
			}
		}

		Ok(best_para_head_hash)
	}

	async fn submit_parachain_heads_proof(
		&self,
		at_relay_block: HeaderIdOf<P::SourceRelayChain>,
		updated_parachains: Vec<ParaId>,
		proof: ParaHeadsProof,
	) -> Result<(), Self::Error> {
		let genesis_hash = *self.client.genesis_hash();
		let transaction_params = self.transaction_params.clone();
		let (spec_version, transaction_version) = self.client.simple_runtime_version().await?;
		let call = P::SubmitParachainHeadsCallBuilder::build_submit_parachain_heads_call(
			at_relay_block,
			updated_parachains,
			proof,
		);
		self.client
			.submit_signed_extrinsic(
				self.transaction_params.signer.public().into(),
				move |best_block_id, transaction_nonce| {
					Ok(Bytes(
						P::TransactionSignScheme::sign_transaction(SignParam {
							spec_version,
							transaction_version,
							genesis_hash,
							signer: transaction_params.signer,
							era: TransactionEra::new(best_block_id, transaction_params.mortality),
							unsigned: UnsignedTransaction::new(call.into(), transaction_nonce),
						})?
						.encode(),
					))
				},
			)
			.await
			.map(drop)
	}
}
