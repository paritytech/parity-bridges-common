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

//! Pallet that may be used instead of `SovereignPaidRemoteExporter` in the XCM router
//! configuration. The main thing that the pallet offers is the dynamic message fee,
//! that is computed based on the bridge queues state. It starts exponentially increasing
//! if there are too many messages stuck across all bridge queues. When situation is
//! back to normal, the fee starts decreasing.

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config {
		/// Bridge limits.
		#[pallet::constant]
		type BridgeLimits: Get<BridgeLimits>;

		/// Bridge hub origin.
		type BridgeHubOrigin: EnsureOrigin<<Self as frame_system::Config>::RuntimeOrigin>;

		/// Child/sibling bridge hub configuration.
		type BridgeHub: BridgeHub;

		/// Underlying transport (XCMP or DMP) which allows sending messages to the child/sibling
		/// bridge hub. We assume this channel satisfies all XCM requirements - we rely on the
		/// fact that it guarantees ordered delivery.
		type ToBridgeHubSender: SendXcm;
	}

	#[pallet::pallet]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::hooks]
	impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I> {
		fn on_initialize(_n: BlockNumberFor<T>) -> Weight {
			RelievingBridges::<T, I>::mutate(|relieving_bridges| {
				relieving_bridges.retain(|lane_id| {
					let result = Bridges::<T, I>::try_mutate_exists(|lane_id|, |stored_bridge| {
						// remove deleted bridge from the `RelievingBridges` set
						let bridge = match stored_bridge.take() {
							Some(bridge) => bridge,
							None => return Err(false),
						};

						// remove bridge from the `RelievingBridges` set if it isn't relieving anymore
						if !bridge.is_relieving {
							return Err(false);
						}

						// decrease fee factor
						bridge.fee_factor = FixedU128::one().max(bridge.fee_factor / EXPONENTIAL_FEE_BASE);

						// stop relieveing if fee factor is `1`
						let keep_relieving = bridge.fee_factor != FixedU128::one();
						if !keep_relieving {
							bridge.is_relieving = false;
						}

						*stored_bridge = Some(bridge);
						Ok(bridge.is_relieving)
					});
				});

				// remove from the set if bridge is no longer relieving
				result == Ok(true)
			})

			// TODO: use benchmarks for that
			Weight::zero()
		}

		fn on_finalize(_n: BlockNumberFor<T>) {
			IncreaseFeeFactorOnSend::<T, I>::kill();
		}
	}

	#[pallet::call]
	impl<T: Config<I>, I: 'static> Pallet<T, I> {


		#[pallet::call_index(0)]
		#[pallet::weight(Weight::zero())]
		pub fn receive_bridge_state_report(
			origin: OriginFor<T>,
			lane_id: LaneId,
			request_sent_at: T::BlockNumber,
			at_bridge_hub_queues_state: AtBridgeHubBridgeQueuesState,
		) -> DispatchResultWithPostInfo {
			// we only accept reports from the bridge hub
			T::BridgeHubOrigin::ensure_origin(origin)?;

			Bridges::<T, I>::try_mutate_exists(lane_id, |stored_bridge| {
				// fail reports for unknown bridges
				let mut bridge = match stored_bridge.take() {
					Some(stored_bridge) => stored_bridge,
					None => fail!(Error::<T, I>::UnknownBridge),
				};

				// we only accept report for the latest request
				let last_report_request = match bridge.last_report_request.take() {
					Some(last_report_request) if last_report_request.at_block == request_sent_at => last_report_request,
					_ => fail!(Error::<T, I>::UnexpectedReport),
				};

				// update total number of enqueued messages. To compute numnber of enqueued messages in
				// `outbound_here` and `inbound_at_sibling` queues we use our own counter because we can't
				// rely on the information from the sibling/child bridge hub - it is processed with a delay
				// and during that delay any number of messages may be sent over the bridge.
 				bridge.total_enqueued_messages = last_report_request.messages_sent_after_request
					.saturating_add(at_bridge_hub_queues_state.total_enqueued_messages());


				// if we need to start decreasing fee factor - let's remember that
				let old_is_relieving = bridge.is_relieving;
				bridge.is_relieving = is_relief_required(&bridge);
				if bridge.is_relieving && !old_is_relieving {
					RelievingBridges::<T, I>::append(lane_id);
				}

				stored_bridge = Some(bridge);
				Ok(())
			});

			Ok(())
		}
	}

	/// All registered bridges.
	#[pallet::storage]
	pub type Bridges<T: Config<I>, I: 'static = ()> = StorageMap<_, Identity, LaneId, BridgeOf<T, I>, OptionQuery>;

	/// Bridges that are currently in the relieving phase.
	pub type RelievingBridges<T: Config<I>, I: 'static = ()> = StorageValue<_, Vec<LaneId>, ValueQuery>;
}

/// We'll be using `SovereignPaidRemoteExporter` to send remote messages over the sibling/child bridge hub.
type ViaBridgeHubExporter<T, I> = SovereignPaidRemoteExporter<
	Pallet<T, I>,
	<T as Config<I>>::ToBridgeHubSender,
	T as Config<I>>::UniversalLocation,
>;

