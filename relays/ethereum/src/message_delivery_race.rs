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

//! Message delivery race within one-way message lane.

use crate::message_lane::{
	MessageLane, Race,
	SourceHeaderIdOf,
};
use crate::message_lane_loop::{
	ClientsState,
	SourceClient,
	TargetClient,
};
use crate::utils::{process_future_result, retry_backoff, FailedClient};

use futures::{
	future::FutureExt,
	stream::{FusedStream, StreamExt},
};

/// Run race loop until connection with target or source node is lost.
pub async fn run<P: MessageLane>(
	race: &mut Race<SourceHeaderIdOf<P>, P::MessageNonce, P::MessagesProof>,
	source_client: impl SourceClient<P>,
	target_client: impl TargetClient<P>,
	states: impl FusedStream<Item = ClientsState<P>>,
) -> Result<(), FailedClient> {
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
				).fail_if_connection_error(FailedClient::Source)?;
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
				).fail_if_connection_error(FailedClient::Source)?;
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
				).fail_if_connection_error(FailedClient::Target)?;
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
				).fail_if_connection_error(FailedClient::Target)?;
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
