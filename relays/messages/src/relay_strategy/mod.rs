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
use std::ops::Range;

use bp_messages::{MessageNonce, Weight};

use crate::{
	message_lane::MessageLane,
	message_lane_loop::{
		MessageDetails, MessageDetailsMap, SourceClient as MessageLaneSourceClient,
		TargetClient as MessageLaneTargetClient,
	},
	message_race_strategy::SourceRangesQueue,
};

pub mod altruistic_strategy;
pub mod rational_strategy;

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
		reference: RelayReference<P, SourceClient, TargetClient>,
	) -> Option<MessageNonce>;
}

/// Relay reference data
pub struct RelayReference<
	P: MessageLane,
	SourceClient: MessageLaneSourceClient<P>,
	TargetClient: MessageLaneTargetClient<P>,
> {
	pub max_messages_in_this_batch: MessageNonce,
	pub max_messages_weight_in_single_batch: Weight,
	pub max_messages_size_in_single_batch: u32,
	pub lane_source_client: SourceClient,
	pub lane_target_client: TargetClient,
	pub nonces_queue: SourceRangesQueue<
		P::SourceHeaderHash,
		P::SourceHeaderNumber,
		MessageDetailsMap<P::SourceChainBalance>,
	>,
	pub nonces_queue_range: Range<usize>,
}
