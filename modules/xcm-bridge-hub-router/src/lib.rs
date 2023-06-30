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

		/// Initial `FeeFactor` value.
		#[pallet::type_value]
		pub fn InitialFeeFactor() -> FixedU128 { FixedU128::from_u32(1) }

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

	// TODO: what's the chance that this parachain will have bridges with multiple chains at the bridged
	// side? This can be handled either by converting `StorageValue` below to `StorageMap`s or by adding
	// another instances of this pallet. The latter looks more clear, but if that will be a usual case,
	// the maps option is dynamic-friendly.
	//
	// We need to use single pallet for dynamic bridges and for dynamic fee support at source parachains. 

	/// Ephemeral value that is set to true when we need to increase fee factor when sending message
	/// over the bridge.
	#[pallet::storage]
	#[pallet::whitelist_storage]
	#[pallet::getter(fn increase_fee_factor_on_send)]
	pub type IncreaseFeeFactorOnSend<T: Config<I>, I: 'static = ()> = StorageValue<_, bool, ValueQuery>;

	/// The number to multiply the base delivery fee by.
	#[pallet::storage]
	#[pallet::getter(fn fee_factor)]
	pub type FeeFactor<T: Config<I>, I: 'static = ()> = StorageValue<_, T::Balance, ValueQuery, InitialFactor>;

	/// Last known bridge queues state.
	#[pallet::storage]
	#[pallet::getter(fn bridge_queues_state)]
	pub type BridgeQueuesState<T: Config<I>, I: 'static = ()> = StorageValue<_, BridgeQueuesState, OptionQuery>;

	/// The block at which we have sent request for new bridge queues state.
	#[pallet::storage]
	#[pallet::getter(fn report_bridge_state_request_sent_at)]
	pub type ReportBrReportBridgeStateRequestSentAtidgeStateRequestSentAt<T: Config<I>, I: 'static = ()> = StorageValue<_, T::BlockNumber, OptionQuery>;
}

/// We'll be using `SovereignPaidRemoteExporter` to send remote messages over the sibling/child bridge hub.
type ViaBridgeHubExporter<T, I> = SovereignPaidRemoteExporter<
	Pallet<T, I>,
	<T as Config<I>>::ToBridgeHubSender,
	T as Config<I>>::UniversalLocation,
>;

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
		let mesage_size = message.encoded_size();
		let message_fee = (mesage_size as u128).saturating_mul(T::MessageByteFee::get());
		let fee_sum = T::MessageBaseFee::get().saturating_add(message_fee);
		let fee = fee_factor.saturating_mul_int(fee_sum);

		Some((T::LocalBridgeHubLocation::get(), (T::FeeAsset::get(), fee).into()))
	}
}

impl<T: Config<I>, I: 'static> SendXcm for Pallet<T, I> {
	type Ticket = (u32, T::ToBridgeHubSender::Ticket);

	fn validate(
		dest: &mut Option<MultiLocation>,
		xcm: &mut Option<Xcm<()>>,
	) -> SendResult<Router::Ticket> {
		// just use exporter to validate destination and insert instructions to pay message fee
		// at the sibling/child bridge hub
		let message_size = xcm.as_ref().map(|xcm| xcm.encoded_size());
		ViaBridgeHubExporter::<T, I>::validate(dest, xcm).map(|ticket| (message_size, ticket))
	}

	fn deliver(ticket: (u32, Router::Ticket)) -> Result<XcmHash, SendError> {
		// use router to enqueue message to the sibling/child bridge hub. This also should handle
		// payment for passing through this queue.
		let (message_size, ticket) = ticket;
		let xcm_hash = T::ToBridgeHubSender::deliver(ticket)?;

		// the message has been validated by us, so we know that it heads to the remote network.
		// So we may increase our fee factor here, if required.
		if IncreaseFeeFactorOnSend::<T, I>::get() {
			let message_size_factor = FixedU128::from_u32(message_size.saturating_div(1024) as u32)
				.saturating_mul(MESSAGE_SIZE_FEE_BASE);
			FeeFactor::<T, I>::mutate(|f| {
				*f = f.saturating_mul(EXPONENTIAL_FEE_BASE + message_size_factor);
			});
		}
	}
}

// TODO: what about dynamic bridges - will bridge hub have some logic (barrier) to only allow paid execution?
