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

//! The code that allows to use the pallet (`pallet-xcm-bridge-hub`) as XCM message
//! exporter at the sending bridge hub. Internally, it just enqueues outbound blob
//! in the messages pallet queue.
//!
//! This code is executed at the source bridge hub.

use crate::{Config, Pallet, SuspendedBridges, LOG_TARGET};

use bp_messages::{
	source_chain::{MessagesBridge, OnMessagesDelivered},
	LaneId, MessageNonce,
};
use bp_xcm_bridge_hub::{BridgeId, BridgeLocations, LocalXcmChannelManager, XcmAsPlainPayload};
use frame_support::traits::Get;
use pallet_bridge_messages::{Config as BridgeMessagesConfig, Pallet as BridgeMessagesPallet};
use sp_std::boxed::Box;
use xcm::prelude::*;
use xcm_builder::{HaulBlob, HaulBlobError, HaulBlobExporter};
use xcm_executor::traits::ExportXcm;

/// Maximal number of messages in the outbound bridge queue. Once we reach this limit, we
/// suspend a bridge.
const OUTBOUND_LANE_CONGESTED_THRESHOLD: MessageNonce = 8_192;

/// After we have suspended the bridge, we wait until number of messages in the outbound bridge
/// queue drops to this count, before sending resuming the bridge.
const OUTBOUND_LANE_UNCONGESTED_THRESHOLD: MessageNonce = 1_024;

// An easy way to access `HaulBlobExporter`.
type PalletAsHaulBlobExporter<T, I> = HaulBlobExporter<
	DummyHaulBlob,
	<T as Config<I>>::BridgedNetworkId,
	<T as Config<I>>::MessageExportPrice,
>;
/// An easy way to access associated messages pallet.
type MessagesPallet<T, I> = BridgeMessagesPallet<T, <T as Config<I>>::BridgeMessagesPalletInstance>;

impl<T: Config<I>, I: 'static> ExportXcm for Pallet<T, I>
where
	T: BridgeMessagesConfig<
		<T as Config<I>>::BridgeMessagesPalletInstance,
		OutboundPayload = XcmAsPlainPayload,
	>,
{
	type Ticket = (Box<BridgeLocations>, XcmAsPlainPayload, XcmHash);

	fn validate(
		network: NetworkId,
		channel: u32,
		universal_source: &mut Option<InteriorMultiLocation>,
		destination: &mut Option<InteriorMultiLocation>,
		message: &mut Option<Xcm<()>>,
	) -> Result<(Self::Ticket, MultiAssets), SendError> {
		// `HaulBlobExporter` may consume the `universal_source` and `destination` arguments, so
		// let's save them before
		let bridge_origin_universal_location =
			universal_source.clone().take().ok_or(SendError::MissingArgument)?;
		let bridge_destination_interior_location =
			destination.clone().take().ok_or(SendError::MissingArgument)?;

		// check if we are able to route the message. We use existing `HaulBlobExporter` for that.
		// It will make all required changes and will encode message properly, so that the
		// `DispatchBlob` at the bridged bridge hub will be able to decode it
		let ((blob, id), price) = PalletAsHaulBlobExporter::<T, I>::validate(
			network,
			channel,
			universal_source,
			destination,
			message,
		)?;

		// ok - now we know that the message may be routed by the pallet, let's prepare the
		// destination universal location
		let mut bridge_destination_universal_location =
			X1(GlobalConsensus(T::BridgedNetworkId::get()));
		bridge_destination_universal_location
			.append_with(bridge_destination_interior_location)
			.map_err(|_| SendError::Unroutable)?;

		// .. and the origin relative location
		let bridge_origin_relative_location =
			bridge_origin_universal_location.relative_to(&T::UniversalLocation::get());

		// then we are able to compute the lane id used to send messages
		let locations = Self::bridge_locations(
			Box::new(bridge_origin_relative_location),
			Box::new(bridge_destination_universal_location.into()),
		)
		.map_err(|_| SendError::Unroutable)?;

		Ok(((locations, blob, id), price))
	}

	fn deliver(
		(locations, blob, id): (Box<BridgeLocations>, XcmAsPlainPayload, XcmHash),
	) -> Result<XcmHash, SendError> {
		let send_result = MessagesPallet::<T, I>::send_message(locations.bridge_id.lane_id(), blob);

		match send_result {
			Ok(artifacts) => {
				log::info!(
					target: LOG_TARGET,
					"XCM message {:?} has been enqueued at bridge {:?} with nonce {}",
					id,
					locations.bridge_id,
					artifacts.nonce,
				);

				// maybe we need switch to congested state
				Self::on_bridge_message_enqueued(locations, artifacts.enqueued_messages);
			},
			Err(error) => {
				log::debug!(
					target: LOG_TARGET,
					"XCM message {:?} has been dropped because of bridge error {:?} on bridge {:?}",
					id,
					error,
					locations.bridge_id,
				);
				return Err(SendError::Transport("BridgeSendError"))
			},
		}

		Ok(id)
	}
}

