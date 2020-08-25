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

//! Message delivery loop. Designed to work with message-lane pallet.
//!
//! Single relay instance delivers messages of single lane in single direction.
//! To serve two-way lane, you would need two instances of relay.
//! To serve N two-way lanes, you would need N*2 instances of relay.
//!
//! Please keep in mind that the best header in this file is actually best
//! finalized header. I.e. when talking about headers in lane context, we
//! only care about finalized headers.

// Until there'll be actual message-lane in the runtime.
#![allow(dead_code)]

use crate::sync_types::HeaderId;
use crate::utils::{interval, retry_backoff, MaybeConnectionError};

use async_trait::async_trait;
use backoff::{backoff::Backoff, ExponentialBackoff};
use futures::{
	channel::mpsc::unbounded,
	future::FutureExt,
	stream::{FusedStream, StreamExt},
};
use std::{fmt::Debug, future::Future, time::Duration};

/// Delay after connection-related error happened before we'll try
/// reconnection again.
const CONNECTION_ERROR_DELAY: Duration = Duration::from_secs(10);

enum Error {
	Source,
	Target,
}

/// One-way message lane.
pub trait MessageLane {
	/// Name of the messages source.
	const SOURCE_NAME: &'static str;
	/// Name of the messages target.
	const TARGET_NAME: &'static str;

	/// Message nonce type.
	type MessageNonce: Clone + Copy + Debug + Default + From<u32> + Ord + std::ops::Add<Output = Self::MessageNonce>;

	/// Messages proof.
	type MessagesProof;

	/// Number of the source header.
	type SourceHeaderNumber: Clone + Debug + Default + Ord + PartialEq;
	/// Hash of the source header.
	type SourceHeaderHash: Clone + Debug + Default + PartialEq;

	/// Number of the target header.
	type TargetHeaderNumber: Clone + Debug + Default + Ord + PartialEq;
	/// Hash of the target header.
	type TargetHeaderHash: Clone + Debug + Default + PartialEq;
}

/// Source header id withing given one-way message lane.
type SourceHeaderIdOf<P> = HeaderId<<P as MessageLane>::SourceHeaderHash, <P as MessageLane>::SourceHeaderNumber>;

/// Target header id withing given one-way message lane.
type TargetHeaderIdOf<P> = HeaderId<<P as MessageLane>::TargetHeaderHash, <P as MessageLane>::TargetHeaderNumber>;

/// State of the client.
#[derive(Debug, Default, PartialEq)]
pub struct ClientState<SelfHeaderId, PeerHeaderId> {
	/// Best header id of this chain.
	pub best_self: SelfHeaderId,
	/// Best header id of the peer chain.
	pub best_peer: PeerHeaderId,
}

/// State of source client in one-way message lane.
type SourceClientState<P> = ClientState<SourceHeaderIdOf<P>, TargetHeaderIdOf<P>>;

/// State of target client in one-way message lane.
type TargetClientState<P> = ClientState<TargetHeaderIdOf<P>, SourceHeaderIdOf<P>>;

/// Both clients state.
#[derive(Debug, Default)]
struct ClientsState<P: MessageLane> {
	/// Source client state.
	pub source: Option<SourceClientState<P>>,
	/// Target client state.
	pub target: Option<TargetClientState<P>>,
}

/// Source client trait.
#[async_trait]
pub trait SourceClient<P: MessageLane>: Clone {
	/// Type of error this clients returns.
	type Error: std::fmt::Debug + MaybeConnectionError;

	/// Try to reconnect to source node.
	fn reconnect(self) -> Self;

	/// Returns state of the client.
	async fn state(&self) -> Result<SourceClientState<P>, Self::Error>;

	/// Get nonce of instance of latest generated message.
	async fn latest_generated_nonce(
		&self,
		id: SourceHeaderIdOf<P>,
	) -> Result<(SourceHeaderIdOf<P>, P::MessageNonce), Self::Error>;

