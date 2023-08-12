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
	finality_base::{finality_proofs, SubstrateFinalityProofsStream},
};

use async_trait::async_trait;
use equivocation_detector::SourceClient;
use finality_relay::SourceClientBase;
use relay_substrate_client::{Client, Error};
use relay_utils::relay_loop::Client as RelayClient;
use std::marker::PhantomData;

/// Substrate node as equivocation source.
pub struct SubstrateEquivocationSource<P: SubstrateEquivocationDetectionPipeline, SourceClnt> {
	client: SourceClnt,

	_phantom: PhantomData<P>,
}

impl<P: SubstrateEquivocationDetectionPipeline, SourceClnt: Client<P::SourceChain>>
	SubstrateEquivocationSource<P, SourceClnt>
{
}

impl<P: SubstrateEquivocationDetectionPipeline, SourceClnt: Clone> Clone
	for SubstrateEquivocationSource<P, SourceClnt>
{
	fn clone(&self) -> Self {
		Self { client: self.client.clone(), _phantom: Default::default() }
	}
}

#[async_trait]
impl<P: SubstrateEquivocationDetectionPipeline, SourceClnt: Client<P::SourceChain>> RelayClient
	for SubstrateEquivocationSource<P, SourceClnt>
{
	type Error = Error;

	async fn reconnect(&mut self) -> Result<(), Error> {
		self.client.reconnect().await
	}
}

#[async_trait]
impl<P: SubstrateEquivocationDetectionPipeline, SourceClnt: Client<P::SourceChain>>
	SourceClientBase<EquivocationDetectionPipelineAdapter<P>>
	for SubstrateEquivocationSource<P, SourceClnt>
{
	type FinalityProofsStream = SubstrateFinalityProofsStream<P>;

	async fn finality_proofs(&self) -> Result<Self::FinalityProofsStream, Error> {
		finality_proofs::<P>(&self.client).await
	}
}

#[async_trait]
impl<P: SubstrateEquivocationDetectionPipeline, SourceClnt: Client<P::SourceChain>>
	SourceClient<EquivocationDetectionPipelineAdapter<P>>
	for SubstrateEquivocationSource<P, SourceClnt>
{
}
