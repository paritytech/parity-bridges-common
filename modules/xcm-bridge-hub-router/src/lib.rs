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
//! if the queue between this chain and the sibling/child bridge hub is congested.
//!
//! All other bridge hub queues offer some backpressure mechanisms. So if at least one
//! of all queues is congested, it will eventually lead to the growth of the queue at
//! this chain.
//!
//! **A note on terminology**: when we mention the bridge hub here, we mean the chain that
//! has the messages pallet deployed (`pallet-bridge-grandpa`, `pallet-bridge-messages`,
//! `pallet-xcm-bridge-hub`, ...). It may be the system bridge hub parachain or any other
//! chain.

#![cfg_attr(not(feature = "std"), no_std)]

use bp_runtime::RangeInclusiveExt;
use bp_xcm_bridge_hub_router::{
	bridge_id_from_locations, Bridge, BridgeId, LocalXcmChannelManager,
};
use codec::{Codec, Compact, CompactLen, Encode};
use frame_support::traits::Get;
use sp_runtime::{BoundedVec, FixedPointNumber, FixedU128, SaturatedConversion, Saturating};
use xcm::prelude::*;
use xcm_builder::{ExporterFor, SovereignPaidRemoteExporter};

pub use pallet::*;
pub use weights::WeightInfo;

pub mod benchmarking;
pub mod weights;

mod mock;

/// Minimal delivery fee factor.
pub const MINIMAL_DELIVERY_FEE_FACTOR: FixedU128 = FixedU128::from_u32(1);

/// The factor that is used to increase current message fee factor when bridge experiencing
/// some lags.
const EXPONENTIAL_FEE_BASE: FixedU128 = FixedU128::from_rational(105, 100); // 1.05
/// The factor that is used to increase current message fee factor for every sent kilobyte.
const MESSAGE_SIZE_FEE_BASE: FixedU128 = FixedU128::from_rational(1, 1000); // 0.001

/// Maximal size of the XCM message that may be sent over bridge.
///
/// This should be less than the maximal size, allowed by the messages pallet, because
/// the message itself is wrapped in other structs and is double encoded.
pub const HARD_MESSAGE_SIZE_LIMIT: u32 = 32 * 1024;

/// Maximal size of suspended outbound message.
pub const HARD_SUSPENDED_MESSAGE_SIZE_LIMIT: u32 = HARD_MESSAGE_SIZE_LIMIT + 1_024;

