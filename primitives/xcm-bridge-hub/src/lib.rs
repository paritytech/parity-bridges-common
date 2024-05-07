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

#![warn(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]

use bp_messages::LaneId;
use bp_runtime::{AccountIdOf, BalanceOf, Chain};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	ensure, CloneNoBound, PalletError, PartialEqNoBound, RuntimeDebug, RuntimeDebugNoBound,
};
use scale_info::TypeInfo;
use sp_std::boxed::Box;
use xcm::{latest::prelude::*, VersionedMultiLocation};

/// A manager of XCM communication channels between the bridge hub and parent/sibling chains
/// that have opened bridges at this bridge hub.
///
/// We use this interface to suspend and resume channels programmatically to implement backpressure
/// mechanism for bridge queues.
#[allow(clippy::result_unit_err)] // XCM uses `Result<(), ()>` everywhere
pub trait LocalXcmChannelManager {
	// TODO: https://github.com/paritytech/parity-bridges-common/issues/2255
	// check following assumptions. They are important at least for following cases:
	// 1) we now close the associated outbound lane when misbehavior is reported. If we'll keep
	//    handling inbound XCM messages after the `suspend_inbound_channel`, they will be dropped
	// 2) the sender will be able to enqueue message to othe lanes if we won't stop handling inbound
	//    XCM immediately. He even may open additional bridges

	/// Stop handling new incoming XCM messages from given bridge `owner` (parent/sibling chain).
	///
	/// We assume that the channel will be suspended immediately, but we don't mind if inbound
	/// messages will keep piling up here for some time. Once this is communicated to the
	/// `owner` chain (in any form), we expect it to stop sending messages to us and queue
	/// messages at that `owner` chain instead.
	///
	/// We expect that:
	///
	/// - no more incoming XCM messages from the `owner` will be processed until further
	///  `resume_inbound_channel` call;
	///
	/// - soon after the call, the channel will switch to the state when incoming messages are
	///   piling up at the sending chain, not at the bridge hub.
	///
	/// This method shall not fail if the channel is already suspended.
	fn suspend_inbound_channel(owner: MultiLocation) -> Result<(), ()>;

	/// Start handling incoming messages from from given bridge `owner` (parent/sibling chain)
	/// again.
	///
	/// The channel is assumed to be suspended by the previous `suspend_inbound_channel` call,
	/// however we don't check it anywhere.
	///
	/// This method shall not fail if the channel is already resumed.
	fn resume_inbound_channel(owner: MultiLocation) -> Result<(), ()>;
}

impl LocalXcmChannelManager for () {
	fn suspend_inbound_channel(_owner: MultiLocation) -> Result<(), ()> {
		Ok(())
	}

	fn resume_inbound_channel(_owner: MultiLocation) -> Result<(), ()> {
		Err(())
	}
}

/// Bridge state.
#[derive(Clone, Copy, Decode, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub enum BridgeState {
	/// Bridge is opened. Associated lanes are also opened.
	Opened,
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
	/// Relative location of the bridge origin chain.
	pub bridge_origin_relative_location: Box<VersionedMultiLocation>,
	/// Current bridge state.
	pub state: BridgeState,
	/// Account with the reserved funds.
	pub bridge_owner_account: AccountIdOf<ThisChain>,
	/// Reserved amount on the sovereign account of the sibling bridge origin.
	pub reserve: BalanceOf<ThisChain>,
}

