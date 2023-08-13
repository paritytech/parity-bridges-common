// Copyright 2019-2023 Parity Technologies (UK) Ltd.
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

//! Default generic implementation of equivocation source for basic Substrate client.

use crate::{
	equivocation::{EquivocationDetectionPipelineAdapter, SubstrateEquivocationDetectionPipeline},
	finality_base::engine::Engine,
};

use crate::equivocation::FinalityVerificationContextfOf;
use async_trait::async_trait;
use bp_runtime::BlockNumberOf;
use equivocation_detector::TargetClient;
use relay_substrate_client::{Client, Error};
use relay_utils::relay_loop::Client as RelayClient;
use std::marker::PhantomData;

/// Substrate node as equivocation source.
pub struct SubstrateEquivocationTarget<P: SubstrateEquivocationDetectionPipeline, TargetClnt> {
	client: TargetClnt,

	_phantom: PhantomData<P>,
}

impl<P: SubstrateEquivocationDetectionPipeline, TargetClnt: Client<P::TargetChain>>
	SubstrateEquivocationTarget<P, TargetClnt>
{
}

impl<P: SubstrateEquivocationDetectionPipeline, TargetClnt: Clone> Clone
	for SubstrateEquivocationTarget<P, TargetClnt>
{
	fn clone(&self) -> Self {
		Self { client: self.client.clone(), _phantom: Default::default() }
	}
}

#[async_trait]
impl<P: SubstrateEquivocationDetectionPipeline, TargetClnt: Client<P::TargetChain>> RelayClient
	for SubstrateEquivocationTarget<P, TargetClnt>
{
	type Error = Error;

	async fn reconnect(&mut self) -> Result<(), Error> {
		self.client.reconnect().await
	}
}

#[async_trait]
impl<P: SubstrateEquivocationDetectionPipeline, TargetClnt: Client<P::TargetChain>>
	TargetClient<EquivocationDetectionPipelineAdapter<P>>
	for SubstrateEquivocationTarget<P, TargetClnt>
{
	async fn finality_verification_context(
		&self,
		at: BlockNumberOf<P::TargetChain>,
	) -> Result<FinalityVerificationContextfOf<P>, Self::Error> {
		P::FinalityEngine::finality_verification_context(
			&self.client,
			self.client.header_hash_by_number(at).await?,
		)
		.await
	}
}