/// The target that will be used when publishing logs related to this pallet.
///
/// This doesn't match the pattern used by other bridge pallets (`runtime::bridge-*`). But this
/// pallet has significant differences with those pallets. The main one is that is intended to
/// be deployed at sending chains. Other bridge pallets are likely to be deployed at the separate
/// bridge hub parachain.
pub const LOG_TARGET: &str = "xcm::bridge-hub-router";

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config {
		/// Benchmarks results from runtime we're plugged into.
		type WeightInfo: WeightInfo;

		/// Universal location of this runtime.
		type UniversalLocation: Get<InteriorMultiLocation>;
		/// Relative location of the sibling bridge hub.
		type SiblingBridgeHubLocation: Get<MultiLocation>;
		/// The bridged network that this config is for if specified.
		/// Also used for filtering `Bridges` by `BridgedNetworkId`.
		/// If not specified, allows all networks pass through.
		type BridgedNetworkId: Get<NetworkId>;

		/// Weight of the `ToBridgeHubSender::deliver` call.
		type ToBridgeHubSendWeight: Get<Weight>;
		/// Actual message sender (`HRMP` or `DMP`) to the sibling bridge hub location.
		type ToBridgeHubSender: SendXcm;
		/// Local XCM channel manager.
		type LocalXcmChannelManager: LocalXcmChannelManager;

		/// Base fee that is paid for every byte of the outbound message.
		type BaseFee: Get<u128>;
		/// Additional fee that is paid for every byte of the outbound message.
		type ByteFee: Get<u128>;
		/// Asset that is used to paid bridge fee.
		type FeeAsset: Get<AssetId>;
	}

	#[pallet::pallet]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::hooks]
	impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I> where
		<T::ToBridgeHubSender as SendXcm>::Ticket: Codec,
	{
		fn on_initialize(_n: BlockNumberFor<T>) -> Weight {
			// if XCM channel is still congested, we don't change anything
			if T::LocalXcmChannelManager::is_congested(&T::SiblingBridgeHubLocation::get()) {
				return T::WeightInfo::on_initialize_when_congested()
			}

			// if we can't decrease the congestion fee factor anymore, we don't change anything
			let mut congestion_fee_factor = Self::congestion_fee_factor();
			if congestion_fee_factor == MINIMAL_DELIVERY_FEE_FACTOR {
				return T::WeightInfo::on_initialize_when_congested()
			}

			let previous_factor = congestion_fee_factor;
			congestion_fee_factor =
				MINIMAL_DELIVERY_FEE_FACTOR.max(congestion_fee_factor / EXPONENTIAL_FEE_BASE);
			log::info!(
				target: LOG_TARGET,
				"With-bridge-hub channel is uncongested. Decreased congestion fee factor from {} to {}",
				previous_factor,
				congestion_fee_factor,
			);

			CongestionFeeFactor::<T, I>::put(congestion_fee_factor);
			T::WeightInfo::on_initialize_when_non_congested()
		}

		fn on_idle(_block: BlockNumberFor<T>, remaining_weight: Weight) -> Weight {
			// we need to read+update range + move (1 read + two writes) message
			let suspended_messages_range_read_weight = Self::suspended_messages_range_read_weight();
			let suspended_message_read_weight = Self::suspended_message_read_weight();
			let deliver_message_weight = T::ToBridgeHubSendWeight::get();
			let db_weight = T::DbWeight::get();
			let min_required_weight = suspended_messages_range_read_weight
				.saturating_add(suspended_message_read_weight)
				.saturating_add(deliver_message_weight)
				.saturating_add(db_weight.writes(2));
			if remaining_weight.any_lt(min_required_weight) {
				return Weight::zero();
			}

			let mut total_used_weight = suspended_messages_range_read_weight;
			SuspendedMessagesRange::<T, I>::mutate_exists(|range| {
				// nothing to do if there are no suspended messages
				let (mut message_index, range_end) = match *range {
					Some((message_index, range_end)) => (message_index, range_end),
					None => return,
				};

				// start delivering messages
				while message_index <= range_end {
					let mut message_used_weight = suspended_message_read_weight;
					let message = match SuspendedMessages::<T, I>::take(message_index) {
						Some(message) => message,
						None => {
							message_index += 1;
							total_used_weight.saturating_reduce(Weight::from_parts(0, HARD_SUSPENDED_MESSAGE_SIZE_LIMIT as _));

							continue;
						},
					};
					let message_len = message.len() as u32;
					
					// the `HARD_SUSPENDED_MESSAGE_SIZE_LIMIT` is quite large, so we want to decrease
					// the size of PoV if actual message is smaller than this limit
					message_used_weight.saturating_reduce(
						Weight::from_parts(
							0,
							HARD_SUSPENDED_MESSAGE_SIZE_LIMIT
								.saturating_sub(message_len.saturating_add(Compact::compact_len(&message_len) as _)) as _,
						)
					);

					// decode the ticket
					let ticket = match Decode::decode(&mut &message[..]) {
						Ok(ticket) => ticket,
						Err(e) => {
							log::debug!(
								target: LOG_TARGET,
								"Failed to decode suspended bridge message {}: {:?}. The message is dropped",
								message_index,
								e,
							);

							message_index += 1;
							total_used_weight.saturating_accrue(message_used_weight);
							continue;
						},
					};

					// deliver the message
					match T::ToBridgeHubSender::deliver(ticket) {
						Ok(_) => {
							log::debug!(
								target: LOG_TARGET,
								"Sending suspended bridge message {}. {} suspended messages remaining",
								message_index,
								((message_index + 1)..=range_end).saturating_len(),
							);
						},
						Err(e) => {
							log::debug!(
								target: LOG_TARGET,
								"Failed to deliver bridge message {}: {:?}",
								message_index,
								e,
							);
						},
					}

					message_index += 1;
					message_used_weight.saturating_accrue(deliver_message_weight);
					total_used_weight.saturating_accrue(message_used_weight);
				}

				*range = if (message_index..=range_end).is_empty() {
					None
				} else {
					Some((message_index, range_end))
				};

				total_used_weight.saturating_accrue(db_weight.writes(1))
			});

			total_used_weight
		}
	}

	/// Initialization value for the congestion fee factor.
	#[pallet::type_value]
	pub fn InitialCongestionFactor() -> FixedU128 {
		MINIMAL_DELIVERY_FEE_FACTOR
	}

	/// One (congestion-related) component of the number to multiply the base delivery fee by.
	///
	/// This factor is shared by all bridges, served by this pallet. For example, if this
	/// chain (`Config::UniversalLocation`) opens two bridges (
	/// `X2(GlobalConsensus(Config::BridgedNetworkId::get()), Parachain(1000))` and
	/// `X2(GlobalConsensus(Config::BridgedNetworkId::get()), Parachain(2000))`), then they
	/// both will be sharing the same congestion fee factor. This is because both bridges are
	/// sharing the same local XCM channel with the child/sibling bridge hub, which may be
	/// congested.
	#[pallet::storage]
	#[pallet::getter(fn congestion_fee_factor)]
	pub type CongestionFeeFactor<T: Config<I>, I: 'static = ()> =
		StorageValue<_, FixedU128, ValueQuery, InitialCongestionFactor>;

	/// All registered bridges and their delivery fee factors.
	#[pallet::storage]
	#[pallet::getter(fn bridge)]
	pub type Bridges<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Identity, BridgeId, Bridge<BlockNumberFor<T>>>;

	/// 
	#[pallet::storage]
	pub type SuspendedMessagesRange<T: Config<I>, I: 'static = ()> =
		StorageValue<_, (u64, u64), OptionQuery>;

	///
	#[pallet::storage]
	pub type SuspendedMessages<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Identity, u64, BoundedVec<u8, ConstU32<HARD_SUSPENDED_MESSAGE_SIZE_LIMIT>>>; // TODO: const

	impl<T: Config<I>, I: 'static> Pallet<T, I> where
		<T::ToBridgeHubSender as SendXcm>::Ticket: Encode,
	{
		/// Called when we receive a bridge-suspended signal.
		pub fn on_bridge_suspended(bridge_id: BridgeId) {
			Bridges::<T, I>::mutate_extant(bridge_id, |bridge| {
				bridge.bridge_resumed_at = None;
			});
		}

		/// Called when we receive a bridge-resume signal.
		pub fn on_bridge_resumed(bridge_id: BridgeId) {
			Bridges::<T, I>::mutate_extant(bridge_id, |bridge| {
				bridge.bridge_resumed_at = Some(frame_system::Pallet::<T>::block_number());
			});
		}

		/// Called when new message is sent (queued to local outbound XCM queue) over the bridge.
		pub(crate) fn on_message_sent_to_bridge(bridge_id: BridgeId, message_size: u32) {
			// both fee factor components are increased using the same `total_factor`
			let message_size_factor = FixedU128::from_u32(message_size.saturating_div(1024))
				.saturating_mul(MESSAGE_SIZE_FEE_BASE);
			let total_factor = EXPONENTIAL_FEE_BASE.saturating_add(message_size_factor);

			// if the channel with the sibling/child bridge hub is congested, let's increase
			// the congestion fee factor
			let is_bridge_hub_channel_congested =
				T::LocalXcmChannelManager::is_congested(&T::SiblingBridgeHubLocation::get());
			if is_bridge_hub_channel_congested {
				CongestionFeeFactor::<T, I>::mutate(|f| {
					let previous_factor = *f;
					*f = f.saturating_mul(total_factor);
					log::info!(
						target: LOG_TARGET,
						"With-bridge-hub channel is congested. Increased congestion fee factor from {} to {}",
						previous_factor,
						f,
					);
					*f
				});
			}

			// if the bridge is suspended, increase the bridge fee factor
			Bridges::<T, I>::mutate_extant(bridge_id, |bridge| {
				let is_bridge_suspended = bridge.is_suspended();
				if is_bridge_suspended {
					let previous_factor = bridge.bridge_fee_factor;
					bridge.bridge_fee_factor =
						bridge.bridge_fee_factor.saturating_mul(total_factor);
					log::info!(
						target: LOG_TARGET,
						"Bridge {:?} is suspended. Increased bridge fee factor from {} to {}",
						bridge_id,
						previous_factor,
						bridge.bridge_fee_factor,
					);
				}
			});
		}

		/// Return weight of reading the `SuspendedMessagesRange` value.
		fn suspended_messages_range_read_weight() -> Weight {
			Weight::zero() // TODO
		}

		/// Return weight of reading the maximal size message from `SuspendedMessages`.
		fn suspended_message_read_weight() -> Weight {
			Weight::zero() // TODO
		}
	}
}

