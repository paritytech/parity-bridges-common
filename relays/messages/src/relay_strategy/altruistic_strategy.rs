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

//! Altruistic relay strategy

use async_trait::async_trait;
use num_traits::Zero;

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

/// The relayer doesn't care about rewards.
#[derive(Clone)]
pub struct AltruisticStrategy;

#[async_trait]
impl RelayStrategy for AltruisticStrategy {
	async fn decide<
		P: MessageLane,
		SourceClient: MessageLaneSourceClient<P>,
		TargetClient: MessageLaneTargetClient<P>,
	>(
		reference: RelayReference<P, SourceClient, TargetClient>,
	) -> Option<MessageNonce> {
		let mut hard_selected_count = 0;
		let mut soft_selected_count = 0;

		let mut selected_weight: Weight = 0;
		let mut selected_unpaid_weight: Weight = 0;
		let mut selected_prepaid_nonces = 0;
		let mut selected_size: u32 = 0;
		let mut selected_count: MessageNonce = 0;

		let hard_selected_begin_nonce =
			reference.nonces_queue[reference.nonces_queue_range.start].1.begin();

		let all_ready_nonces = reference
			.nonces_queue
			.range(reference.nonces_queue_range.clone())
			.flat_map(|(_, ready_nonces)| ready_nonces.iter())
			.enumerate();
		for (index, (_nonce, details)) in all_ready_nonces {
			// Since we (hopefully) have some reserves in `max_messages_weight_in_single_batch`
			// and `max_messages_size_in_single_batch`, we may still try to submit transaction
			// with single message if message overflows these limits. The worst case would be if
			// transaction will be rejected by the target runtime, but at least we have tried.

			// limit messages in the batch by weight
			let new_selected_weight = match selected_weight.checked_add(details.dispatch_weight) {
				Some(new_selected_weight)
					if new_selected_weight <= reference.max_messages_weight_in_single_batch =>
					new_selected_weight,
				new_selected_weight if selected_count == 0 => {
					log::warn!(
						target: "bridge",
						"Going to submit message delivery transaction with declared dispatch \
						weight {:?} that overflows maximal configured weight {}",
						new_selected_weight,
						reference.max_messages_weight_in_single_batch,
					);
					new_selected_weight.unwrap_or(Weight::MAX)
				},
				_ => break,
			};

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

			soft_selected_count = index + 1;

			hard_selected_count = index + 1;
			selected_weight = new_selected_weight;
			selected_unpaid_weight = new_selected_unpaid_weight;
			selected_prepaid_nonces = new_selected_prepaid_nonces;
			selected_size = new_selected_size;
			selected_count = new_selected_count;
		}

		if hard_selected_count != soft_selected_count {
			let hard_selected_end_nonce =
				hard_selected_begin_nonce + hard_selected_count as MessageNonce - 1;
			let soft_selected_begin_nonce = hard_selected_begin_nonce;
			let soft_selected_end_nonce =
				soft_selected_begin_nonce + soft_selected_count as MessageNonce - 1;
			log::warn!(
				target: "bridge",
				"Relayer may deliver nonces [{:?}; {:?}], but because of its strategy (Altruistic) it has selected \
				nonces [{:?}; {:?}].",
				hard_selected_begin_nonce,
				hard_selected_end_nonce,
				soft_selected_begin_nonce,
				soft_selected_end_nonce,
			);
			hard_selected_count = soft_selected_count;
		}

		if hard_selected_count != 0 {
			Some(hard_selected_begin_nonce + hard_selected_count as MessageNonce - 1)
		} else {
			None
		}
	}
}
