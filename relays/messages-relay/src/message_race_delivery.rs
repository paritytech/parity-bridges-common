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

//! Message delivery race delivers proof-of-messages from lane.source to lane.target.

use crate::message_lane::{MessageLane, SourceHeaderIdOf, TargetHeaderIdOf};
use crate::message_lane_loop::{
	SourceClient as MessageLaneSourceClient, SourceClientState, TargetClient as MessageLaneTargetClient,
	TargetClientState,
};
use crate::message_race_loop::{ClientNonces, MessageRace, RaceState, RaceStrategy, SourceClient, TargetClient};
use crate::message_race_strategy::BasicStrategy;
use crate::metrics::MessageLaneLoopMetrics;

use async_trait::async_trait;
use futures::{
	future::{FutureExt, TryFutureExt},
	stream::FusedStream,
};
use relay_utils::FailedClient;
use std::{marker::PhantomData, ops::RangeInclusive, time::Duration};

/// Maximal number of messages to relay in single transaction.
const MAX_MESSAGES_TO_RELAY_IN_SINGLE_TX: u32 = 4;

/// Run message delivery race.
pub async fn run<P: MessageLane>(
	source_client: impl MessageLaneSourceClient<P>,
	source_state_updates: impl FusedStream<Item = SourceClientState<P>>,
	target_client: impl MessageLaneTargetClient<P>,
	target_state_updates: impl FusedStream<Item = TargetClientState<P>>,
	stall_timeout: Duration,
	metrics_msg: Option<MessageLaneLoopMetrics>,
) -> Result<(), FailedClient> {
	crate::message_race_loop::run(
		MessageDeliveryRaceSource {
			client: source_client,
			metrics_msg: metrics_msg.clone(),
			_phantom: Default::default(),
		},
		source_state_updates,
		MessageDeliveryRaceTarget {
			client: target_client,
			metrics_msg,
			_phantom: Default::default(),
		},
		target_state_updates,
		stall_timeout,
		MessageDeliveryStrategy::<P> {
			strategy: BasicStrategy::new(MAX_MESSAGES_TO_RELAY_IN_SINGLE_TX.into()),
		},
	)
	.await
}

/// Message delivery race.
struct MessageDeliveryRace<P>(std::marker::PhantomData<P>);

impl<P: MessageLane> MessageRace for MessageDeliveryRace<P> {
	type SourceHeaderId = SourceHeaderIdOf<P>;
	type TargetHeaderId = TargetHeaderIdOf<P>;

	type MessageNonce = P::MessageNonce;
	type Proof = P::MessagesProof;

	fn source_name() -> String {
		format!("{}::MessagesDelivery", P::SOURCE_NAME)
	}

	fn target_name() -> String {
		format!("{}::MessagesDelivery", P::TARGET_NAME)
	}
}

/// Message delivery race source, which is a source of the lane.
struct MessageDeliveryRaceSource<P: MessageLane, C> {
	client: C,
	metrics_msg: Option<MessageLaneLoopMetrics>,
	_phantom: PhantomData<P>,
}

#[async_trait]
impl<P, C> SourceClient<MessageDeliveryRace<P>> for MessageDeliveryRaceSource<P, C>
where
	P: MessageLane,
	C: MessageLaneSourceClient<P>,
{
	type Error = C::Error;

	async fn nonces(
		&self,
		at_block: SourceHeaderIdOf<P>,
	) -> Result<(SourceHeaderIdOf<P>, ClientNonces<P::MessageNonce>), Self::Error> {
		let result = self
			.client
			.latest_generated_nonce(at_block)
			.and_then(|(at_block, latest_generated_nonce)| {
				self.client
					.latest_confirmed_received_nonce(at_block)
					.map(move |result| {
						result.map(|(at_block, latest_confirmed_nonce)| {
							(at_block, latest_generated_nonce, latest_confirmed_nonce)
						})
					})
			})
			.await;
		if let Some(metrics_msg) = self.metrics_msg.as_ref() {
			if let Ok((_, source_latest_generated_nonce, _)) = result.as_ref() {
				metrics_msg.update_source_latest_generated_nonce::<P>(*source_latest_generated_nonce);
			}
		}

		result.map(|(at_block, latest_generated_nonce, latest_confirmed_nonce)| {
			(
				at_block,
				ClientNonces {
					latest_nonce: latest_generated_nonce,
					confirmed_nonce: Some(latest_confirmed_nonce),
				},
			)
		})
	}

	async fn generate_proof(
		&self,
		at_block: SourceHeaderIdOf<P>,
		nonces: RangeInclusive<P::MessageNonce>,
		additional_proof_required: bool,
	) -> Result<(SourceHeaderIdOf<P>, RangeInclusive<P::MessageNonce>, P::MessagesProof), Self::Error> {
		self.client
			.prove_messages(at_block, nonces, additional_proof_required)
			.await
	}
}

