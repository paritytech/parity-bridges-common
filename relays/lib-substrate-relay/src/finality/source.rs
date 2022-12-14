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

//! Default generic implementation of finality source for basic Substrate client.

use crate::finality::{engine::Engine, FinalitySyncPipelineAdapter, SubstrateFinalitySyncPipeline};

use async_std::sync::{Arc, Mutex};
use async_trait::async_trait;
use bp_header_chain::FinalityProof;
use codec::Decode;
use finality_relay::SourceClient;
use futures::stream::{unfold, Stream, StreamExt};
use num_traits::One;
use relay_substrate_client::{
	BlockNumberOf, BlockWithJustification, Chain, Client, Error, HeaderOf,
};
use relay_utils::{relay_loop::Client as RelayClient, UniqueSaturatedInto};
use std::pin::Pin;

/// Shared updatable reference to the maximal header number that we want to sync from the source.
pub type RequiredHeaderNumberRef<C> = Arc<Mutex<<C as bp_runtime::Chain>::BlockNumber>>;

/// Substrate finality proofs stream.
pub type SubstrateFinalityProofsStream<P> =
	Pin<Box<dyn Stream<Item = SubstrateFinalityProof<P>> + Send>>;

/// Substrate finality proof. Specific to the used `FinalityEngine`.
pub type SubstrateFinalityProof<P> =
	<<P as SubstrateFinalitySyncPipeline>::FinalityEngine as Engine<
		<P as SubstrateFinalitySyncPipeline>::SourceChain,
	>>::FinalityProof;

/// Substrate node as finality source.
pub struct SubstrateFinalitySource<P: SubstrateFinalitySyncPipeline> {
	client: Client<P::SourceChain>,
	maximal_header_number: Option<RequiredHeaderNumberRef<P::SourceChain>>,
}

impl<P: SubstrateFinalitySyncPipeline> SubstrateFinalitySource<P> {
	/// Create new headers source using given client.
	pub fn new(
		client: Client<P::SourceChain>,
		maximal_header_number: Option<RequiredHeaderNumberRef<P::SourceChain>>,
	) -> Self {
		SubstrateFinalitySource { client, maximal_header_number }
	}

	/// Returns reference to the underlying RPC client.
	pub fn client(&self) -> &Client<P::SourceChain> {
		&self.client
	}

	/// Returns best finalized block number.
	pub async fn on_chain_best_finalized_block_number(
		&self,
	) -> Result<BlockNumberOf<P::SourceChain>, Error> {
		// we **CAN** continue to relay finality proofs if source node is out of sync, because
		// target node may be missing proofs that are already available at the source
		self.client.best_finalized_header_number().await
	}

	/// Return header and its justification of the given block or its earlier descendant that
	/// has a GRANDPA justification.
	///
	/// This method is optimized for cases when `block_number` is close to the best finalized
	/// chain block.
	pub async fn prove_block_finality(
		&self,
		block_number: BlockNumberOf<P::SourceChain>,
	) -> Result<
		(relay_substrate_client::SyncHeader<HeaderOf<P::SourceChain>>, SubstrateFinalityProof<P>),
		Error,
	> {
		// when we talk about GRANDPA finality:
		//
		// only mandatory headers have persistent notifications (that are returned by the
		// `header_and_finality_proof` call). Since this method is supposed to work with arbitrary
		// headers, we can't rely only on persistent justifications. So let's start with subscribing
		// to ephemeral justifications to avoid waiting too much if we have failed to find
		// persistent one.
		let best_finalized_block_number = self.client.best_finalized_header_number().await?;
		let mut finality_proofs = self.finality_proofs().await?;

		// start searching for persistent justificaitons
		let mut current_block_number = block_number;
		while current_block_number <= best_finalized_block_number {
			let (header, maybe_proof) =
				self.header_and_finality_proof(current_block_number).await?;
			match maybe_proof {
				Some(proof) => return Ok((header, proof)),
				None => {
					current_block_number += One::one();
				},
			}
		}

		// we have failed to find persistent justification, so let's try with ephemeral
		while let Some(proof) = finality_proofs.next().await {
			// this is just for safety, in practice we shall never get notifications for earlier
			// headers here (if `block_number <= best_finalized_block_number` of course)
			if proof.target_header_number() < block_number {
				continue
			}

			let header = self.client.header_by_number(proof.target_header_number()).await?;
			return Ok((header.into(), proof))
		}

		Err(Error::FailedToFindFinalityProof(block_number.unique_saturated_into()))
	}
}

impl<P: SubstrateFinalitySyncPipeline> Clone for SubstrateFinalitySource<P> {
	fn clone(&self) -> Self {
		SubstrateFinalitySource {
			client: self.client.clone(),
			maximal_header_number: self.maximal_header_number.clone(),
		}
	}
}

#[async_trait]
impl<P: SubstrateFinalitySyncPipeline> RelayClient for SubstrateFinalitySource<P> {
	type Error = Error;

	async fn reconnect(&mut self) -> Result<(), Error> {
		self.client.reconnect().await
	}
}

#[async_trait]
impl<P: SubstrateFinalitySyncPipeline> SourceClient<FinalitySyncPipelineAdapter<P>>
	for SubstrateFinalitySource<P>
{
	type FinalityProofsStream = SubstrateFinalityProofsStream<P>;

	async fn best_finalized_block_number(&self) -> Result<BlockNumberOf<P::SourceChain>, Error> {
		let mut finalized_header_number = self.on_chain_best_finalized_block_number().await?;
		// never return block number larger than requested. This way we'll never sync headers
		// past `maximal_header_number`
		if let Some(ref maximal_header_number) = self.maximal_header_number {
			let maximal_header_number = *maximal_header_number.lock().await;
			if finalized_header_number > maximal_header_number {
				finalized_header_number = maximal_header_number;
			}
		}
		Ok(finalized_header_number)
	}

	async fn header_and_finality_proof(
		&self,
		number: BlockNumberOf<P::SourceChain>,
	) -> Result<
		(
			relay_substrate_client::SyncHeader<HeaderOf<P::SourceChain>>,
			Option<SubstrateFinalityProof<P>>,
		),
		Error,
	> {
		let header_hash = self.client.block_hash_by_number(number).await?;
		let signed_block = self.client.get_block(Some(header_hash)).await?;

		let justification = signed_block
			.justification(P::FinalityEngine::ID)
			.map(|raw_justification| {
				SubstrateFinalityProof::<P>::decode(&mut raw_justification.as_slice())
			})
			.transpose()
			.map_err(Error::ResponseParseFailed)?;

		Ok((signed_block.header().into(), justification))
	}

	async fn finality_proofs(&self) -> Result<Self::FinalityProofsStream, Error> {
		Ok(unfold(
			P::FinalityEngine::finality_proofs(&self.client).await?,
			move |subscription| async move {
				loop {
					let log_error = |err| {
						log::error!(
							target: "bridge",
							"Failed to read justification target from the {} justifications stream: {:?}",
							P::SourceChain::NAME,
							err,
						);
					};

					let next_justification = subscription
						.next()
						.await
						.map_err(|err| log_error(err.to_string()))
						.ok()??;

					let decoded_justification =
						<P::FinalityEngine as Engine<P::SourceChain>>::FinalityProof::decode(
							&mut &next_justification[..],
						);

					let justification = match decoded_justification {
						Ok(j) => j,
						Err(err) => {
							log_error(format!("decode failed with error {err:?}"));
							continue
						},
					};

					return Some((justification, subscription))
				}
			},
		)
		.boxed())
	}
}
