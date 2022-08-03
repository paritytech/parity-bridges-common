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
use bp_messages::MessageNonce;

use crate::{
	message_lane::MessageLane,
	message_lane_loop::{
		SourceClient as MessageLaneSourceClient, TargetClient as MessageLaneTargetClient,
	},
	relay_strategy::{RelayMessagesBatchReference, RelayReference, RelayStrategy},
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
		&mut self,
		reference: &mut RelayReference<P, SourceClient, TargetClient>,
	) -> bool {
		let delivery_transaction_cost = reference
			.lane_target_client
			.estimate_delivery_transaction_in_source_tokens(
				reference.hard_selected_begin_nonce..=
					(reference.hard_selected_begin_nonce + reference.index as MessageNonce),
				reference.selected_prepaid_nonces,
				reference.selected_unpaid_weight,
				reference.selected_size as u32,
			)
			.await;

		true
	}

	async fn final_decision<
		P: MessageLane,
		SourceClient: MessageLaneSourceClient<P>,
		TargetClient: MessageLaneTargetClient<P>,
	>(
		&self,
		reference: &RelayMessagesBatchReference<P, SourceClient, TargetClient>,
		selected_max_nonce: MessageNonce,
	) {
		// TODO
		// if let Some(ref metrics) = reference.metrics {
		// 	let total_cost = estimate_messages_delivery_cost(reference);
		// 	metrics.note_unprofitable_delivery_transactions();
		// }
	}
}
