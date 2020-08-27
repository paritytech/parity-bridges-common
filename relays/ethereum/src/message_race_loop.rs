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

//! Message confirm (either receiving or processing) race within one-way message lane.

// Until there'll be actual message-lane in the runtime.
#![allow(dead_code)]

use crate::message_lane_loop::ClientState;
use crate::utils::{process_future_result, retry_backoff, FailedClient, MaybeConnectionError};

use async_trait::async_trait;
use futures::{
	future::FutureExt,
	stream::{FusedStream, StreamExt},
};
use std::{fmt::Debug, ops::RangeInclusive};

/// One of races within lane.
pub trait MessageRace {
	/// Header id of the race source.
	type SourceHeaderId: Debug + Clone;
	/// Header id of the race source.
	type TargetHeaderId: Debug + Clone;

	/// Message nonce used in the race.
	type MessageNonce: Debug + Clone;
	/// Proof that is generated and delivered in this race.
	type Proof: Clone;

	/// Name of the race source.
	fn source_name() -> String;
	/// Name of the race target.
	fn target_name() -> String;
}

/// State of race source client.
type SourceClientState<P> = ClientState<<P as MessageRace>::SourceHeaderId, <P as MessageRace>::TargetHeaderId>;

/// State of race target client.
type TargetClientState<P> = ClientState<<P as MessageRace>::TargetHeaderId, <P as MessageRace>::SourceHeaderId>;

/// One of message lane clients, which is source client for the race.
#[async_trait(?Send)]
pub trait SourceClient<P: MessageRace> {
	/// Type of error this clients returns.
	type Error: std::fmt::Debug + MaybeConnectionError;

	/// Return latest nonce that is known to the source client.
	async fn latest_nonce(
		&self,
		at_block: P::SourceHeaderId,
	) -> Result<(P::SourceHeaderId, P::MessageNonce), Self::Error>;
	/// Generate proof for delivering to the target client.
	async fn generate_proof(
		&self,
		at_block: P::SourceHeaderId,
		nonces: RangeInclusive<P::MessageNonce>,
	) -> Result<(P::SourceHeaderId, RangeInclusive<P::MessageNonce>, P::Proof), Self::Error>;
}

/// One of message lane clients, which is target client for the race.
#[async_trait(?Send)]
pub trait TargetClient<P: MessageRace> {
	/// Type of error this clients returns.
	type Error: std::fmt::Debug + MaybeConnectionError;

	/// Return latest nonce that is known to the target client.
	async fn latest_nonce(
		&self,
		at_block: P::TargetHeaderId,
	) -> Result<(P::TargetHeaderId, P::MessageNonce), Self::Error>;
	/// Submit proof to the target client.
	async fn submit_proof(
		&self,
		generated_at_block: P::SourceHeaderId,
		nonces: RangeInclusive<P::MessageNonce>,
		proof: P::Proof,
	) -> Result<RangeInclusive<P::MessageNonce>, Self::Error>;
}

/// Race strategy.
pub trait RaceStrategy<SourceHeaderId, TargetHeaderId, MessageNonce, Proof> {
	/// Called when latest nonce is updated at source node of the race.
	fn source_nonce_updated(&mut self, at_block: SourceHeaderId, nonce: MessageNonce);
	/// Called when latest nonce is updated at target node of the race.
	fn target_nonce_updated(&mut self, nonce: MessageNonce);
	/// Should return `Some(_)` with if we need to deliver proof of given nonces from
	/// source to target node.
	fn select_nonces_to_deliver(
		&mut self,
		race_state: &mut RaceState<SourceHeaderId, TargetHeaderId, MessageNonce, Proof>,
	) -> Option<RangeInclusive<MessageNonce>>;
}