/// Locations of bridge endpoints at both sides of the bridge.
#[derive(Clone, RuntimeDebug, PartialEq, Eq)]
pub struct BridgeLocations {
	/// Relative (to this bridge hub) location of this side of the bridge.
	pub bridge_origin_relative_location: MultiLocation,
	/// Universal (unique) location of this side of the bridge.
	pub bridge_origin_universal_location: InteriorMultiLocation,
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
	here_universal_location: Box<InteriorMultiLocation>,
	bridge_origin_relative_location: Box<MultiLocation>,
	bridge_destination_universal_location: Box<InteriorMultiLocation>,
	expected_remote_network: NetworkId,
) -> Result<Box<BridgeLocations>, BridgeLocationsError> {
	fn strip_low_level_junctions(
		location: InteriorMultiLocation,
	) -> Result<InteriorMultiLocation, BridgeLocationsError> {
		let mut junctions = location.into_iter();

		let global_consensus = junctions
			.next()
			.filter(|junction| matches!(junction, GlobalConsensus(_)))
			.ok_or(BridgeLocationsError::NonUniversalLocation)?;

		// we only expect `Parachain` junction here. There are other junctions that
		// may need to be supported (like `GeneralKey` and `OnlyChild`), but now we
		// only support bridges with relay and parachans
		//
		// if there's something other than parachain, let's strip it
		let maybe_parachain = junctions.next().filter(|junction| matches!(junction, Parachain(_)));
		Ok(match maybe_parachain {
			Some(parachain) => X2(global_consensus, parachain),
			None => X1(global_consensus),
		})
	}

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
		.within_global(*bridge_origin_relative_location)
		.map_err(|_| BridgeLocationsError::InvalidBridgeOrigin)?;
	// strip low-level junctions within universal locations
	let bridge_origin_universal_location =
		strip_low_level_junctions(bridge_origin_universal_location)?;
	let bridge_destination_universal_location =
		strip_low_level_junctions(*bridge_destination_universal_location)?;

	// we know that the `bridge_destination_universal_location` starts from the
	// `GlobalConsensus` and we know that the `bridge_origin_universal_location`
	// is also within the `GlobalConsensus`. So we know that the lane id will be
	// the same on both ends of the bridge
	let lane_id =
		LaneId::new(bridge_origin_universal_location, bridge_destination_universal_location);

	Ok(Box::new(BridgeLocations {
		bridge_origin_relative_location: *bridge_origin_relative_location,
		bridge_origin_universal_location,
		bridge_destination_universal_location,
		lane_id,
	}))
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

		bridge_origin_universal_location: InteriorMultiLocation,
		bridge_destination_universal_location: InteriorMultiLocation,
	}

	fn run_successful_test(test: SuccessfulTest) -> BridgeLocations {
		let locations = bridge_locations(
			Box::new(test.here_universal_location),
			Box::new(test.bridge_origin_relative_location),
			Box::new(test.bridge_destination_universal_location),
			REMOTE_NETWORK,
		);
		assert_eq!(
			locations,
			Ok(Box::new(BridgeLocations {
				bridge_origin_relative_location: test.bridge_origin_relative_location,
				bridge_origin_universal_location: test.bridge_origin_universal_location,
				bridge_destination_universal_location: test.bridge_destination_universal_location,
				lane_id: LaneId::new(
					test.bridge_origin_universal_location,
					test.bridge_destination_universal_location,
				),
			})),
		);

		*locations.unwrap()
	}

	// successful tests that with various origins and destinations

	#[test]
	fn at_relay_from_local_relay_to_remote_relay_works() {
		run_successful_test(SuccessfulTest {
			here_universal_location: X1(GlobalConsensus(LOCAL_NETWORK)),
			bridge_origin_relative_location: Here.into(),

			bridge_origin_universal_location: X1(GlobalConsensus(LOCAL_NETWORK)),
			bridge_destination_universal_location: X1(GlobalConsensus(REMOTE_NETWORK)),
		});
	}

	#[test]
	fn at_relay_from_sibling_parachain_to_remote_relay_works() {
		run_successful_test(SuccessfulTest {
			here_universal_location: X1(GlobalConsensus(LOCAL_NETWORK)),
			bridge_origin_relative_location: X1(Parachain(SIBLING_PARACHAIN)).into(),

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

			bridge_origin_universal_location: X1(GlobalConsensus(LOCAL_NETWORK)),
			bridge_destination_universal_location: X1(GlobalConsensus(REMOTE_NETWORK)),
		});
		let locations2 = run_successful_test(SuccessfulTest {
			here_universal_location: X1(GlobalConsensus(LOCAL_NETWORK)),
			bridge_origin_relative_location: X1(PalletInstance(0)).into(),

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

			bridge_origin_universal_location: X1(GlobalConsensus(LOCAL_NETWORK)),
			bridge_destination_universal_location: X1(GlobalConsensus(REMOTE_NETWORK)),
		});
		let locations2 = run_successful_test(SuccessfulTest {
			here_universal_location: X1(GlobalConsensus(LOCAL_NETWORK)),
			bridge_origin_relative_location: Here.into(),

			bridge_origin_universal_location: X1(GlobalConsensus(LOCAL_NETWORK)),
			bridge_destination_universal_location: X1(GlobalConsensus(REMOTE_NETWORK)),
		});

		assert_eq!(locations1.lane_id, locations2.lane_id);
	}

	// negative tests

	#[test]
	fn bridge_locations_fails_when_here_is_not_universal_location() {
		assert_eq!(
			bridge_locations(
				Box::new(X1(Parachain(1000))),
				Box::new(Here.into()),
				Box::new(X1(GlobalConsensus(REMOTE_NETWORK))),
				REMOTE_NETWORK,
			),
			Err(BridgeLocationsError::NonUniversalLocation),
		);
	}

	#[test]
	fn bridge_locations_fails_when_computed_destination_is_not_universal_location() {
		assert_eq!(
			bridge_locations(
				Box::new(X1(GlobalConsensus(LOCAL_NETWORK))),
				Box::new(Here.into()),
				Box::new(X1(OnlyChild)),
				REMOTE_NETWORK,
			),
			Err(BridgeLocationsError::NonUniversalLocation),
		);
	}

	#[test]
	fn bridge_locations_fails_when_computed_destination_is_local() {
		assert_eq!(
			bridge_locations(
				Box::new(X1(GlobalConsensus(LOCAL_NETWORK))),
				Box::new(Here.into()),
				Box::new(X2(GlobalConsensus(LOCAL_NETWORK), OnlyChild)),
				REMOTE_NETWORK,
			),
			Err(BridgeLocationsError::DestinationIsLocal),
		);
	}

	#[test]
	fn bridge_locations_fails_when_computed_destination_is_unreachable() {
		assert_eq!(
			bridge_locations(
				Box::new(X1(GlobalConsensus(LOCAL_NETWORK))),
				Box::new(Here.into()),
				Box::new(X1(GlobalConsensus(UNREACHABLE_NETWORK))),
				REMOTE_NETWORK,
			),
			Err(BridgeLocationsError::UnreachableDestination),
		);
	}
}