impl<T: Config<I>, I: 'static> OnMessagesDelivered for Pallet<T, I> {
	fn on_messages_delivered(lane_id: LaneId, enqueued_messages: MessageNonce) {
		Self::on_bridge_messages_delivered(lane_id, enqueued_messages);
	}
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
	/// Called when new message is pushed onto outbound bridge queue.
	fn on_bridge_message_enqueued(
		locations: Box<BridgeLocations>,
		enqueued_messages: MessageNonce,
	) {
		// if the bridge queue is not congested, we don't want to do anything
		let is_congested = enqueued_messages > OUTBOUND_LANE_CONGESTED_THRESHOLD;
		if !is_congested {
			return
		}

		// TODO: https://github.com/paritytech/parity-bridges-common/issues/2006 we either need fishermens
		// to watch thsi rule violation (suspended, but keep sending new messages), or we need a
		// hard limit for that like other XCM queues have

		// check if the lane is already suspended. If it is, do nothing. We still accept new
		// messages to the suspended bridge, hoping that it'll be actually suspended soon
		let is_already_suspended = SuspendedBridges::<T, I>::get().contains(&locations.bridge_id);
		if is_already_suspended {
			return
		}

		// else - suspend the bridge
		let suspend_result = T::LocalXcmChannelManager::suspend_bridge(
			&locations.bridge_origin_relative_location,
			locations.bridge_id,
		);
		match suspend_result {
			Ok(_) => {
				log::debug!(
					target: LOG_TARGET,
					"Suspended the bridge {:?}, originated by the {:?}",
					locations.bridge_id,
					locations.bridge_origin_relative_location,
				);
			},
			Err(e) => {
				log::debug!(
					target: LOG_TARGET,
					"Failed to suspended the bridge {:?}, originated by the {:?}: {:?}",
					locations.bridge_id,
					locations.bridge_origin_relative_location,
					e,
				);

				return
			},
		}

		// and remember that we have suspended the bridge
		SuspendedBridges::<T, I>::mutate(|suspended_bridges| {
			let maybe_error = suspended_bridges.try_push(locations.bridge_id);
			if let Err(e) = maybe_error {
				// TODO: https://github.com/paritytech/parity-bridges-common/issues/2006
				// we've sent the suspend signal, but failed to remember that => we'll keep
				// up sending the signal on every further message, effectively blocking the XCM
				// lane. We need some limit on total number of bridges so that this call won't ever
				// fail.

				log::debug!(
					target: LOG_TARGET,
					"Failed to remember the suspended bridge {:?}, originated by the {:?}: {:?}",
					locations.bridge_id,
					locations.bridge_origin_relative_location,
					e,
				);
			}
		});
	}

	/// Must be called whenever we receive a message delivery confirmation.
	fn on_bridge_messages_delivered(lane_id: LaneId, enqueued_messages: MessageNonce) {
		// if the bridge queue is still congested, we don't want to do anything
		let is_congested = enqueued_messages > OUTBOUND_LANE_UNCONGESTED_THRESHOLD;
		if is_congested {
			return
		}

		// if we have not suspended the bridge before, we don't want to do anything
		let bridge_id = BridgeId::from_lane_id(lane_id);
		if !SuspendedBridges::<T, I>::get().contains(&bridge_id) {
			return
		}

		// else - resume the bridge
		if let Some(bridge) = Self::bridge(bridge_id) {
			let bridge_origin_relative_location =
				(*bridge.bridge_origin_relative_location).try_into();
			let bridge_origin_relative_location = match bridge_origin_relative_location {
				Ok(bridge_origin_relative_location) => bridge_origin_relative_location,
				Err(e) => {
					log::debug!(
						target: LOG_TARGET,
						"Failed to convert the bridge  {:?} location: {:?}",
						lane_id,
						e,
					);

					return
				},
			};

			let resume_result = T::LocalXcmChannelManager::resume_bridge(
				&bridge_origin_relative_location,
				bridge_id,
			);
			match resume_result {
				Ok(_) => {
					log::debug!(
						target: LOG_TARGET,
						"Resumed the bridge {:?}, originated by the {:?}",
						lane_id,
						bridge_origin_relative_location,
					);
				},
				Err(e) => {
					log::debug!(
						target: LOG_TARGET,
						"Failed to resume the bridge {:?}, originated by the {:?}: {:?}",
						lane_id,
						bridge_origin_relative_location,
						e,
					);

					return
				},
			}
		}

		// and forget that we have previously suspended the bridge
		SuspendedBridges::<T, I>::mutate(|suspended_bridges| {
			suspended_bridges.retain(|b| *b != bridge_id);
		});
	}
}

