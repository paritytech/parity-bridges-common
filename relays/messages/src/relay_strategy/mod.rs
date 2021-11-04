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

use std::ops::Range;

use async_trait::async_trait;

use bp_messages::{MessageNonce, Weight};

use crate::{
	message_lane::MessageLane,
	message_lane_loop::{
		MessageDetailsMap, SourceClient as MessageLaneSourceClient,
		TargetClient as MessageLaneTargetClient,
	},
	message_race_strategy::SourceRangesQueue,
};

pub use self::{altruistic_strategy::*, mix_strategy::*, rational_strategy::*};

mod altruistic_strategy;
mod mix_strategy;
mod rational_strategy;

/// Relayer strategy trait
#[async_trait]
pub trait RelayStrategy: 'static + Clone + Send + Sync {
	/// The relayer decide how to process nonce by reference.
	/// From given set of source nonces, that are ready to be delivered, select nonces
	/// to fit into single delivery transaction.
	///
	/// The function returns last nonce that must be delivered to the target chain.
	async fn decide<
		P: MessageLane,
		SourceClient: MessageLaneSourceClient<P>,
		TargetClient: MessageLaneTargetClient<P>,
	>(
		&self,
		reference: RelayReference<P, SourceClient, TargetClient>,
	) -> Option<MessageNonce>;
}

/// Relay reference data
pub struct RelayReference<
	P: MessageLane,
	SourceClient: MessageLaneSourceClient<P>,
	TargetClient: MessageLaneTargetClient<P>,
> {
	/// Maximal number of relayed messages in single delivery transaction.
	pub max_messages_in_this_batch: MessageNonce,
	/// Maximal cumulative dispatch weight of relayed messages in single delivery transaction.
	pub max_messages_weight_in_single_batch: Weight,
	/// Maximal cumulative size of relayed messages in single delivery transaction.
	pub max_messages_size_in_single_batch: u32,
	/// The client that is connected to the message lane source node.
	pub lane_source_client: SourceClient,
	/// The client that is connected to the message lane target node.
	pub lane_target_client: TargetClient,
	/// Source queue.
	pub nonces_queue: SourceRangesQueue<
		P::SourceHeaderHash,
		P::SourceHeaderNumber,
		MessageDetailsMap<P::SourceChainBalance>,
	>,
	/// Source queue range
	pub nonces_queue_range: Range<usize>,
}