/// We'll be using `SovereignPaidRemoteExporter` to send remote messages over the sibling/child
/// bridge hub.
type ViaBridgeHubExporter<T, I> = SovereignPaidRemoteExporter<
	Pallet<T, I>,
	<T as Config<I>>::ToBridgeHubSender,
	<T as Config<I>>::UniversalLocation,
>;

// This pallet acts as the `ExporterFor` for the `SovereignPaidRemoteExporter` to compute
// message fee using fee factor.
impl<T: Config<I>, I: 'static> ExporterFor for Pallet<T, I> {
	fn exporter_for(
		network: &NetworkId,
		remote_location: &InteriorMultiLocation,
		message: &Xcm<()>,
	) -> Option<(MultiLocation, Option<MultiAsset>)> {
		// compute bridge id using universal locations
		let mut bridge_destination_universal_location = X1(GlobalConsensus(*network));
		bridge_destination_universal_location.append_with(*remote_location).ok()?;
		let bridge_destination_universal_location = Box::new(bridge_destination_universal_location);
		let bridge_id = bridge_id_from_locations(
			&T::UniversalLocation::get(),
			&bridge_destination_universal_location,
		);

		// ensure that the bridge with remote destination exists
		let bridge = match Self::bridge(bridge_id) {
			Some(bridge) => bridge,
			None => {
				log::trace!(
					target: LOG_TARGET,
					"Router with bridged_network_id {:?} has no opened bridge with {:?}",
					T::BridgedNetworkId::get(),
					bridge_destination_universal_location,
				);

				return None
			},
		};

		// we do NOT decrease bridge-specific fee factor from every block `on_initialize` after
		// bridge is resumed. Instead, we compute an actual factor here, when we actually send
		// a message over the bridge.
		let mut bridge_fee_factor = bridge.bridge_fee_factor;
		if let Some(bridge_resumed_at) = bridge.bridge_resumed_at {
			let resumed_for_blocks =
				frame_system::Pallet::<T>::block_number().saturating_sub(bridge_resumed_at);
			bridge_fee_factor = MINIMAL_DELIVERY_FEE_FACTOR.max(
				bridge_fee_factor /
					EXPONENTIAL_FEE_BASE.saturating_pow(resumed_for_blocks.saturated_into()),
			);
		}

		// fee factor is comprised of two components - congestion fee factor and bridge fee factor.
		// They both are changed independently and the total factor is the sum of both factors.
		// (minus the `MINIMAL_DELIVERY_FEE_FACTOR`, because it is included into both components)
		let fee_factor = bridge_fee_factor
			.saturating_add(Self::congestion_fee_factor())
			.saturating_sub(MINIMAL_DELIVERY_FEE_FACTOR);

		// compute fee amount. Keep in mind that this is only the bridge fee. The fee for sending
		// message from this chain to child/sibling bridge hub is determined by the
		// `Config::ToBridgeHubSender`
		let base_fee = T::BaseFee::get();
		let message_size = message.encoded_size();
		let message_fee = (message_size as u128).saturating_mul(T::ByteFee::get());
		let fee_sum = base_fee.saturating_add(message_fee);
		let fee = fee_factor.saturating_mul_int(fee_sum);
		let fee = if fee > 0 { Some((T::FeeAsset::get(), fee).into()) } else { None };

		log::info!(
			target: LOG_TARGET,
			"Going to send message ({} bytes) over bridge. Computed bridge fee {:?} using fee factor {}",
			message_size,
			fee,
			fee_factor,
		);

		Some((T::SiblingBridgeHubLocation::get(), fee))
	}
}