	/// Prove messages in inclusive range [begin; end].
	async fn prove_messages(
		&self,
		id: SourceHeaderIdOf<P>,
		nonces_begin: P::MessageNonce,
		nonces_end: P::MessageNonce,
	) -> Result<(SourceHeaderIdOf<P>, P::MessageNonce, P::MessageNonce, P::MessagesProof), Self::Error>;
}

/// Target client trait.
#[async_trait]
pub trait TargetClient<P: MessageLane>: Clone {
	/// Type of error this clients returns.
	type Error: std::fmt::Debug + MaybeConnectionError;

	/// Try to reconnect to source node.
	fn reconnect(self) -> Self;

	/// Returns state of the client.
	async fn state(&self) -> Result<TargetClientState<P>, Self::Error>;

	/// Get nonce of latest message, which receival has been confirmed.
	async fn latest_received_nonce(
		&self,
		id: TargetHeaderIdOf<P>,
	) -> Result<(TargetHeaderIdOf<P>, P::MessageNonce), Self::Error>;

	/// Submit messages proof.
	async fn submit_messages_proof(
		&self,
		proof_header: SourceHeaderIdOf<P>,
		nonces_begin: P::MessageNonce,
		nonces_end: P::MessageNonce,
		proof: P::MessagesProof,
	) -> Result<(P::MessageNonce, P::MessageNonce), Self::Error>;
}

/// Withing single one-way lane we have three 'races' where we try to:
///
/// 1) relay new messages from source to target node;
/// 2) relay proof-of-receiving from target to source node;
/// 3) relay proof-of-processing from target no source node.
///
/// Direction of these races isn't always source -> target. So to distinguish between
/// one-way lane' source and target, let's call them race begin and race end.
#[derive(Debug)]
struct Race<Id, Nonce, Proof> {
	/// The nonce at race begin.
	pub nonce_at_start: Nonce,
	/// `nonce_at_start` has been read at this block.
	pub nonce_at_start_block: Id,
	/// Best nonce at race end.
	pub nonce_at_end: Nonce,
	/// Prepared proof, if any.
	pub proof: Option<(Id, Nonce, Nonce, Proof)>,
	/// Latest nonce that we have submitted, if any.
	pub submitted: Option<Nonce>,
}

impl<Id: Default, Nonce: Default, Proof> Default for Race<Id, Nonce, Proof> {
	fn default() -> Self {
		Race {
			nonce_at_start: Default::default(),
			nonce_at_start_block: Default::default(),
			nonce_at_end: Default::default(),
			proof: None,
			submitted: None,
		}
	}
}

/// Run one-way message delivery loop.
pub fn run<P: MessageLane>(
	mut source_client: impl SourceClient<P>,
	source_tick: Duration,
	mut target_client: impl TargetClient<P>,
	target_tick: Duration,
	exit_signal: impl Future<Output = ()>,
) {
	let mut local_pool = futures::executor::LocalPool::new();
	let exit_signal = exit_signal.shared();

	local_pool.run_until(async move {
		loop {
			let result = run_until_connection_lost(
				source_client.clone(),
				source_tick,
				target_client.clone(),
				target_tick,
				exit_signal.clone(),
			)
			.await;

			match result {
				Ok(()) => break,
				Err(Error::Source) => {
					async_std::task::sleep(CONNECTION_ERROR_DELAY).await;
					source_client = source_client.reconnect();
				}
				Err(Error::Target) => {
					async_std::task::sleep(CONNECTION_ERROR_DELAY).await;
					target_client = target_client.reconnect();
				}
			}
		}
	});
}

