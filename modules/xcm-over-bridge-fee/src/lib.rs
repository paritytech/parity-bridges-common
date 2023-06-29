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

//! Pallet that is able to compute over-bridge XCM message fee and automatically adjusts
//! it using bridge queues state. The pallet needs to be deployed at the sending chain,
//! sibling/parent to the bridge hub.

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

		/// A way to send report requests to the sibling/child bridge hub.
		type ReportBridgeStateRequestSender: ReportBridgeStateRequestSender;
/*		/// Expected delay (in blocks) before we get bridge state response after
		/// sending request. After this delay has passed, we consider request lost.
		/// This eventually will result in unbounded message fee raise and can
		/// be stopped only by the pallet controller.
		type BridgeStateReportDelay: Get<Self::BlockNumber>;*/
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
				// TODO: even if `ReportBridgeStateRequestSentAt` is `None`, we may want to 
				T::ReportBridgeStateRequestSender::send();
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
			_at_bridge_hub_queues_state: AtBridgeHubBridgeQueuesState,
		) -> DispatchResultWithPostInfo {
			// TODO: check origin - it must be either parachain, or some controller account
			// TODO: convert `at_bridge_hub_queues_state` to `BridgeQueuesState`
			// TODO: kill `ReportBridgeStateRequestSentAt`

			// we have receoved state report and may kill the `ReportBridgeStateRequestSentAt` value.
			// This means that at the next block we may send another report request and hopefully bridge
			// will
			ReportBridgeStateRequestSentAt::<T, I>::kill();
		}
	}

	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		/// Returns fee that needs to be paid for sending message of given size.
		///
		/// This function also increases the fee factor if there's a lof ot enqueued messages
		/// across all bridge queues.
		pub fn price_for_message_delivery(message_size: u32) -> FixedU128 {
			let fee_factor = if IncreaseFeeFactorOnSend::get() {
				let message_size_factor = FixedU128::from_u32(message_size.saturating_div(1024) as u32)
					.saturating_mul(MESSAGE_SIZE_FEE_BASE);
				FeeFactor::<T, I>::mutate(|f| {
					*f = f.saturating_mul(EXPONENTIAL_FEE_BASE + message_size_factor);
					*f
				})
			} else {
				FeeFactor::<T, I>::get()
			};

			let message_fee = (mesage_size as u128).saturating_mul(T::MessageByteFee::get());
			let fee_sum = T::MessageBaseFee::get().saturating_add(message_fee);
			let price = fee_factor.saturating_mul_int(fee_sum);

			price
		}

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
	pub type ReportBridgeStateRequestSentAt<T: Config<I>, I: 'static = ()> = StorageValue<_, T::BlockNumber, OptionQuery>;
}
