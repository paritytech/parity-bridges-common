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

		/// Child/sibling bridge hub configuration.
		type BridgeHub: BridgeHub;

		/// Underlying transport (XCMP or DMP) which allows sending messages to the child/sibling
		/// bridge hub.
		type ToBridgeHubSender: SendXcm;
	}

	#[pallet::pallet]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::hooks]
	impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I> {
		fn on_initialize(_n: BlockNumberFor<T>) -> Weight {
			// TODO: since this pallet now supports dynamic bridges, we need to move this code
			// to e.g. deliver or at least schedule everything required from there

			// if we have never sent bridge state report request or have sent it more than
			// `bridge_state_report_delay` blocks ago and haven't yet received a response, we
			// need to increase fee factor
			let limits = T::BridgeLimits::get();
			let current_block = frame_system::Pallet<T>::block_number();
			let request_sent_at = Self::report_bridge_state_request_sent_at::get();
			let missing_bridge_state_report = request_sent_at
				.map(|request_sent_at| {
					let expected_report_delay = limits.bridge_state_report_delay;
					current_block.saturating_sub(request_sent_at) > expected_report_delay
				})
				.unwrap_or(false);

			// if we believe there are more enqueued messages than during regular bridge operations,
			// we need to increase fee factor
			let queues_state = Self::bridge_queues_state().unwrap_or_default();
			let total_enqueued_messages = queues_state.total_enqueued_messages();
			let too_many_enqueued_messages = total_enqueued_messages > limits.increase_fee_factor_threshold;

			// remember that we need to increase fee factor or decrease it
			if missing_bridge_state_report || too_many_enqueued_messages {
				IncreaseFeeFactorOnSend::<T, I>::set(true);
			} else {
				Self::decrease_fee_factor();
			}

			// now it's time to decide whether we want to send bridge state report request. We want
			// to send it when we believe number of enqueued messages is close to increase-fee-factor
			// threshold AND we don't have an active request
			let close_to_too_many_enqueued_messages = total_enqueued_messages > limits.send_report_bridge_state_threshold;
			if request_sent_at.is_none() && close_to_too_many_enqueued_messages {
				// TODO: even if `ReportBridgeStateRequestSentAt` is `None`, we may want to delay next
				// request to avoid being too frequent
				T::ReportBridgeStateRequestSender::send(current_block);
				ReportBridgeStateRequestSentAt::set(current_block);
			}

			// TODO: use benchmarks
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
			_origin: OriginFor<T>,
			_request_sent_at: T::BlockNumber,
			_at_bridge_hub_queues_state: AtBridgeHubBridgeQueuesState,
		) -> DispatchResultWithPostInfo {
			// TODO: check origin - it must be either parachain, or some controller account
			// TODO: convert `at_bridge_hub_queues_state` to `BridgeQueuesState`
			// TODO: kill `ReportBridgeStateRequestSentAt`
			// TODO: we shall only accept response for our last request - i.e. if something
			//       will go wrong and controller wil use forced report BUT then some old
			//       report will come, then we need to avoid it

			// we have receoved state report and may kill the `ReportBridgeStateRequestSentAt` value.
			// This means that at the next block we may send another report request and hopefully bridge
			// will
			ReportBridgeStateRequestSentAt::<T, I>::kill();
		}
	}

	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		/// Decrement fee factor, making messages cheaper. This is called by the pallet itself
		/// at the beginning of every block.
		fn decrement_fee_factor() -> FixedU128 {
			FeeFactor::<T, I>::mutate(|f| {
				*f = InitialFeeFactor::get().max(*f / EXPONENTIAL_FEE_BASE);
				*f
			})
		}
	}

	/// All registered bridges.
	#[pallet::storage]
	pub type Bridges<T: Config<I>, I: 'static = ()> = StorageMap<_, Identity, LaneId, BridgeOf<T, I>>;
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
		_: &InteriorMultiLocation,
		message: &Xcm<()>,
	) -> Option<(MultiLocation, Option<MultiAsset>)> {
		// ensure that the message is sent to the expected bridged network
		if *network != T::BridgedNetworkId::get() {
			return None;
		}

		// compute fee amount
		let bridge = Bridges::<T, I>::get();
		let mesage_size = message.encoded_size();
		let message_fee = (mesage_size as u128).saturating_mul(T::MessageByteFee::get());
		let fee_sum = T::MessageBaseFee::get().saturating_add(message_fee);
		let fee = fee_factor.saturating_mul_int(fee_sum);

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
		let lane_id = LaneId::new(UniversalLocation::get(), <TODO: universal location of the destination chain>);
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
		let state = BridgeState::<T, I>::get(lane_id);
		let limits = BridgeLimits::get();
		if is_fee_increment_required(current_block, &state, &limits) {
			let message_size_factor = FixedU128::from_u32(message_size.saturating_div(1024) as u32)
				.saturating_mul(MESSAGE_SIZE_FEE_BASE);
			FeeFactor::<T, I>::mutate(|f| {
				*f = f.saturating_mul(EXPONENTIAL_FEE_BASE + message_size_factor);
			});
		}

		// let's check if we need to send bridge state report request
		if is_state_report_required(current_block, &state, &limits) {
			ScheduledStateReportRequests::<T, I>::append(lane_id);
		}

		Ok(xcm_hash)
	}
}

/// Returns `true` if bridge has exceeded its limits and operates with a lower than
/// expected performance. It means that we need to set higher fee for messages that
/// are sent over the bridge to avoid further throughput drops.
fn is_fee_increment_required<BlockNumber: Ord>(
	current_block: BlockNumber,
	state: &BridgeState<BlockNumber>,
	limits: &BridgeLimits<BlockNumber>,
) -> bool {
	// if there are more messages than we expect, we need to increase fee
	if state.total_enqueued_messages > limits.increase_fee_factor_threshold {
		return true;
	}

	// if we are waiting for the report for too long, we need to increase fee
	let current_delay = state.last_report_request_block.map(|b| b.saturating_sub(current_block)).unwrap_or_else(Zero::zero);
	if current_delay > limits.maximal_bridge_state_report_delay {
		return true;
	}

	false
}

/// Returns `true` if we need new status report from the bridge hub.
fn is_state_report_required<BlockNumber: Ord>(
	current_block: BlockNumber,
	state: &BridgeState<BlockNumber>,
	limits: &BridgeLimits<BlockNumber>,
) -> bool {
	// we never issue multiple report requests
	if state.last_report_request_block.is_some() {
		return false;
	}

	// we don't need new request if we believe there aren't much messages in the queue
	if state.total_enqueued_messages <= limits.send_report_bridge_state_threshold {
		return false;
	}

	// we don't need new request if we have received report recently
	if current_block.saturating_sub(state.last_report_block) < limits.minimal_bridge_state_request_delay {
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