/// Run one-way message delivery loop until connection with target or source node is lost, or exit signal is received.
async fn run_until_connection_lost<P: MessageLane, SC: SourceClient<P>, TC: TargetClient<P>>(
	source_client: SC,
	source_tick: Duration,
	target_client: TC,
	target_tick: Duration,
	exit_signal: impl Future<Output = ()>,
) -> Result<(), Error> {
	let mut source_retry_backoff = retry_backoff();
	let mut source_client_is_online = false;
	let mut source_state_required = true;
	let source_state = source_client.state().fuse();
	let source_go_offline_future = futures::future::Fuse::terminated();
	let source_tick_stream = interval(source_tick).fuse();

	let mut target_retry_backoff = retry_backoff();
	let mut target_client_is_online = false;
	let mut target_state_required = true;
	let target_state = target_client.state().fuse();
	let target_go_offline_future = futures::future::Fuse::terminated();
	let target_tick_stream = interval(target_tick).fuse();

	let (mut delivery_race, (delivery_states_sender, delivery_states_receiver)) = (Race::default(), unbounded());
	let delivery_race_loop = run_race(
		&mut delivery_race,
		source_client.clone(),
		target_client.clone(),
		delivery_states_receiver,
	)
	.fuse();

	let exit_signal = exit_signal.fuse();

	futures::pin_mut!(
		source_state,
		source_go_offline_future,
		source_tick_stream,
		target_state,
		target_go_offline_future,
		target_tick_stream,
		delivery_race_loop,
		exit_signal
	);

	loop {
		futures::select! {
			new_source_state = source_state => {
				source_state_required = false;

				source_client_is_online = process_future_result(
					new_source_state,
					&mut source_retry_backoff,
					|new_source_state| {
						let _ = delivery_states_sender.unbounded_send(ClientsState {
							source: Some(new_source_state),
							target: None,
						});
					},
					&mut source_go_offline_future,
					|delay| async_std::task::sleep(delay),
					|| format!("Error retrieving state from {} node", P::SOURCE_NAME),
				).map_err(|_| Error::Source)?;
			},
			_ = source_go_offline_future => {
				source_client_is_online = true;
			},
			_ = source_tick_stream.next() => {
				source_state_required = true;
			},
			new_target_state = target_state => {
				target_state_required = false;

				target_client_is_online = process_future_result(
					new_target_state,
					&mut target_retry_backoff,
					|new_target_state| {
						let _ = delivery_states_sender.unbounded_send(ClientsState {
							source: None,
							target: Some(new_target_state),
						});
					},
					&mut target_go_offline_future,
					|delay| async_std::task::sleep(delay),
					|| format!("Error retrieving state from {} node", P::TARGET_NAME),
				).map_err(|_| Error::Target)?;
			},
			_ = target_go_offline_future => {
				target_client_is_online = true;
			},
			_ = target_tick_stream.next() => {
				target_state_required = true;
			},

			delivery_error = delivery_race_loop => {
				match delivery_error {
					Ok(_) => unreachable!("only ends with error; qed"),
					Err(err) => return Err(err),
				}
			}
		}

		if source_client_is_online {
			source_client_is_online = false;

			if source_state_required {
				log::debug!(target: "bridge", "Asking {} node about its state", P::SOURCE_NAME);
				source_state.set(source_client.state().fuse());
			} else {
				source_client_is_online = true;
			}
		}

		if target_client_is_online {
			target_client_is_online = false;

			if target_state_required {
				log::debug!(target: "bridge", "Asking {} node about its state", P::TARGET_NAME);
				target_state.set(target_client.state().fuse());
			} else {
				target_client_is_online = true;
			}
		}
	}
}

