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

use crate::message_lane::{MessageLane, SourceHeaderIdOf, TargetHeaderIdOf};
use crate::message_race_delivery::run as run_message_delivery_race;
use crate::utils::{interval, process_future_result, retry_backoff, FailedClient, MaybeConnectionError};

use async_trait::async_trait;
use futures::{channel::mpsc::unbounded, future::FutureExt, stream::StreamExt};
use std::{fmt::Debug, future::Future, ops::RangeInclusive, time::Duration};

/// Delay after connection-related error happened before we'll try
/// reconnection again.
const CONNECTION_ERROR_DELAY: Duration = Duration::from_secs(10);

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
		nonces: RangeInclusive<P::MessageNonce>,
	) -> Result<(SourceHeaderIdOf<P>, RangeInclusive<P::MessageNonce>, P::MessagesProof), Self::Error>;
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
		generated_at_header: SourceHeaderIdOf<P>,
		nonces: RangeInclusive<P::MessageNonce>,
		proof: P::MessagesProof,
	) -> Result<RangeInclusive<P::MessageNonce>, Self::Error>;
}

/// State of the client.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClientState<SelfHeaderId, PeerHeaderId> {
	/// Best header id of this chain.
	pub best_self: SelfHeaderId,
	/// Best header id of the peer chain.
	pub best_peer: PeerHeaderId,
}

/// State of source client in one-way message lane.
pub type SourceClientState<P> = ClientState<SourceHeaderIdOf<P>, TargetHeaderIdOf<P>>;

/// State of target client in one-way message lane.
pub type TargetClientState<P> = ClientState<TargetHeaderIdOf<P>, SourceHeaderIdOf<P>>;

/// Both clients state.
#[derive(Debug, Default)]
pub struct ClientsState<P: MessageLane> {
	/// Source client state.
	pub source: Option<SourceClientState<P>>,
	/// Target client state.
	pub target: Option<TargetClientState<P>>,
}

/// Run message lane service loop.
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
				Err(FailedClient::Source) => {
					async_std::task::sleep(CONNECTION_ERROR_DELAY).await;
					source_client = source_client.reconnect();
				}
				Err(FailedClient::Target) => {
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
) -> Result<(), FailedClient> {
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

	let (
		(delivery_source_state_sender, delivery_source_state_receiver),
		(delivery_target_state_sender, delivery_target_state_receiver),
	) = (unbounded(), unbounded());
	let delivery_race_loop = run_message_delivery_race(
		source_client.clone(),
		delivery_source_state_receiver,
		target_client.clone(),
		delivery_target_state_receiver,
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
						let _ = delivery_source_state_sender.unbounded_send(new_source_state);
					},
					&mut source_go_offline_future,
					|delay| async_std::task::sleep(delay),
					|| format!("Error retrieving state from {} node", P::SOURCE_NAME),
				).fail_if_connection_error(FailedClient::Source)?;
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
						let _ = delivery_target_state_sender.unbounded_send(new_target_state);
					},
					&mut target_go_offline_future,
					|delay| async_std::task::sleep(delay),
					|| format!("Error retrieving state from {} node", P::TARGET_NAME),
				).fail_if_connection_error(FailedClient::Target)?;
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
