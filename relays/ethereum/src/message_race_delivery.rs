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

use crate::message_lane::{MessageLane, SourceHeaderIdOf, TargetHeaderIdOf};
use crate::message_lane_loop::{
	SourceClient as MessageLaneSourceClient, SourceClientState, TargetClient as MessageLaneTargetClient,
	TargetClientState,
};
use crate::message_race_loop::{MessageRace, RaceState, RaceStrategy, SourceClient, TargetClient};
use crate::utils::FailedClient;

use async_trait::async_trait;
use futures::stream::FusedStream;
use std::{collections::VecDeque, marker::PhantomData, ops::RangeInclusive};

/// Maximal number of messages to relay in single transaction.
const MAX_MESSAGES_TO_RELAY_IN_SINGLE_TX: u32 = 4;

/// Run message delivery race.
pub async fn run<P: MessageLane>(
	source_client: impl MessageLaneSourceClient<P>,
	source_state_updates: impl FusedStream<Item = SourceClientState<P>>,
	target_client: impl MessageLaneTargetClient<P>,
	target_state_updates: impl FusedStream<Item = TargetClientState<P>>,
) -> Result<(), FailedClient> {
	crate::message_race_loop::run(
		MessageDeliveryRaceSource {
			client: source_client,
			_phantom: Default::default(),
		},
		source_state_updates,
		MessageDeliveryRaceTarget {
			client: target_client,
			_phantom: Default::default(),
		},
		target_state_updates,
		MessageDeliveryStrategy::<P>::default(),
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
	_phantom: PhantomData<P>,
}

#[async_trait(?Send)]
impl<P, C> SourceClient<MessageDeliveryRace<P>> for MessageDeliveryRaceSource<P, C>
where
	P: MessageLane,
	C: MessageLaneSourceClient<P>,
{
	type Error = C::Error;

	async fn latest_nonce(
		&self,
		at_block: SourceHeaderIdOf<P>,
	) -> Result<(SourceHeaderIdOf<P>, P::MessageNonce), Self::Error> {
		self.client.latest_generated_nonce(at_block).await
	}

	async fn generate_proof(
		&self,
		at_block: SourceHeaderIdOf<P>,
		nonces: RangeInclusive<P::MessageNonce>,
	) -> Result<(SourceHeaderIdOf<P>, RangeInclusive<P::MessageNonce>, P::MessagesProof), Self::Error> {
		self.client.prove_messages(at_block, nonces).await
	}
}

/// Message delivery race target, which is a target of the lane.
struct MessageDeliveryRaceTarget<P: MessageLane, C> {
	client: C,
	_phantom: PhantomData<P>,
}

#[async_trait(?Send)]
impl<P, C> TargetClient<MessageDeliveryRace<P>> for MessageDeliveryRaceTarget<P, C>
where
	P: MessageLane,
	C: MessageLaneTargetClient<P>,
{
	type Error = C::Error;

	async fn latest_nonce(
		&self,
		at_block: TargetHeaderIdOf<P>,
	) -> Result<(TargetHeaderIdOf<P>, P::MessageNonce), Self::Error> {
		self.client.latest_received_nonce(at_block).await
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

/// Message delivery strategy.
struct MessageDeliveryStrategy<P: MessageLane> {
	/// All queued nonces.
	source_queue: VecDeque<(SourceHeaderIdOf<P>, P::MessageNonce)>,
	/// Best nonce known to target node.
	target_nonce: P::MessageNonce,
	/// Unused generic types dump.
	_phantom: PhantomData<P>,
}

impl<P: MessageLane> Default for MessageDeliveryStrategy<P> {
	fn default() -> Self {
		MessageDeliveryStrategy {
			source_queue: VecDeque::new(),
			target_nonce: Default::default(),
			_phantom: Default::default(),
		}
	}
}

impl<P: MessageLane> RaceStrategy<SourceHeaderIdOf<P>, TargetHeaderIdOf<P>, P::MessageNonce, P::MessagesProof>
	for MessageDeliveryStrategy<P>
{
	fn source_nonce_updated(&mut self, at_block: SourceHeaderIdOf<P>, nonce: P::MessageNonce) {
		if nonce <= self.target_nonce {
			return;
		}

		match self.source_queue.back() {
			Some((_, prev_nonce)) if *prev_nonce != nonce => (),
			Some(_) => return,
			None => (),
		}

		self.source_queue.push_back((at_block, nonce))
	}

	fn target_nonce_updated(
		&mut self,
		nonce: P::MessageNonce,
		race_state: &mut RaceState<SourceHeaderIdOf<P>, TargetHeaderIdOf<P>, P::MessageNonce, P::MessagesProof>,
	) {
		while self
			.source_queue
			.front()
			.map(|(_, source_nonce)| *source_nonce <= nonce)
			.unwrap_or(false)
		{
			self.source_queue.pop_front();
		}

		let need_to_select_new_nonces = race_state
			.nonces_to_submit
			.as_ref()
			.map(|(_, nonces, _)| *nonces.end() <= nonce)
			.unwrap_or(false);
		if need_to_select_new_nonces {
			race_state.nonces_to_submit = None;
		}

		let need_new_nonces_to_submit = race_state
			.nonces_submitted
			.as_ref()
			.map(|nonces| *nonces.end() <= nonce)
			.unwrap_or(false);
		if need_new_nonces_to_submit {
			race_state.nonces_submitted = None;
		}

		self.target_nonce = nonce;
	}

	fn select_nonces_to_deliver(
		&mut self,
		race_state: &mut RaceState<SourceHeaderIdOf<P>, TargetHeaderIdOf<P>, P::MessageNonce, P::MessagesProof>,
	) -> Option<RangeInclusive<P::MessageNonce>> {
		// if we have already selected nonces that we want to submit, do nothing
		if race_state.nonces_to_submit.is_some() {
			return None;
		}

		// if we already submitted some nonces, do nothing
		if race_state.nonces_submitted.is_some() {
			return None;
		}

		// 1) we want to deliver all nonces, starting from `target_nonce + 1`
		// 2) we want to deliver at most `MAX_MESSAGES_TO_RELAY_IN_SINGLE_TX` nonces in this batch
		// 3) we can't deliver new nonce until header, that has emitted this nonce, is finalized
		// by target client
		let nonces_begin = self.target_nonce + 1.into();
		let best_header_at_target = &race_state.target_state.as_ref()?.best_peer;
		let mut nonces_end = None;
		for i in 0..MAX_MESSAGES_TO_RELAY_IN_SINGLE_TX {
			let nonce = nonces_begin + i.into();

			// if queue is empty, we don't need to prove anything
			let (first_queued_at, first_queued_nonce) = match self.source_queue.front() {
				Some((first_queued_at, first_queued_nonce)) => (first_queued_at.clone(), *first_queued_nonce),
				None => break,
			};

			// if header that has queued the message is not yet finalized at bridged chain,
			// we can't prove anything
			if first_queued_at.0 > best_header_at_target.0 {
				break;
			}

			// ok, we may deliver this nonce
			nonces_end = Some(nonce);

			// probably remove it from the queue?
			if nonce == first_queued_nonce {
				self.source_queue.pop_front();
			}
		}

		nonces_end.map(|nonces_end| RangeInclusive::new(nonces_begin, nonces_end))
	}
}