/// Run race loop until connection with target or source node is lost.
async fn run_race<P: MessageLane>(
	race: &mut Race<SourceHeaderIdOf<P>, P::MessageNonce, P::MessagesProof>,
	source_client: impl SourceClient<P>,
	target_client: impl TargetClient<P>,
	states: impl FusedStream<Item = ClientsState<P>>,
) -> Result<(), Error> {
	let mut state: ClientsState<P> = ClientsState {
		source: None,
		target: None,
	};

	let mut source_retry_backoff = retry_backoff();
	let mut source_client_is_online = false;
	let mut source_latest_generated_nonce_required = false;
	let source_latest_generated_nonce = futures::future::Fuse::terminated();
	let source_prove_messages = futures::future::Fuse::terminated();
	let source_go_offline_future = futures::future::Fuse::terminated();

	let mut target_retry_backoff = retry_backoff();
	let mut target_client_is_online = false;
	let mut target_latest_received_nonce_required = false;
	let target_latest_received_nonce = futures::future::Fuse::terminated();
	let target_submit_messages_proof = futures::future::Fuse::terminated();
	let target_go_offline_future = futures::future::Fuse::terminated();

	futures::pin_mut!(
		states,
		source_latest_generated_nonce,
		source_prove_messages,
		source_go_offline_future,
		target_latest_received_nonce,
		target_submit_messages_proof,
		target_go_offline_future
	);

	loop {
		futures::select! {
			state_update = states.next() => {
				if let Some(state_update) = state_update {
					if let Some(updated_source_state) = state_update.source {
						if state.source.as_ref() != Some(&updated_source_state) {
							state.source = Some(updated_source_state);
							if race.nonce_at_start == race.nonce_at_end {
								source_latest_generated_nonce_required = true;
							}
						}
					}
					if let Some(updated_target_state) = state_update.target {
						if state.target.as_ref() != Some(&updated_target_state) {
							state.target = Some(updated_target_state);
							target_latest_received_nonce_required = true;
						}
					}
				}
			},
			latest_generated_nonce = source_latest_generated_nonce => {
				source_latest_generated_nonce_required = false;

				source_client_is_online = process_future_result(
					latest_generated_nonce,
					&mut source_retry_backoff,
					|(at_header, latest_generated_nonce)| {
						log::debug!(
							target: "bridge",
							"Received latest generated message nonce from {}: {:?}",
							P::SOURCE_NAME,
							latest_generated_nonce,
						);

						race.nonce_at_start = latest_generated_nonce;
						race.nonce_at_start_block = at_header;
					},
					&mut source_go_offline_future,
					|delay| async_std::task::sleep(delay),
					|| format!("Error retrieving latest generated nonce from {} node", P::SOURCE_NAME),
				).map_err(|_| Error::Source)?;
			},
			messages_proof = source_prove_messages => {
				source_client_is_online = process_future_result(
					messages_proof,
					&mut source_retry_backoff,
					|(at_header, nonces_begin, nonces_end, messages_proof)| {
						log::debug!(
							target: "bridge",
							"Received proof of messages in range [{:?}; {:?}] from {}",
							nonces_begin,
							nonces_end,
							P::SOURCE_NAME,
						);

						race.proof = Some((at_header, nonces_begin, nonces_end, messages_proof));
					},
					&mut source_go_offline_future,
					|delay| async_std::task::sleep(delay),
					|| format!("Error retrieving latest generated nonce from {} node", P::SOURCE_NAME),
				).map_err(|_| Error::Source)?;
			},
			latest_received_nonce = target_latest_received_nonce => {
				target_latest_received_nonce_required = false;

				target_client_is_online = process_future_result(
					latest_received_nonce,
					&mut target_retry_backoff,
					|(_, latest_received_nonce)| {
						log::debug!(
							target: "bridge",
							"Received latest received message nonce from {}: {:?}",
							P::TARGET_NAME,
							latest_received_nonce,
						);

						race.nonce_at_end = latest_received_nonce;
					},
					&mut target_go_offline_future,
					|delay| async_std::task::sleep(delay),
					|| format!("Error retrieving latest received nonce from {} node", P::TARGET_NAME),
				).map_err(|_| Error::Target)?;
			},
			proof_submit_result = target_submit_messages_proof => {
				target_client_is_online = process_future_result(
					proof_submit_result,
					&mut target_retry_backoff,
					|(nonces_begin, nonces_end)| {
						log::debug!(
							target: "bridge",
							"Successfully submitted proof of messages in range [{:?}; {:?}] to {} node",
							nonces_begin,
							nonces_end,
							P::TARGET_NAME,
						);

						race.submitted = Some(nonces_end);
					},
					&mut target_go_offline_future,
					|delay| async_std::task::sleep(delay),
					|| format!("Error retrieving latest received nonce from {} node", P::TARGET_NAME),
				).map_err(|_| Error::Target)?;
			}
		}

		if source_client_is_online {
			source_client_is_online = false;

			if let Some((nonces_begin, nonces_end)) = select_messages_to_deliver(&state, race) {
				log::debug!(
					target: "bridge",
					"Asking {} to prove messages in range [{:?}; {:?}]",
					P::SOURCE_NAME,
					nonces_begin,
					nonces_end,
				);
				let at_block = state.source.as_ref().expect("TODO").best_self.clone();
				source_prove_messages.set(source_client.prove_messages(at_block, nonces_begin, nonces_end).fuse());
			} else if source_latest_generated_nonce_required {
				log::debug!(target: "bridge", "Asking {} about latest generated message nonce", P::SOURCE_NAME);
				let at_block = state.source.as_ref().expect("TODO").best_self.clone();
				source_latest_generated_nonce.set(source_client.latest_generated_nonce(at_block).fuse());
			} else {
				source_client_is_online = true;
			}
		}

		if target_client_is_online {
			target_client_is_online = false;

			if let Some((at_block, nonces_begin, nonces_end, proof)) = race.proof.take() {
				log::debug!(
					target: "bridge",
					"Going to submit proof of messages in range [{:?}; {:?}] to {} node",
					nonces_begin,
					nonces_end,
					P::TARGET_NAME,
				);
				target_submit_messages_proof.set(
					target_client
						.submit_messages_proof(at_block, nonces_begin, nonces_end, proof)
						.fuse(),
				);
			}
			if target_latest_received_nonce_required {
				log::debug!(target: "bridge", "Asking {} about latest received message nonce", P::TARGET_NAME);
				let at_block = state.target.as_ref().expect("TODO").best_self.clone();
				target_latest_received_nonce.set(target_client.latest_received_nonce(at_block).fuse());
			} else {
				target_client_is_online = true;
			}
		}
	}
}

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

