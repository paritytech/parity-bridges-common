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

//! Relayer strategy

use async_trait::async_trait;
use num_traits::{SaturatingAdd, Zero};

use bp_messages::{MessageNonce, Weight};

use crate::{
	message_lane::MessageLane,
	message_lane_loop::{
		MessageDetails, RelayerMode, SourceClient as MessageLaneSourceClient,
		TargetClient as MessageLaneTargetClient,
	},
};

/// Relayer strategy trait
#[async_trait]
pub trait RelayerStrategy: 'static + Clone + Send + Sync {
	/// The relayer decide how to process nonce by reference
	async fn decide<
		P: MessageLane,
		SourceClient: MessageLaneSourceClient<P>,
		TargetClient: MessageLaneTargetClient<P>,
	>(
		reference: RelayerReference<P, SourceClient, TargetClient>,
	) -> Option<RelayerDecide<P>>;
}

/// Relayer reference data
pub struct RelayerReference<
	P: MessageLane,
	SourceClient: MessageLaneSourceClient<P>,
	TargetClient: MessageLaneTargetClient<P>,
> {
	/// Relayer operating mode.
	pub relayer_mode: RelayerMode,
	/// source chain client
	pub lane_source_client: SourceClient,
	/// target chain client
	pub lane_target_client: TargetClient,
	pub hard_selected_begin_nonce: MessageNonce,
	pub new_selected_prepaid_nonces: MessageNonce,
	pub new_selected_unpaid_weight: Weight,
	pub new_selected_size: u32,
	pub ready_nonces_index: usize,
	pub ready_nonce: MessageNonce,
	pub ready_details: MessageDetails<P::SourceChainBalance>,
}

/// Relayer's decision
pub struct RelayerDecide<P: MessageLane> {
	/// Whether to participate
	pub participate: bool,
	pub total_reward: Option<P::SourceChainBalance>,
	pub total_cost: Option<P::SourceChainBalance>,
}

/// The default strategy
#[derive(Clone)]
pub struct DefaultRelayerStrategy {}

#[async_trait]
impl RelayerStrategy for DefaultRelayerStrategy {
	async fn decide<
		P: MessageLane,
		SourceClient: MessageLaneSourceClient<P>,
		TargetClient: MessageLaneTargetClient<P>,
	>(
		reference: RelayerReference<P, SourceClient, TargetClient>,
	) -> Option<RelayerDecide<P>> {
		match reference.relayer_mode {
			RelayerMode::Altruistic =>
				Some(RelayerDecide { participate: true, total_reward: None, total_cost: None }),
			RelayerMode::Rational => {
				// technically, multiple confirmations will be delivered in a single transaction,
				// meaning less loses for relayer. But here we don't know the final relayer yet, so
				// we're adding a separate transaction for every message. Normally, this cost is
				// covered by the message sender. Probably reconsider this?
				let confirmation_transaction_cost =
					reference.lane_source_client.estimate_confirmation_transaction().await;

				let mut total_reward = P::SourceChainBalance::zero();
				let mut total_confirmations_cost = P::SourceChainBalance::zero();
				let mut total_cost = P::SourceChainBalance::zero();

				let delivery_transaction_cost = reference
					.lane_target_client
					.estimate_delivery_transaction_in_source_tokens(
						reference.hard_selected_begin_nonce..=
							(reference.hard_selected_begin_nonce +
								reference.ready_nonces_index as MessageNonce),
						reference.new_selected_prepaid_nonces,
						reference.new_selected_unpaid_weight,
						reference.new_selected_size as u32,
					)
					.await
					.map_err(|err| {
						log::debug!(
							target: "bridge",
							"Failed to estimate delivery transaction cost: {:?}. No nonces selected for delivery",
							err,
						);
					})
					.ok()?;

				// if it is the first message that makes reward less than cost, let's log it
				// if this message makes batch profitable again, let's log it
				let is_total_reward_less_than_cost = total_reward < total_cost;
				let prev_total_cost = total_cost;
				let prev_total_reward = total_reward;
				total_confirmations_cost =
					total_confirmations_cost.saturating_add(&confirmation_transaction_cost);
				total_reward = total_reward.saturating_add(&reference.ready_details.reward);
				total_cost = total_confirmations_cost.saturating_add(&delivery_transaction_cost);
				if !is_total_reward_less_than_cost && total_reward < total_cost {
					log::debug!(
						target: "bridge",
						"Message with nonce {} (reward = {:?}) changes total cost {:?}->{:?} and makes it larger than \
						total reward {:?}->{:?}",
						reference.ready_nonce,
						reference.ready_details.reward,
						prev_total_cost,
						total_cost,
						prev_total_reward,
						total_reward,
					);
				} else if is_total_reward_less_than_cost && total_reward >= total_cost {
					log::debug!(
						target: "bridge",
						"Message with nonce {} (reward = {:?}) changes total cost {:?}->{:?} and makes it less than or \
						equal to the total reward {:?}->{:?} (again)",
						reference.ready_nonce,
						reference.ready_details.reward,
						prev_total_cost,
						total_cost,
						prev_total_reward,
						total_reward,
					);
				}

				// Rational relayer never want to lose his funds
				if total_reward >= total_cost {
					// soft_selected_count = index + 1;
					Some(RelayerDecide {
						participate: true,
						total_reward: Some(total_reward),
						total_cost: Some(total_cost),
					})
				} else {
					None
				}
			},
		}
	}
}