/// Dummy implementation of the `HaulBlob` trait that is never called.
///
/// We are using `HaulBlobExporter`, which requires `HaulBlob` implementation. It assumes that
/// there's a single channel between two bridge hubs - `HaulBlob` only accepts the blob and nothing
/// else. But bridge messages pallet may have a dedicated channel (lane) for every pair of bridged
/// chains. So we are using our own `ExportXcm` implementation, but to utilize `HaulBlobExporter` we
/// still need this `DummyHaulBlob`.
struct DummyHaulBlob;

impl HaulBlob for DummyHaulBlob {
	fn haul_blob(_blob: XcmAsPlainPayload) -> Result<(), HaulBlobError> {
		Err(HaulBlobError::Transport("DummyHaulBlob"))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{mock::*, LanesManagerOf};

	use xcm_executor::traits::export_xcm;

	#[test]
	fn proper_lane_is_used_by_export_xcm() {
		run_test(|| {
			// open expected outbound lane
			let origin = OpenBridgeOrigin::sibling_parachain_origin();
			let with = bridged_asset_hub_location();
			let locations =
				XcmOverBridge::bridge_locations_from_origin(origin, Box::new(with.into())).unwrap();

			let lanes_manager = LanesManagerOf::<TestRuntime, ()>::new();
			lanes_manager.create_outbound_lane(locations.bridge_id.lane_id()).unwrap();
			assert!(lanes_manager
				.active_outbound_lane(locations.bridge_id.lane_id())
				.unwrap()
				.queued_messages()
				.is_empty());

			// now let's try to enqueue message using our `ExportXcm` implementation
			export_xcm::<XcmOverBridge>(
				BridgedRelayNetwork::get(),
				0,
				locations.bridge_origin_universal_location,
				locations.bridge_destination_universal_location.split_first().0,
				vec![Instruction::ClearOrigin].into(),
			)
			.unwrap();

			// double check that the message has been pushed to the expected lane
			// (it should already been checked during `send_message` call)
			assert!(!lanes_manager
				.active_outbound_lane(locations.bridge_id.lane_id())
				.unwrap()
				.queued_messages()
				.is_empty());
		});
	}
}
