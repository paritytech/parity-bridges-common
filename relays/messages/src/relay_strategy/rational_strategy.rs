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

//! Rational relay strategy

use async_trait::async_trait;
use num_traits::{SaturatingAdd, Zero};

use bp_messages::{MessageNonce, Weight};
use bp_runtime::messages::DispatchFeePayment;

use crate::{
	message_lane::MessageLane,
	message_lane_loop::{
		SourceClient as MessageLaneSourceClient, TargetClient as MessageLaneTargetClient,
	},
	message_race_loop::NoncesRange,
	relay_strategy::{RelayReference, RelayStrategy},
};

/// The relayer will deliver all messages and confirmations as long as he's not losing any
/// funds.
#[derive(Clone)]
pub struct RationalStrategy;

#[async_trait]
impl RelayStrategy for RationalStrategy {
	async fn decide<
		P: MessageLane,
		SourceClient: MessageLaneSourceClient<P>,
		TargetClient: MessageLaneTargetClient<P>,
	>(
		&self,
		reference: RelayReference<P, SourceClient, TargetClient>,
	) -> Option<MessageNonce> {
		let mut soft_selected_count = 0;

		let mut selected_unpaid_weight: Weight = 0;
		let mut selected_prepaid_nonces = 0;
		let mut selected_reward = P::SourceChainBalance::zero();
		let mut selected_size: u32 = 0;
		let mut selected_cost = P::SourceChainBalance::zero();
		let mut selected_count: MessageNonce = 0;

		let mut total_reward = P::SourceChainBalance::zero();
		let mut total_confirmations_cost = P::SourceChainBalance::zero();
		let mut total_cost = P::SourceChainBalance::zero();

		let hard_selected_begin_nonce =
			reference.nonces_queue[reference.nonces_queue_range.start].1.begin();

		// technically, multiple confirmations will be delivered in a single transaction,
		// meaning less loses for relayer. But here we don't know the final relayer yet, so
		// we're adding a separate transaction for every message. Normally, this cost is covered
		// by the message sender. Probably reconsider this?
		let confirmation_transaction_cost =
			reference.lane_source_client.estimate_confirmation_transaction().await;

		let all_ready_nonces = reference
			.nonces_queue
			.range(reference.nonces_queue_range.clone())
			.flat_map(|(_, ready_nonces)| ready_nonces.iter())
			.enumerate();
		for (index, (nonce, details)) in all_ready_nonces {
			// limit messages in the batch by size
			let new_selected_size = match selected_size.checked_add(details.size) {
				Some(new_selected_size)
					if new_selected_size <= reference.max_messages_size_in_single_batch =>
					new_selected_size,
				new_selected_size if selected_count == 0 => {
					log::warn!(
						target: "bridge",
						"Going to submit message delivery transaction with message \
						size {:?} that overflows maximal configured size {}",
						new_selected_size,
						reference.max_messages_size_in_single_batch,
					);
					new_selected_size.unwrap_or(u32::MAX)
				},
				_ => break,
			};
			// limit number of messages in the batch
			let new_selected_count = selected_count + 1;
			if new_selected_count > reference.max_messages_in_this_batch {
				break
			}

			// If dispatch fee has been paid at the source chain, it means that it is **relayer**
			// who's paying for dispatch at the target chain AND reward must cover this dispatch
			// fee.
			//
			// If dispatch fee is paid at the target chain, it means that it'll be withdrawn from
			// the dispatch origin account AND reward is not covering this fee.
			//
			// So in the latter case we're not adding the dispatch weight to the delivery
			// transaction weight.
			let mut new_selected_prepaid_nonces = selected_prepaid_nonces;
			let new_selected_unpaid_weight = match details.dispatch_fee_payment {
				DispatchFeePayment::AtSourceChain => {
					new_selected_prepaid_nonces += 1;
					selected_unpaid_weight.saturating_add(details.dispatch_weight)
				},
				DispatchFeePayment::AtTargetChain => selected_unpaid_weight,
			};

			// now the message has passed all 'strong' checks, and we CAN deliver it. But do we WANT
			// to deliver it? It depends on the relayer strategy.

			let delivery_transaction_cost = reference
				.lane_target_client
				.estimate_delivery_transaction_in_source_tokens(
					hard_selected_begin_nonce..=(hard_selected_begin_nonce + index as MessageNonce),
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
				soft_selected_count = index + 1;
				selected_reward = total_reward;
				selected_cost = total_cost;
			}

			selected_size = new_selected_size;
			selected_count = new_selected_count;
		}

		if soft_selected_count != 0 {
			log::trace!(
				target: "bridge",
				"Expected reward from delivering nonces [{:?}; ?] is: {:?} - {:?} = {:?}",
				hard_selected_begin_nonce,
				selected_reward,
				selected_cost,
				selected_reward - selected_cost,
			);
		}
		Some(soft_selected_count as MessageNonce)
	}
}