// This pallet acts as the `ExporterFor` for the `SovereignPaidRemoteExporter` to compute
// message fee using fee factor.
impl<T: Config<I>, I: 'static> ExporterFor for Pallet<T, I> {
	fn exporter_for(
		network: &NetworkId,
		remote_location: &InteriorMultiLocation,
		message: &Xcm<()>,
	) -> Option<(MultiLocation, Option<MultiAsset>)> {
		// ensure that the message is sent to the expected bridged network
		if *network != T::BridgedNetworkId::get() {
			return None;
		}

		// compute fee amount
		let lane_id = bridge_lane_id(network, remote_location, T::UniversalLocation::get());
		let bridge = Bridges::<T, I>::get(lane_id)?;
		let mesage_size = message.encoded_size();
		let message_fee = (mesage_size as u128).saturating_mul(T::MessageByteFee::get());
		let fee_sum = T::MessageBaseFee::get().saturating_add(message_fee);
		let fee = bridge_fee_factor.saturating_mul_int(fee_sum);

		Some((T::LocalBridgeHubLocation::get(), (T::FeeAsset::get(), fee).into()))
	}
}

// This pallet acts as the `SendXcm` to the sibling/child bridge hub instead of regular
// XCMP/DMP transport. This allows injecting dynamic message fees into XCM programs that
// are going to the bridged network.
impl<T: Config<I>, I: 'static> SendXcm for Pallet<T, I> {
	type Ticket = (LaneId, u32, T::ToBridgeHubSender::Ticket);

	fn validate(
		dest: &mut Option<MultiLocation>,
		xcm: &mut Option<Xcm<()>>,
	) -> SendResult<Router::Ticket> {
		// just use exporter to validate destination and insert instructions to pay message fee
		// at the sibling/child bridge hub
		let lane_id = bridge_lane_id(<TODO>);
		let message_size = xcm.as_ref().map(|xcm| xcm.encoded_size());
		ViaBridgeHubExporter::<T, I>::validate(dest, xcm).map(|ticket| (lane_id, message_size, ticket))
	}

	fn deliver(ticket: (u32, Router::Ticket)) -> Result<XcmHash, SendError> {
		// use router to enqueue message to the sibling/child bridge hub. This also should handle
		// payment for passing through this queue.
		let (lane_id, message_size, ticket) = ticket;
		let xcm_hash = T::ToBridgeHubSender::deliver(ticket)?;

		// let's check if we need to increase fee for the further messages
		let limits = T::BridgeLimits::get();
		let current_block = frame_system::Pallet<T>::block_number();
		let mut bridge = Bridges::<T, I>::get(lane_id).ok_or_else(SendError::Unroutable)?;
		let limits = BridgeLimits::get();
		if is_fee_increment_required(current_block, &bridge, &limits) {
			// remove bridge from relieving set
			if bridge.is_relieving {
				RelievingBridges::<T, I>::mutate(|v| {
					v.remove(&lane_id);
				});
			}
			bridge.is_relieving = false;

			// update fee factor
			let message_size_factor = FixedU128::from_u32(message_size.saturating_div(1024) as u32)
				.saturating_mul(MESSAGE_SIZE_FEE_BASE);
			bridge.fee_factor = bridge.fee_factor.saturating_mul(EXPONENTIAL_FEE_BASE + message_size_factor);
		}

		// increment number of enqueued messages that we think are in all bridge queues now
		bridge.total_enqueued_messages.saturating_inc();

		// let's check if we need to send bridge state report request
		if is_state_report_required(current_block, &bridge, &limits) {
			unimplemented!("TODO: implement me");
		}

		// also increment number of messages that are sent **after** our last report request
		if let Some(ref mut last_report_request) = bridge.last_report_request {
			last_report_request.messages_sent_after_request.saturating_inc();
		}

		// TODO: update bridge in the storage

		Ok(xcm_hash)
	}
}

/// Returns `true` if bridge has exceeded its limits and operates with a lower than
/// expected performance. It means that we need to set higher fee for messages that
/// are sent over the bridge to avoid further throughput drops.
fn is_fee_increment_required<BlockNumber: Ord>(
	current_block: BlockNumber,
	bridge: &Bridge<BlockNumber>,
	limits: &BridgeLimits<BlockNumber>,
) -> bool {
	// if there are more messages than we expect, we need to increase fee
	if bridge.total_enqueued_messages > limits.increase_fee_factor_threshold {
		return true;
	}

	// if we are waiting for the report for too long, we need to increase fee
	let current_delay = bridge.last_report_request_block.map(|b| b.saturating_sub(current_block)).unwrap_or_else(Zero::zero);
	if current_delay > limits.maximal_bridge_state_report_delay {
		return true;
	}

	false
}

/// Returns `true` if we need new status report from the bridge hub.
fn is_state_report_required<BlockNumber: Ord>(
	current_block: BlockNumber,
	bridge: &Bridge<BlockNumber>,
	limits: &BridgeLimits<BlockNumber>,
) -> bool {
	// we never issue multiple report requests
	let last_report_request = match bridge.last_report_request {
		Some(last_report_request) => last_report_request,
		None => return false,
	};

	// we don't need new request if we believe there aren't much messages in the queue
	if bridge.total_enqueued_messages <= limits.send_report_bridge_state_threshold {
		return false;
	}

	// we don't need new request if we have received report recently
	if current_block.saturating_sub(last_report_request.sent_at) < limits.minimal_bridge_state_request_delay {
		return false
	}

	true
}

// TODO: move this function to the bp-messages or bp-runtime - there's similar function in the pallet-xcm-bridge-hub.
/// Given remote (bridged) network id and interior location within that network, return
/// lane identifier that will be used to bridge with local `local_universal_location`.
///
/// We will assume that remote location is either relay chain network, or its parachain.
/// All lower-level junctions (accounts, pallets and so on) are dropped, because they
/// will be handled by the XCM router at selected remote relay/para -chain.
pub fn bridge_lane_id(
	_bridged_network: NetworkId,
	_bridged_location: InteriorMultiLocation,
	_local_universal_location: MultiLocation,
) -> LaneId {
	unimplemented!("TODO: reuse function from the pallet-xcm-bridge-hub")
}