/// Process result of the future from a client.
///
/// Returns whether or not the client we're interacting with is online. In this context
/// what online means is that the client is currently not handling any other requests
/// that we've previously sent.
pub(crate) fn process_future_result<TResult, TError, TGoOfflineFuture>(
	result: Result<TResult, TError>,
	retry_backoff: &mut ExponentialBackoff,
	on_success: impl FnOnce(TResult),
	go_offline_future: &mut std::pin::Pin<&mut futures::future::Fuse<TGoOfflineFuture>>,
	go_offline: impl FnOnce(Duration) -> TGoOfflineFuture,
	error_pattern: impl FnOnce() -> String,
) -> Result<bool, ()>
where
	TError: std::fmt::Debug + MaybeConnectionError,
	TGoOfflineFuture: FutureExt,
{
	let mut client_is_online = false;

	match result {
		Ok(result) => {
			on_success(result);
			retry_backoff.reset();
			client_is_online = true
		}
		Err(error) => {
			let is_connection_error = error.is_connection_error();
			let retry_delay = if is_connection_error {
				return Err(());
			} else {
				retry_backoff.next_backoff().unwrap_or(CONNECTION_ERROR_DELAY)
			};
			go_offline_future.set(go_offline(retry_delay).fuse());

			log::error!(
				target: "bridge",
				"{}: {:?}. Retrying in {}s",
				error_pattern(),
				error,
				retry_delay.as_secs_f64(),
			);
		}
	}

	Ok(client_is_online)
}