/// State of the race.
pub struct RaceState<SourceHeaderId, TargetHeaderId, MessageNonce, Proof> {
	/// Source state, if known.
	pub source_state: Option<ClientState<SourceHeaderId, TargetHeaderId>>,
	/// Target state, if known.
	pub target_state: Option<ClientState<TargetHeaderId, SourceHeaderId>>,
	/// Range of nonces that we have selected to submit.
	pub nonces_to_submit: Option<(SourceHeaderId, RangeInclusive<MessageNonce>, Proof)>,
	/// Range of nonces that is currently submitted.
	pub nonces_submitted: Option<RangeInclusive<MessageNonce>>,
}

/// Run race loop until connection with target or source node is lost.
pub async fn run<P: MessageRace>(
	race_source: impl SourceClient<P>,
	race_source_updated: impl FusedStream<Item = SourceClientState<P>>,
	race_target: impl TargetClient<P>,
	race_target_updated: impl FusedStream<Item = TargetClientState<P>>,
	mut strategy: impl RaceStrategy<P::SourceHeaderId, P::TargetHeaderId, P::MessageNonce, P::Proof>,
) -> Result<(), FailedClient> {
	let mut race_state = RaceState {
		source_state: None,
		target_state: None,
		nonces_to_submit: None,
		nonces_submitted: None,
	};

	let mut source_retry_backoff = retry_backoff();
	let mut source_client_is_online = false;
	let mut source_latest_nonce_required = false;
	let source_latest_nonce = futures::future::Fuse::terminated();
	let source_generate_proof = futures::future::Fuse::terminated();
	let source_go_offline_future = futures::future::Fuse::terminated();

	let mut target_retry_backoff = retry_backoff();
	let mut target_client_is_online = false;
	let mut target_latest_nonce_required = false;
	let target_latest_nonce = futures::future::Fuse::terminated();
	let target_submit_proof = futures::future::Fuse::terminated();
	let target_go_offline_future = futures::future::Fuse::terminated();

	futures::pin_mut!(
		race_source_updated,
		source_latest_nonce,
		source_generate_proof,
		source_go_offline_future,
		race_target_updated,
		target_latest_nonce,
		target_submit_proof,
		target_go_offline_future,
	);

	loop {
		futures::select! {
			// when headers ids are updated
			source_state = race_source_updated.next() => {
				if let Some(source_state) = source_state {
					race_state.source_state = Some(source_state);
				}
			},
			target_state = race_target_updated.next() => {
				if let Some(target_state) = target_state {
					race_state.target_state = Some(target_state);
				}
			},

			// when nonces are updated
			latest_nonce = source_latest_nonce => {
				source_latest_nonce_required = false;

				source_client_is_online = process_future_result(
					latest_nonce,
					&mut source_retry_backoff,
					|(at_block, latest_nonce)| {
						log::debug!(
							target: "bridge",
							"Received latest nonce from {}: {:?}",
							P::source_name(),
							latest_nonce,
						);

						strategy.source_nonce_updated(at_block, latest_nonce);
					},
					&mut source_go_offline_future,
					|delay| async_std::task::sleep(delay),
					|| format!("Error retrieving latest nonce from {}", P::source_name()),
				).fail_if_connection_error(FailedClient::Source)?;
			},
			latest_nonce = target_latest_nonce => {
				target_latest_nonce_required = false;

				target_client_is_online = process_future_result(
					latest_nonce,
					&mut target_retry_backoff,
					|(_, latest_nonce)| {
						log::debug!(
							target: "bridge",
							"Received latest nonce from {}: {:?}",
							P::target_name(),
							latest_nonce,
						);

						strategy.target_nonce_updated(latest_nonce);
					},
					&mut target_go_offline_future,
					|delay| async_std::task::sleep(delay),
					|| format!("Error retrieving latest nonce from {}", P::target_name()),
				).fail_if_connection_error(FailedClient::Target)?;
			},

			// proof generation and submittal
			proof = source_generate_proof => {
				source_client_is_online = process_future_result(
					proof,
					&mut source_retry_backoff,
					|(at_block, nonces_range, proof)| {
						log::debug!(
							target: "bridge",
							"Received proof for nonces in range {:?} from {}",
							nonces_range,
							P::source_name(),
						);

						race_state.nonces_to_submit = Some((at_block, nonces_range, proof));
					},
					&mut source_go_offline_future,
					|delay| async_std::task::sleep(delay),
					|| format!("Error generating proof at {}", P::source_name()),
				).fail_if_connection_error(FailedClient::Source)?;
			},
			proof_submit_result = target_submit_proof => {
				target_client_is_online = process_future_result(
					proof_submit_result,
					&mut target_retry_backoff,
					|nonces_range| {
						log::debug!(
							target: "bridge",
							"Successfully submitted proof of nonces {:?} to {}",
							nonces_range,
							P::target_name(),
						);

						race_state.nonces_to_submit = None;
						race_state.nonces_submitted = Some(nonces_range);
					},
					&mut target_go_offline_future,
					|delay| async_std::task::sleep(delay),
					|| format!("Error submitting proof {}", P::target_name()),
				).fail_if_connection_error(FailedClient::Target)?;
			}
		}

		if source_client_is_online {
			source_client_is_online = false;

			if let Some(nonces_range) = strategy.select_nonces_to_deliver(&mut race_state) {
				log::debug!(
					target: "bridge",
					"Asking {} to prove nonces in range {:?}",
					P::source_name(),
					nonces_range,
				);
				let at_block = race_state.source_state.as_ref().expect("TODO").best_self.clone();
				source_generate_proof.set(race_source.generate_proof(at_block, nonces_range).fuse());
			} else if source_latest_nonce_required {
				log::debug!(target: "bridge", "Asking {} about latest generated message nonce", P::source_name());
				let at_block = race_state.source_state.as_ref().expect("TODO").best_self.clone();
				source_latest_nonce.set(race_source.latest_nonce(at_block).fuse());
			} else {
				source_client_is_online = true;
			}
		}

		if target_client_is_online {
			target_client_is_online = false;

			if let Some((at_block, nonces_range, proof)) = race_state.nonces_to_submit.as_ref() {
				log::debug!(
					target: "bridge",
					"Going to submit proof of messages in range {:?} to {} node",
					nonces_range,
					P::target_name(),
				);
				target_submit_proof.set(
					race_target
						.submit_proof(at_block.clone(), nonces_range.clone(), proof.clone())
						.fuse(),
				);
			}
			if target_latest_nonce_required {
				log::debug!(target: "bridge", "Asking {} about latest nonce", P::target_name());
				let at_block = race_state.target_state.as_ref().expect("TODO").best_self.clone();
				target_latest_nonce.set(race_target.latest_nonce(at_block).fuse());
			} else {
				target_client_is_online = true;
			}
		}
	}
}
/*
/// Select messages to deliver.
fn select_messages_to_deliver<P: MessageLane>(
	state: &ClientsState<P>,
	race: &mut Race<SourceHeaderIdOf<P>, P::MessageNonce, P::MessagesProof>,
) -> Option<(P::MessageNonce, P::MessageNonce)> {
	// maybe there are no new messages at all?
	if race.nonce_at_start <= race.nonce_at_end {
		return None;
	}

	// if we have already prepared some proof, wait until it is accepted
	if race.proof.is_some() {
		return None;
	}

	// if we have something that has been submitted, but haven't reached target chain yet, do nothing
	if let Some(submitted) = race.submitted.as_ref() {
		if *submitted > race.nonce_at_end {
			// TODO: restart sync if stalled
			return None;
		}
	}

	// we do not want to prove anything until we know that the block where message has been seen
	// first time (at least by us), is finalized by the target node
	if race.nonce_at_start_block.0 < state.target.as_ref()?.best_peer.0 {
		return None;
	}

	// TODO: use const/param here
	Some((
		race.nonce_at_end + 1.into(),
		std::cmp::min(race.nonce_at_end + 5.into(), race.nonce_at_start),
	))
}
*/
