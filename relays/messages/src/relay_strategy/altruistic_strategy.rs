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
		&self,
		reference: RelayReference<P, SourceClient, TargetClient>,
	) -> Option<MessageNonce> {
		let mut soft_selected_count = 0;

		let all_ready_nonces = reference
			.nonces_queue
			.range(reference.nonces_queue_range.clone())
			.flat_map(|(_, ready_nonces)| ready_nonces.iter())
			.enumerate();
		for (index, (_nonce, _details)) in all_ready_nonces {
			soft_selected_count = index + 1;
		}

		Some(soft_selected_count as MessageNonce)
	}
}