/// Message delivery race target, which is a target of the lane.
struct MessageDeliveryRaceTarget<P: MessageLane, C> {
	client: C,
	metrics_msg: Option<MessageLaneLoopMetrics>,
	_phantom: PhantomData<P>,
}

#[async_trait]
impl<P, C> TargetClient<MessageDeliveryRace<P>> for MessageDeliveryRaceTarget<P, C>
where
	P: MessageLane,
	C: MessageLaneTargetClient<P>,
{
	type Error = C::Error;

	async fn nonces(
		&self,
		at_block: TargetHeaderIdOf<P>,
	) -> Result<(TargetHeaderIdOf<P>, ClientNonces<P::MessageNonce>), Self::Error> {
		let result = self
			.client
			.latest_received_nonce(at_block)
			.and_then(|(at_block, latest_received_nonce)| {
				self.client
					.latest_confirmed_received_nonce(at_block)
					.map(move |result| {
						result.map(|(at_block, latest_confirmed_nonce)| {
							(at_block, latest_received_nonce, latest_confirmed_nonce)
						})
					})
			})
			.await;

		if let Some(metrics_msg) = self.metrics_msg.as_ref() {
			if let Ok((_, target_latest_received_nonce, target_latest_confirmed_nonce)) = result.as_ref() {
				metrics_msg.update_target_latest_received_nonce::<P>(*target_latest_received_nonce);
				metrics_msg.update_target_latest_confirmed_nonce::<P>(*target_latest_confirmed_nonce);
			}
		}

		result.map(|(at_block, latest_received_nonce, latest_confirmed_nonce)| {
			(
				at_block,
				ClientNonces {
					latest_nonce: latest_received_nonce,
					confirmed_nonce: Some(latest_confirmed_nonce),
				},
			)
		})
	}

	async fn submit_proof(
		&self,
		generated_at_block: SourceHeaderIdOf<P>,
		nonces: RangeInclusive<P::MessageNonce>,
		proof: P::MessagesProof,
	) -> Result<RangeInclusive<P::MessageNonce>, Self::Error> {
		self.client
			.submit_messages_proof(generated_at_block, nonces, proof)
			.await
	}
}

/// Messages delivery strategy.
struct MessageDeliveryStrategy<P: MessageLane> {
	/// Basic delivery strategy.
	strategy: BasicStrategy<
		<P as MessageLane>::SourceHeaderNumber,
		<P as MessageLane>::SourceHeaderHash,
		<P as MessageLane>::TargetHeaderNumber,
		<P as MessageLane>::TargetHeaderHash,
		<P as MessageLane>::MessageNonce,
		<P as MessageLane>::MessagesProof,
	>,
}

impl<P: MessageLane> RaceStrategy<SourceHeaderIdOf<P>, TargetHeaderIdOf<P>, P::MessageNonce, P::MessagesProof>
	for MessageDeliveryStrategy<P>
{
	fn is_empty(&self) -> bool {
		self.strategy.is_empty()
	}

	fn source_nonces_updated(&mut self, at_block: SourceHeaderIdOf<P>, nonces: ClientNonces<P::MessageNonce>) {
		self.strategy.source_nonces_updated(at_block, nonces)
	}

	fn target_nonces_updated(
		&mut self,
		nonces: ClientNonces<P::MessageNonce>,
		race_state: &mut RaceState<SourceHeaderIdOf<P>, TargetHeaderIdOf<P>, P::MessageNonce, P::MessagesProof>,
	) {
		self.strategy.target_nonces_updated(nonces, race_state)
	}

	fn select_nonces_to_deliver(
		&mut self,
		race_state: &RaceState<SourceHeaderIdOf<P>, TargetHeaderIdOf<P>, P::MessageNonce, P::MessagesProof>,
	) -> Option<(RangeInclusive<P::MessageNonce>, bool)> {
		self.strategy.select_nonces_to_deliver(race_state)
	}
}
