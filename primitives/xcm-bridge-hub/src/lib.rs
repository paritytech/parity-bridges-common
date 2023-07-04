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

//! Primitives of the xcm-bridge-hub pallet.

#![cfg_attr(not(feature = "std"), no_std)]

use bp_messages::{LaneId, MessageNonce};
use bp_runtime::{AccountIdOf, BalanceOf, BlockNumberOf, Chain};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	ensure, CloneNoBound, PalletError, PartialEqNoBound, RuntimeDebug, RuntimeDebugNoBound,
};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use sp_std::convert::TryInto;
use xcm::latest::prelude::*;

/// Bridge state.
#[derive(Clone, Copy, Decode, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub enum BridgeState<BlockNumber> {
	/// Bridge is opened. Associated lanes are also opened.
	Opened,
	/// Bridge is closing. It will switch to closed state at given block.
	/// Outbound lane is either closed (if bridged is closing because of misbehavior), or it
	/// is closing. Inbound lane is in closing state.
	Closing(BlockNumber),
	/// Bridge is closed. Associated lanes are also closed.
	/// After all outbound messages will be pruned, the bridge will vanish without any traces.
	Closed,
}

/// Bridge metadata.
#[derive(
	CloneNoBound, Decode, Encode, Eq, PartialEqNoBound, TypeInfo, MaxEncodedLen, RuntimeDebugNoBound,
)]
#[scale_info(skip_type_params(ThisChain))]
pub struct Bridge<ThisChain: Chain> {
	/// Current bridge state.
	pub state: BridgeState<BlockNumberOf<ThisChain>>,
	/// Account with the reserved funds.
	pub bridge_owner_account: AccountIdOf<ThisChain>,
	/// Reserved amount on the sovereign account of the sibling bridge origin.
	pub reserve: BalanceOf<ThisChain>,
}

/// Bridge limits. Bridges that exceed those limits may be reported, fined and closed.
#[derive(Clone, Copy, Decode, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub struct BridgeLimits {
	/// Maximal number of outbound messages that may be queued at the outbound lane at a time.
	/// Normally a bridge maintainers must run at least one relayer that will deliver messages
	/// to the bridged chain and confirm delivery. If there's no relayer running, messages will
	/// keep piling up, which will lead to trie growth, which we don't want.
	///
	/// This limit must be selected with care - it should account possible delays because of
	/// runtime upgrades, spamming queues, finality lags and so on.
	pub max_queued_outbound_messages: MessageNonce,
	// TODO: limit to detect relayer activity - i.e. if there are 10 queued messages, but they
	// are not delivered for 14 days => misbehavior

	// TODO: too low funds on the relayers-fund account?
}

/// Bridge misbehavior.
#[derive(
	Clone,
	Copy,
	Decode,
	Encode,
	Eq,
	PartialEq,
	TypeInfo,
	MaxEncodedLen,
	RuntimeDebug,
	Serialize,
	Deserialize,
)]
pub enum BridgeMisbehavior {
	/// The number of messages in the outbound queue is larger than the limit.
	TooManyQueuedOutboundMessages,
}

/// The state of all (reachable) queues as they seen from the bridge hub.
#[derive(
	Clone,
	Copy,
	Decode,
	Default,
	Encode,
	Eq,
	Ord,
	PartialOrd,
	PartialEq,
	RuntimeDebug,
	TypeInfo,
	MaxEncodedLen,
	Serialize,
	Deserialize,
)]
pub struct BridgeQueuesState {
	/// Number of messages queued at bridge hub outbound (`pallet-bridge-messages`) queue.
	pub outbound_here: MessageNonce,
	/// Number of messages queued at the outbound queue of the bridged bridge hub. This
	/// queue connects bridged bridge hub and remote bridge destination. In most cases
	/// it will be the XCMP (HRMP) or DMP queue.
	pub outbound_at_bridged: MessageNonce,
	/// Number of messages queued at the destination inbound queue. This queue connects
	/// bridged bridge hub and remote bridge destination. In most cases it will be the XCMP
	/// (HRMP) or DMP queue.
	///
	/// Bridged (target) bridge hub doesn't have an access to the exact value of
	/// this metric. But it may get an estimation, depending on the channel
	/// state. The channel between target brige hub and desination is suspended
	/// when there are more than `N` unprocessed messages at the destination inbound
	/// queue. So if we see the suspended channel state at the target bridge hub,
	/// we: (1) assume that there's at least `N` queued messages at the inbound
	/// destination queue and (2) all further messages are now piling up at our
	/// outbound queue (`outbound_at_bridged`), so we have exact count.
	pub inbound_at_destination: MessageNonce,
}