// This pallet acts as the `SendXcm` to the sibling/child bridge hub instead of regular
// XCMP/DMP transport. This allows injecting dynamic message fees into XCM programs that
// are going to the bridged network.
impl<T: Config<I>, I: 'static> SendXcm for Pallet<T, I> where
	<T::ToBridgeHubSender as SendXcm>::Ticket: Encode,
{
	type Ticket = (BridgeId, u32, <T::ToBridgeHubSender as SendXcm>::Ticket);

	fn validate(
		dest: &mut Option<MultiLocation>,
		xcm: &mut Option<Xcm<()>>,
	) -> SendResult<Self::Ticket> {
		// we won't have an access to `dest` and `xcm` in the `delvier` method, so precompute
		// everything required here
		let message_size = xcm
			.as_ref()
			.map(|xcm| xcm.encoded_size() as _)
			.ok_or(SendError::MissingArgument)?;

		// bridge doesn't support oversized/overweight messages now. So it is better to drop such
		// messages here than at the bridge hub. Let's check the message size.
		if message_size > HARD_MESSAGE_SIZE_LIMIT {
			return Err(SendError::ExceedsMaxMessageSize)
		}

		// compute bridge id from universal locations
		let bridge_destination_relative_location =
			dest.as_ref().map(|dest| dest.clone()).ok_or(SendError::MissingArgument)?;
		let bridge_origin_universal_location = T::UniversalLocation::get();
		let bridge_destination_universal_location: InteriorMultiLocation =
			bridge_origin_universal_location
				.into_location()
				.appended_with(bridge_destination_relative_location)
				.map_err(|_| SendError::NotApplicable)?
				.try_into()
				.map_err(|_| SendError::NotApplicable)?;
		let bridge_id = bridge_id_from_locations(
			&T::UniversalLocation::get(),
			&bridge_destination_universal_location,
		);

		// just use exporter to validate destination and insert instructions to pay message fee
		// at the sibling/child bridge hub
		//
		// the cost will include both cost of: (1) to-sibling bridg hub delivery (returned by
		// the `Config::ToBridgeHubSender`) and (2) to-bridged bridge hub delivery (returned by
		// `Self::exporter_for`)
		ViaBridgeHubExporter::<T, I>::validate(dest, xcm)
			.map(|(ticket, cost)| ((bridge_id, message_size, ticket), cost))
	}

	fn deliver(ticket: Self::Ticket) -> Result<XcmHash, SendError> {
		// use router to enqueue message to the sibling/child bridge hub. This also should handle
		// payment for passing through this queue.
		let (bridge_id, message_size, ticket) = ticket;
		let bridge = Bridges::<T, I>::get(bridge_id).ok_or_else(|| SendError::Transport("UnknownBridge"))?;
		let xcm_hash = if bridge.is_suspended() {
			let xcm_hash = Default::default(); // TODO: what about message hash? is it important? where it is used?
			let message_index = SuspendedMessagesRange::<T, I>::mutate(|range| {
				let (start, end) = (*range).unwrap_or((1, 0));
				let message_index = end + 1;
				*range = Some((start, end));
				message_index
			});
			SuspendedMessages::<T, I>::insert(
				message_index,
				BoundedVec::<_, _>::try_from(ticket.encode()).expect("TODO"));
			xcm_hash
		} else {
			ViaBridgeHubExporter::<T, I>::deliver(ticket)?
		};

		// increase delivery fee factor if required
		Self::on_message_sent_to_bridge(bridge_id, message_size);

		Ok(xcm_hash)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use mock::*;

	use frame_support::traits::Hooks;
	use sp_runtime::traits::One;

	fn bridge_destination_relative_location() -> MultiLocation {
		MultiLocation::new(2, X2(GlobalConsensus(Rococo), Parachain(1000)))
	}

	fn bridge_id() -> BridgeId {
		BridgeId::new(
			&UniversalLocation::get().into(),
			&(*bridge_destination_relative_location().interior()).into(),
		)
	}

	fn insert_bridge() {
		Bridges::<TestRuntime, ()>::insert(
			bridge_id(),
			Bridge { bridge_fee_factor: MINIMAL_DELIVERY_FEE_FACTOR, bridge_resumed_at: Some(0) },
		);
	}

	#[test]
	fn initial_fee_factor_is_one() {
		run_test(|| {
			assert_eq!(CongestionFeeFactor::<TestRuntime, ()>::get(), MINIMAL_DELIVERY_FEE_FACTOR);
		})
	}

	#[test]
	fn congestion_fee_factor_is_not_decreased_from_on_initialize_when_queue_is_congested() {
		run_test(|| {
			CongestionFeeFactor::<TestRuntime, ()>::put(FixedU128::from_rational(125, 100));
			TestLocalXcmChannelManager::make_congested();

			// it should not decrease, because queue is congested
			let old_congestion_fee_factor = XcmBridgeHubRouter::congestion_fee_factor();
			XcmBridgeHubRouter::on_initialize(One::one());
			assert_eq!(XcmBridgeHubRouter::congestion_fee_factor(), old_congestion_fee_factor);
		})
	}

	#[test]
	fn congestion_fee_factor_is_decreased_from_on_initialize_when_queue_is_uncongested() {
		run_test(|| {
			CongestionFeeFactor::<TestRuntime, ()>::put(FixedU128::from_rational(125, 100));

			// it shold eventually decreased to one
			while XcmBridgeHubRouter::congestion_fee_factor() > MINIMAL_DELIVERY_FEE_FACTOR {
				XcmBridgeHubRouter::on_initialize(One::one());
			}

			// verify that it doesn't decreases anymore
			XcmBridgeHubRouter::on_initialize(One::one());
			assert_eq!(XcmBridgeHubRouter::congestion_fee_factor(), MINIMAL_DELIVERY_FEE_FACTOR);
		})
	}

	#[test]
	fn not_applicable_if_destination_is_within_other_network() {
		run_test(|| {
			assert_eq!(
				send_xcm::<XcmBridgeHubRouter>(
					bridge_destination_relative_location(),
					vec![].into(),
				),
				Err(SendError::NotApplicable),
			);
		});
	}

	#[test]
	fn exceeds_max_message_size_if_size_is_above_hard_limit() {
		run_test(|| {
			assert_eq!(
				send_xcm::<XcmBridgeHubRouter>(
					MultiLocation::new(2, X2(GlobalConsensus(Rococo), Parachain(1000))),
					vec![ClearOrigin; HARD_MESSAGE_SIZE_LIMIT as usize].into(),
				),
				Err(SendError::ExceedsMaxMessageSize),
			);
		});
	}

	#[test]
	fn returns_proper_delivery_price() {
		run_test(|| {
			insert_bridge();

			let dest = bridge_destination_relative_location();
			let xcm: Xcm<()> = vec![ClearOrigin].into();
			let msg_size = xcm.encoded_size();

			// initially the base fee is used: `BASE_FEE + BYTE_FEE * msg_size + HRMP_FEE`
			let expected_fee = BASE_FEE + BYTE_FEE * (msg_size as u128) + HRMP_FEE;
			assert_eq!(
				XcmBridgeHubRouter::validate(&mut Some(dest), &mut Some(xcm.clone()))
					.unwrap()
					.1
					.get(0),
				Some(&(BridgeFeeAsset::get(), expected_fee).into()),
			);

			// but when factor is larger than one, it increases the fee, so it becomes:
			// `(BASE_FEE + BYTE_FEE * msg_size) * F + HRMP_FEE`
			let factor = FixedU128::from_rational(125, 100);
			CongestionFeeFactor::<TestRuntime, ()>::put(factor);
			let expected_fee =
				(FixedU128::saturating_from_integer(BASE_FEE + BYTE_FEE * (msg_size as u128)) *
					factor)
					.into_inner() / FixedU128::DIV +
					HRMP_FEE;
			assert_eq!(
				XcmBridgeHubRouter::validate(&mut Some(dest), &mut Some(xcm)).unwrap().1.get(0),
				Some(&(BridgeFeeAsset::get(), expected_fee).into()),
			);
		});
	}

	#[test]
	fn sent_message_doesnt_increase_factor_if_queue_is_uncongested() {
		run_test(|| {
			insert_bridge();

			let old_congestion_fee_factor = XcmBridgeHubRouter::congestion_fee_factor();
			assert_eq!(
				send_xcm::<XcmBridgeHubRouter>(
					bridge_destination_relative_location(),
					vec![ClearOrigin].into(),
				)
				.map(drop),
				Ok(()),
			);

			assert!(TestToBridgeHubSender::is_message_sent());
			assert_eq!(old_congestion_fee_factor, XcmBridgeHubRouter::congestion_fee_factor());
		});
	}

	#[test]
	fn sent_message_increases_factor_if_queue_is_congested() {
		run_test(|| {
			insert_bridge();
			TestLocalXcmChannelManager::make_congested();

			let old_congestion_fee_factor = XcmBridgeHubRouter::congestion_fee_factor();
			assert_eq!(
				send_xcm::<XcmBridgeHubRouter>(
					bridge_destination_relative_location(),
					vec![ClearOrigin].into(),
				)
				.map(drop),
				Ok(()),
			);

			assert!(TestToBridgeHubSender::is_message_sent());
			assert!(old_congestion_fee_factor < XcmBridgeHubRouter::congestion_fee_factor());
		});
	}
}
