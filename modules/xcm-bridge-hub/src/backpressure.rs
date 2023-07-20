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

//! Backpressure mechanism for inbound XCM queues at bridge hub.
//!
//! We expect all chains that are exporting XCM messages using bridge hub to have
//! some rate limiter mechanism that will limit a number of messages across all
//! bridge queues (an HRMP/DMP queue between source chain and the source bridge hub,
//! a bridge queue between two bridge hubs and HRMP/UMP queue between target bridge
//! hub and target chain). This could be e.g. a dynamic fee that grows up when number
//! of queued messages is above some limit.
//!
//! But at bridge hub we don't have any guarantees that the sibling/parent chain is
//! using such mechanism. Instead, we introduce an artificial limit for the queue
//! between two bridge hubs and suspend inbound XCM queue when there are too many
//! messages in this queue. All local XCM queues (HRMP/UMP/DMP) have the native
//! backpressure support, so after some time new messages will start piling up at
//! the sending (sibling/parent) chain, not at the bridge hub.

use frame_support:traits::QueuePausedQuery;
use xcm::latest::prelude::*;

/// A structure that implements [`frame_support:traits::messages::QueuePausedQuery`] and may
/// be used in the `pallet-message-queue` configuration to stop processing messages when the
/// bridge queue is overloaded.
pub struct LocalXcmQueueSuspender<Origin, Inner>(PhantomData<(Origin, Inner)>);

impl<Origin, Inner> QueuePausedQuery<Origin> for LocalInboundXcmChannelSuspender where
	Origin: Clone + Into<MultiLocation>,
	Inner: QueuePausedQuery<Origin>,
{
	fn is_paused(origin: &Origin) -> bool {
		// give priority to inner status
		if Inner::is_paused(origin) {
			return true
		}

		// we assume that the messages over local XCM channel are "sent" by the same origin
		// that opens the bridge (sibling parachain or parent relay chain)
		let bridge_origin_relative_location = match T::OpenBridgeOrigin::try_origin(origin.clone()) {
			Ok(bridge_origin_relative_location) => Box::new(bridge_origin_relative_location),
			Err(_) => return false,
		};

		// we need the origin universal location
		let bridge_locations = bridge_locations(
			Box::new(T::UniversalLocation::get()),
			bridge_origin_relative_location,
			T::BridgedNetworkId::get().into(),
			T::BridgedNetworkId::get(),
		);
		let bridge_locations = match bridge_locations {
			Ok(bridge_locations) => bridge_locations,
			Err(_) => return false,
		};

		// if at least one bridge, "owned" by origin is overloaded, we don't want to process any
		// more XCM messages from this origin. Eventually this will lead to growth of outbound
		// XCM queue at the origin chain.
		let origin_bridges = BridgesByLocalOrigin::<T, I>::get(bridge_locations.bridge_origin_universal_location);
		if origin_bridges.overloaded_bridges.is_empty() {
			return false;
		}

		// we know that the origin has opened bridges and at least one of bridges is currently
		// overloaded => pause processing of all inbound XCM messages
		true
	}
}
