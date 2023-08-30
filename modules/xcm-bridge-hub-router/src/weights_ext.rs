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

//! Weight-related utilities.

use crate::weights::WeightInfo;

use frame_support::weights::{RuntimeDbWeight, Weight};

/// Extended weight info.
pub trait WeightInfoExt: WeightInfo {
	/// TODO: move me to benchmarks.
	fn relieving_bridges_read_weight() -> Weight {
		Weight::zero() // TODO
	}

	/// TOOD: move me to benchmarks
	fn bridge_read_weight() -> Weight {
		Weight::zero() // TODO
	}

	/// TODO: move me to benchmarks.
	fn suspended_message_read_weight() -> Weight {
		Weight::zero() // TODO
	}

	/// TODO: move me to benchmarks
	fn to_bridge_hub_deliver_weight() -> Weight {
		Weight::zero() // TODO
	}

	/// Returns minimal weight required to processing suspended messages from `on_idle`.
	fn minimal_weight_to_process_suspended_messages(db_weight: &RuntimeDbWeight) -> Weight {
		// we need to read a list of relieving bridges
		Self::relieving_bridges_read_weight()
			// we need to update a list of relieving bridges
			.saturating_add(db_weight.writes(1))
			// we need to service at least one relieving bridge
			.saturating_add(Self::minimal_weight_to_process_relieving_bridge(db_weight))
	}

	/// Returns minimal weight required to process single relieving bridge from `on_idle`.
	fn minimal_weight_to_process_relieving_bridge(db_weight: &RuntimeDbWeight) -> Weight {
		// we need read a bridge
		Self::bridge_read_weight()
			// we need to update a bridge
			.saturating_add(db_weight.writes(1))
			// we need to send at least one suspended message
			.saturating_add(Self::minimal_weight_to_process_suspended_message(db_weight))
	}

	/// Returns minimal weight required to process single suspended message from `on_idle`.
	fn minimal_weight_to_process_suspended_message(db_weight: &RuntimeDbWeight) -> Weight {
		// we need to read a message
		Self::suspended_message_read_weight()
			// we need to remove a message
			.saturating_add(db_weight.writes(1))
			// we need to send a message
			.saturating_add(Self::to_bridge_hub_deliver_weight())
	}
}

impl<T: WeightInfo> WeightInfoExt for T {}