impl BridgeQueuesState {
	/// Return total number of messsages that we assume are currently in the bridges queue.
	pub fn total_enqueued_messages(&self) -> MessageNonce {
		self.outbound_here
			.saturating_add(self.outbound_at_bridged)
			.saturating_add(self.inbound_at_destination)
	}
}

/// Locations of bridge endpoints at both sides of the bridge.
#[derive(Clone, RuntimeDebug, PartialEq, Eq)]
pub struct BridgeLocations {
	/// Relative (to this bridge hub) location of this side of the bridge.
	pub bridge_origin_relative_location: MultiLocation,
	/// Universal (unique) location of this side of the bridge.
	pub bridge_origin_universal_location: InteriorMultiLocation,
	/// Relative (to this bridge hub) location of the other side of the bridge.
	pub bridge_destination_relative_location: MultiLocation,
	/// Universal (unique) location of other side of the bridge.
	pub bridge_destination_universal_location: InteriorMultiLocation,
	/// An identifier of the dedicated bridge message lane.
	pub lane_id: LaneId,
}

/// Errors that may happen when we check bridge locations.
#[derive(Encode, Decode, RuntimeDebug, PartialEq, Eq, PalletError, TypeInfo)]
pub enum BridgeLocationsError {
	/// Origin or destination locations are not universal.
	NonUniversalLocation,
	/// Bridge origin location is not supported.
	InvalidBridgeOrigin,
	/// Bridge destination is not supported (in general).
	InvalidBridgeDestination,
	/// Destination location is within the same global consensus.
	DestinationIsLocal,
	/// Destination network is not the network we are bridged with.
	UnreachableDestination,
	/// Destination location is unsupported. We only support bridges with relay
	/// chain or its parachains.
	UnsupportedDestinationLocation,
}

