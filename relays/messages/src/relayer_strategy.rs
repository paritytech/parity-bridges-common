use bp_messages::MessageNonce;
use num_traits::{SaturatingAdd, Zero};

use crate::{
	message_lane::MessageLane,
	message_lane_loop::{
		RelayerMode, SourceClient as MessageLaneSourceClient,
		TargetClient as MessageLaneTargetClient,
	},
};

pub trait RelayerStrategy {
	fn decide<
		P: MessageLane,
		SourceClient: MessageLaneSourceClient<P>,
		TargetClient: MessageLaneTargetClient<P>,
	>(
		reference: RelayerReference<P, SourceClient, TargetClient>,
	) -> Option<RelayerDecide<P>>;
}

pub struct RelayerReference<
	P: MessageLane,
	SourceClient: MessageLaneSourceClient<P>,
	TargetClient: MessageLaneTargetClient<P>,
> {
	pub relayer_mode: RelayerMode,
	pub lane_source_client: SourceClient,
	pub lane_target_client: TargetClient,
}

pub struct RelayerDecide<P: MessageLane> {
	pub participate: bool,
	pub total_reward: Option<P::SourceChainBalance>,
	pub total_cost: Option<P::SourceChainBalance>,
}

pub struct DefaultRelayerStrategy {}

impl RelayerStrategy for DefaultRelayerStrategy {
	fn decide<
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
						hard_selected_begin_nonce..=
							(hard_selected_begin_nonce + index as MessageNonce),
						new_selected_prepaid_nonces,
						new_selected_unpaid_weight,
						new_selected_size as u32,
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
				total_reward = total_reward.saturating_add(&details.reward);
				total_cost = total_confirmations_cost.saturating_add(&delivery_transaction_cost);
				if !is_total_reward_less_than_cost && total_reward < total_cost {
					log::debug!(
						target: "bridge",
						"Message with nonce {} (reward = {:?}) changes total cost {:?}->{:?} and makes it larger than \
						total reward {:?}->{:?}",
						nonce,
						details.reward,
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
						nonce,
						details.reward,
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