/// Given XCM locations, generate lane id and universal locations of bridge endpoints.
///
/// The `here_universal_location` is the universal location of the bridge hub runtime.
///
/// The `bridge_origin_relative_location` is the relative (to the `here_universal_location`)
/// location of the bridge endpoint at this side of the bridge. It may be the parent relay
/// chain or the sibling parachain. All junctions below parachain level are dropped.
///
/// The `bridge_destination_universal_location` is the universal location of the bridge
/// destination. It may be the parent relay or the sibling parachain of the **bridged**
/// bridge hub. All junctions below parachain level are dropped.
///
/// Why we drop all junctions between parachain level - that's because the lane is a bridge
/// between two chains. All routing under this level happens when the message is delivered
/// to the bridge destination. So at bridge level we don't care about low level junctions.
///
/// Returns error if `bridge_origin_relative_location` is outside of `here_universal_location`
/// local consensus OR if `bridge_destination_universal_location` is not a universal location.
pub fn bridge_locations(
	here_universal_location: InteriorMultiLocation,
	bridge_origin_relative_location: MultiLocation,
	bridge_destination_relative_location: MultiLocation,
	expected_remote_network: NetworkId,
) -> Result<BridgeLocations, BridgeLocationsError> {
	fn strip_low_level_junctions(
		location: InteriorMultiLocation,
	) -> Result<InteriorMultiLocation, BridgeLocationsError> {
		let mut junctions = location.into_iter();

		// we know that the first junction of the location is `GlobalConsensus`, so we don't check
		// it (this also shall never fail, but let's be extra cautious)
		let global_consensus =
			junctions.next().ok_or(BridgeLocationsError::NonUniversalLocation)?;

		// deal with the next junction
		if let Some(next) = junctions.next() {
			// we only expect `Parachain` junction here. There are other junctions that
			// may need to be supported (like `GeneralKey` and `OnlyChild`), but now we
			// only support bridges with relay and parachans
			//
			// if there's something other than parachain, let's strip it
			if !matches!(next, Junction::Parachain(_)) {
				Ok(X1(global_consensus))
			} else {
				// skip everything below this level
				Ok(X2(global_consensus, next))
			}
		} else {
			// if there's just one item in the location interior, no modifications required
			Ok(X1(global_consensus))
		}
	}

	// get bridge destination universal location
	let bridge_destination_universal_location: InteriorMultiLocation = here_universal_location
		.into_location()
		.appended_with(bridge_destination_relative_location)
		.map_err(|_| BridgeLocationsError::InvalidBridgeDestination)?
		.try_into()
		.map_err(|_| BridgeLocationsError::InvalidBridgeDestination)?;

	// ensure that the `here_universal_location` and `bridge_destination_universal_location`
	// are universal locations within different consensus systems
	let local_network = here_universal_location
		.global_consensus()
		.map_err(|_| BridgeLocationsError::NonUniversalLocation)?;
	let remote_network = bridge_destination_universal_location
		.global_consensus()
		.map_err(|_| BridgeLocationsError::NonUniversalLocation)?;
	ensure!(local_network != remote_network, BridgeLocationsError::DestinationIsLocal);
	ensure!(
		remote_network == expected_remote_network,
		BridgeLocationsError::UnreachableDestination
	);

	// get universal location of endpoint, located at this side of the bridge
	let bridge_origin_universal_location = here_universal_location
		.within_global(bridge_origin_relative_location)
		.map_err(|_| BridgeLocationsError::InvalidBridgeOrigin)?;
	// strip low-level junctions within universal locations
	let bridge_origin_universal_location =
		strip_low_level_junctions(bridge_origin_universal_location)?;
	let bridge_destination_universal_location =
		strip_low_level_junctions(bridge_destination_universal_location)?;

	// we know that the `bridge_destination_universal_location` starts from the
	// `GlobalConsensus` and we know that the `bridge_origin_universal_location`
	// is also within the `GlobalConsensus`. So we know that the lane id will be
	// the same on both ends of the bridge
	let lane_id =
		LaneId::new(bridge_origin_universal_location, bridge_destination_universal_location);

	Ok(BridgeLocations {
		bridge_origin_relative_location,
		bridge_origin_universal_location,
		bridge_destination_relative_location,
		bridge_destination_universal_location,
		lane_id,
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	const LOCAL_NETWORK: NetworkId = Kusama;
	const REMOTE_NETWORK: NetworkId = Polkadot;
	const UNREACHABLE_NETWORK: NetworkId = Rococo;
	const SIBLING_PARACHAIN: u32 = 1000;
	const LOCAL_BRIDGE_HUB: u32 = 1001;
	const REMOTE_PARACHAIN: u32 = 2000;

	struct SuccessfulTest {
		here_universal_location: InteriorMultiLocation,
		bridge_origin_relative_location: MultiLocation,
		bridge_destination_relative_location: MultiLocation,

		bridge_origin_universal_location: InteriorMultiLocation,
		bridge_destination_universal_location: InteriorMultiLocation,
	}

	fn run_successful_test(test: SuccessfulTest) -> BridgeLocations {
		let locations = bridge_locations(
			test.here_universal_location,
			test.bridge_origin_relative_location,
			test.bridge_destination_relative_location,
			REMOTE_NETWORK,
		);
		assert_eq!(
			locations,
			Ok(BridgeLocations {
				bridge_origin_relative_location: test.bridge_origin_relative_location,
				bridge_origin_universal_location: test.bridge_origin_universal_location,
				bridge_destination_relative_location: test.bridge_destination_relative_location,
				bridge_destination_universal_location: test.bridge_destination_universal_location,
				lane_id: LaneId::new(
					test.bridge_origin_universal_location,
					test.bridge_destination_universal_location,
				),
			}),
		);

		locations.unwrap()
	}

	// successful tests that with various origins and destinations

	#[test]
	fn at_relay_from_local_relay_to_remote_relay_works() {
		run_successful_test(SuccessfulTest {
			here_universal_location: X1(GlobalConsensus(LOCAL_NETWORK)),
			bridge_origin_relative_location: Here.into(),
			bridge_destination_relative_location: ParentThen(X1(GlobalConsensus(REMOTE_NETWORK)))
				.into(),

			bridge_origin_universal_location: X1(GlobalConsensus(LOCAL_NETWORK)),
			bridge_destination_universal_location: X1(GlobalConsensus(REMOTE_NETWORK)),
		});
	}

	#[test]
	fn at_relay_from_sibling_parachain_to_remote_relay_works() {
		run_successful_test(SuccessfulTest {
			here_universal_location: X1(GlobalConsensus(LOCAL_NETWORK)),
			bridge_origin_relative_location: X1(Parachain(SIBLING_PARACHAIN)).into(),
			bridge_destination_relative_location: ParentThen(X1(GlobalConsensus(REMOTE_NETWORK)))
				.into(),

			bridge_origin_universal_location: X2(
				GlobalConsensus(LOCAL_NETWORK),
				Parachain(SIBLING_PARACHAIN),
			),
			bridge_destination_universal_location: X1(GlobalConsensus(REMOTE_NETWORK)),
		});
	}

	#[test]
	fn at_relay_from_local_relay_to_remote_parachain_works() {
		run_successful_test(SuccessfulTest {
			here_universal_location: X1(GlobalConsensus(LOCAL_NETWORK)),
			bridge_origin_relative_location: Here.into(),
			bridge_destination_relative_location: ParentThen(X2(
				GlobalConsensus(REMOTE_NETWORK),
				Parachain(REMOTE_PARACHAIN),
			))
			.into(),

			bridge_origin_universal_location: X1(GlobalConsensus(LOCAL_NETWORK)),
			bridge_destination_universal_location: X2(
				GlobalConsensus(REMOTE_NETWORK),
				Parachain(REMOTE_PARACHAIN),
			),
		});
	}

	#[test]
	fn at_relay_from_sibling_parachain_to_remote_parachain_works() {
		run_successful_test(SuccessfulTest {
			here_universal_location: X1(GlobalConsensus(LOCAL_NETWORK)),
			bridge_origin_relative_location: X1(Parachain(SIBLING_PARACHAIN)).into(),
			bridge_destination_relative_location: ParentThen(X2(
				GlobalConsensus(REMOTE_NETWORK),
				Parachain(REMOTE_PARACHAIN),
			))
			.into(),

			bridge_origin_universal_location: X2(
				GlobalConsensus(LOCAL_NETWORK),
				Parachain(SIBLING_PARACHAIN),
			),
			bridge_destination_universal_location: X2(
				GlobalConsensus(REMOTE_NETWORK),
				Parachain(REMOTE_PARACHAIN),
			),
		});
	}

	#[test]
	fn at_bridge_hub_from_local_relay_to_remote_relay_works() {
		run_successful_test(SuccessfulTest {
			here_universal_location: X2(
				GlobalConsensus(LOCAL_NETWORK),
				Parachain(LOCAL_BRIDGE_HUB),
			),
			bridge_origin_relative_location: Parent.into(),
			bridge_destination_relative_location: AncestorThen(
				2,
				X1(GlobalConsensus(REMOTE_NETWORK)),
			)
			.into(),

			bridge_origin_universal_location: X1(GlobalConsensus(LOCAL_NETWORK)),
			bridge_destination_universal_location: X1(GlobalConsensus(REMOTE_NETWORK)),
		});
	}

	#[test]
	fn at_bridge_hub_from_sibling_parachain_to_remote_relay_works() {
		run_successful_test(SuccessfulTest {
			here_universal_location: X2(
				GlobalConsensus(LOCAL_NETWORK),
				Parachain(LOCAL_BRIDGE_HUB),
			),
			bridge_origin_relative_location: ParentThen(X1(Parachain(SIBLING_PARACHAIN))).into(),
			bridge_destination_relative_location: AncestorThen(
				2,
				X1(GlobalConsensus(REMOTE_NETWORK)),
			)
			.into(),

			bridge_origin_universal_location: X2(
				GlobalConsensus(LOCAL_NETWORK),
				Parachain(SIBLING_PARACHAIN),
			),
			bridge_destination_universal_location: X1(GlobalConsensus(REMOTE_NETWORK)),
		});
	}

	#[test]
	fn at_bridge_hub_from_local_relay_to_remote_parachain_works() {
		run_successful_test(SuccessfulTest {
			here_universal_location: X2(
				GlobalConsensus(LOCAL_NETWORK),
				Parachain(LOCAL_BRIDGE_HUB),
			),
			bridge_origin_relative_location: Parent.into(),
			bridge_destination_relative_location: AncestorThen(
				2,
				X2(GlobalConsensus(REMOTE_NETWORK), Parachain(REMOTE_PARACHAIN)),
			)
			.into(),

			bridge_origin_universal_location: X1(GlobalConsensus(LOCAL_NETWORK)),
			bridge_destination_universal_location: X2(
				GlobalConsensus(REMOTE_NETWORK),
				Parachain(REMOTE_PARACHAIN),
			),
		});
	}

	#[test]
	fn at_bridge_hub_from_sibling_parachain_to_remote_parachain_works() {
		run_successful_test(SuccessfulTest {
			here_universal_location: X2(
				GlobalConsensus(LOCAL_NETWORK),
				Parachain(LOCAL_BRIDGE_HUB),
			),
			bridge_origin_relative_location: ParentThen(X1(Parachain(SIBLING_PARACHAIN))).into(),
			bridge_destination_relative_location: AncestorThen(
				2,
				X2(GlobalConsensus(REMOTE_NETWORK), Parachain(REMOTE_PARACHAIN)),
			)
			.into(),

			bridge_origin_universal_location: X2(
				GlobalConsensus(LOCAL_NETWORK),
				Parachain(SIBLING_PARACHAIN),
			),
			bridge_destination_universal_location: X2(
				GlobalConsensus(REMOTE_NETWORK),
				Parachain(REMOTE_PARACHAIN),
			),
		});
	}

	// successful tests that show that we are ignoring low-level junctions of bridge origins

	#[test]
	fn low_level_junctions_at_bridge_origin_are_stripped() {
		let locations1 = run_successful_test(SuccessfulTest {
			here_universal_location: X1(GlobalConsensus(LOCAL_NETWORK)),
			bridge_origin_relative_location: Here.into(),
			bridge_destination_relative_location: ParentThen(X1(GlobalConsensus(REMOTE_NETWORK)))
				.into(),

			bridge_origin_universal_location: X1(GlobalConsensus(LOCAL_NETWORK)),
			bridge_destination_universal_location: X1(GlobalConsensus(REMOTE_NETWORK)),
		});
		let locations2 = run_successful_test(SuccessfulTest {
			here_universal_location: X1(GlobalConsensus(LOCAL_NETWORK)),
			bridge_origin_relative_location: X1(PalletInstance(0)).into(),
			bridge_destination_relative_location: ParentThen(X1(GlobalConsensus(REMOTE_NETWORK)))
				.into(),

			bridge_origin_universal_location: X1(GlobalConsensus(LOCAL_NETWORK)),
			bridge_destination_universal_location: X1(GlobalConsensus(REMOTE_NETWORK)),
		});

		assert_eq!(locations1.lane_id, locations2.lane_id);
	}

	#[test]
	fn low_level_junctions_at_bridge_destination_are_stripped() {
		let locations1 = run_successful_test(SuccessfulTest {
			here_universal_location: X1(GlobalConsensus(LOCAL_NETWORK)),
			bridge_origin_relative_location: Here.into(),
			bridge_destination_relative_location: ParentThen(X1(GlobalConsensus(REMOTE_NETWORK)))
				.into(),

			bridge_origin_universal_location: X1(GlobalConsensus(LOCAL_NETWORK)),
			bridge_destination_universal_location: X1(GlobalConsensus(REMOTE_NETWORK)),
		});
		let locations2 = run_successful_test(SuccessfulTest {
			here_universal_location: X1(GlobalConsensus(LOCAL_NETWORK)),
			bridge_origin_relative_location: Here.into(),
			bridge_destination_relative_location: ParentThen(X2(
				GlobalConsensus(REMOTE_NETWORK),
				PalletInstance(0),
			))
			.into(),

			bridge_origin_universal_location: X1(GlobalConsensus(LOCAL_NETWORK)),
			bridge_destination_universal_location: X1(GlobalConsensus(REMOTE_NETWORK)),
		});

		assert_eq!(locations1.lane_id, locations2.lane_id);
	}

	// negative tests

	#[test]
	fn bridge_locations_fails_when_we_fail_to_compute_destination_universal_location() {
		assert_eq!(
			bridge_locations(
				X6(
					GlobalConsensus(LOCAL_NETWORK),
					Parachain(1000),
					OnlyChild,
					OnlyChild,
					OnlyChild,
					OnlyChild
				),
				Here.into(),
				ParentThen(X4(GlobalConsensus(REMOTE_NETWORK), OnlyChild, OnlyChild, OnlyChild))
					.into(),
				REMOTE_NETWORK,
			),
			Err(BridgeLocationsError::InvalidBridgeDestination),
		);
	}

	#[test]
	fn bridge_locations_fails_when_here_is_not_universal_location() {
		assert_eq!(
			bridge_locations(
				X1(Parachain(1000)),
				Here.into(),
				ParentThen(X1(GlobalConsensus(REMOTE_NETWORK))).into(),
				REMOTE_NETWORK,
			),
			Err(BridgeLocationsError::NonUniversalLocation),
		);
	}

	#[test]
	fn bridge_locations_fails_when_computed_destination_is_not_universal_location() {
		assert_eq!(
			bridge_locations(
				X1(GlobalConsensus(LOCAL_NETWORK)),
				Here.into(),
				ParentThen(X1(OnlyChild)).into(),
				REMOTE_NETWORK,
			),
			Err(BridgeLocationsError::NonUniversalLocation),
		);
	}

	#[test]
	fn bridge_locations_fails_when_computed_destination_is_local() {
		assert_eq!(
			bridge_locations(
				X1(GlobalConsensus(LOCAL_NETWORK)),
				Here.into(),
				X1(OnlyChild).into(),
				REMOTE_NETWORK,
			),
			Err(BridgeLocationsError::DestinationIsLocal),
		);
	}

	#[test]
	fn bridge_locations_fails_when_computed_destination_is_unreachable() {
		assert_eq!(
			bridge_locations(
				X1(GlobalConsensus(LOCAL_NETWORK)),
				Here.into(),
				ParentThen(X1(GlobalConsensus(UNREACHABLE_NETWORK))).into(),
				REMOTE_NETWORK,
			),
			Err(BridgeLocationsError::UnreachableDestination),
		);
	}
}
