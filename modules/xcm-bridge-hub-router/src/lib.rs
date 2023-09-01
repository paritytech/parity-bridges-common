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

use bp_xcm_bridge_hub_router::{
	bridge_id_from_locations, Bridge, BridgeId, LocalXcmChannelManager, RelievingBridgesQueue,
};
use codec::{Codec, Encode};
use frame_support::{traits::Get, Hashable};
use sp_runtime::{BoundedVec, FixedPointNumber, FixedU128, SaturatedConversion, Saturating};
use sp_std::boxed::Box;
use xcm::prelude::*;
use xcm_builder::{ExporterFor, SovereignPaidRemoteExporter};

pub use pallet::*;
pub use weights::WeightInfo;
pub use weights_ext::WeightInfoExt;

pub mod benchmarking;
pub mod weights;
pub mod weights_ext;

mod mock;
mod relieving;

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

/// Maximal size of suspended outbound ticket.
pub const HARD_SUSPENDED_MESSAGE_SIZE_LIMIT: u32 = HARD_MESSAGE_SIZE_LIMIT + 4 * 1_024;

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
		type WeightInfo: WeightInfoExt;

		/// Maximal number of bridges, supported by this pallet.
		#[pallet::constant]
		type MaxBridges: Get<u32> + Get<Option<u32>>;

		/// Universal location of this runtime.
		type UniversalLocation: Get<InteriorMultiLocation>;
		/// Relative location of the sibling bridge hub.
		type SiblingBridgeHubLocation: Get<MultiLocation>;
		/// The bridged network that this config is for if specified.
		/// Also used for filtering `Bridges` by `BridgedNetworkId`.
		/// If not specified, allows all networks pass through.
		type BridgedNetworkId: Get<NetworkId>;

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

	/// A type alias for `ToBridgeHubSender` ticket. Such tickets are saved into runtime storage
	/// when message is sent over suspended bridge. Later, when bridge is resumed, tickets are
	/// actually delivered to the sibling/child bridge hub.
	pub type ToBridgeHubTicket<T, I> = <<T as Config<I>>::ToBridgeHubSender as SendXcm>::Ticket;

	#[pallet::pallet]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::hooks]
	impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I>
	where
		ToBridgeHubTicket<T, I>: Decode,
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

		fn on_idle(_block: BlockNumberFor<T>, mut remaining_weight: Weight) -> Weight {
			// check if we can do anything given the remaining weight
			let db_weight = T::DbWeight::get();
			if remaining_weight
				.any_lt(T::WeightInfo::minimal_weight_to_process_suspended_messages(&db_weight))
			{
				return Weight::zero()
			}

			// ok, we can - let's start servicing relieving bridges
			RelievingBridges::<T, I>::mutate(|relieving_bridges| {
				// remember that we have read the `RelievingBridges`
				let read_weight = T::WeightInfo::relieving_bridges_read_weight();
				let read_and_write_weight = read_weight.saturating_add(db_weight.writes(1));

				// nothing to do if there are no relieving bridges
				let queue = match relieving_bridges.take() {
					Some(queue) if !queue.is_empty() => queue,
					Some(_) => {
						// this can be caused e.g. by corrupted storage
						*relieving_bridges = None;
						return read_and_write_weight
					},
					None => return read_weight,
				};

				// remember that we will update the `RelievingBridges` later
				remaining_weight.saturating_reduce(
					T::WeightInfo::relieving_bridges_read_weight()
						.saturating_add(db_weight.writes(1)),
				);

				// try to service all relieving bridges
				let (used_weight, queue) = Self::service_relieving_bridges(queue, remaining_weight);
				*relieving_bridges =
					queue.and_then(|queue| if queue.is_empty() { None } else { Some(queue) });
				used_weight.saturating_add(read_and_write_weight)
			})
		}
	}

	/// Initialization value for the congestion fee factor.
	#[pallet::type_value]
	pub fn InitialCongestionFactor() -> FixedU128 {
		MINIMAL_DELIVERY_FEE_FACTOR
	}

	/// First (congestion-related) component of the number to multiply the base delivery fee by.
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
	pub type Bridges<T: Config<I>, I: 'static = ()> = StorageMap<
		Hasher = Identity,
		Key = BridgeId,
		Value = Bridge<BlockNumberFor<T>>,
		QueryKind = OptionQuery,
		OnEmpty = GetDefault,
		MaxValues = T::MaxBridges,
	>;

	/// Identifiers of bridges, which have been resumed and still have suspended messages. Once
	/// all suspended messages are pushed further, the bridge identifier is removed from this
	/// vector.
	#[pallet::storage]
	#[pallet::getter(fn relieving_bridges)]
	pub type RelievingBridges<T: Config<I>, I: 'static = ()> =
		StorageValue<_, RelievingBridgesQueue<T::MaxBridges>, OptionQuery>;

	/// All currently suspended messages.
	///
	/// Suspended message is actually an encoded ticket for `T::ToBridgeHubSender`. We are not
	/// saving tickets directly here, because they do not need to provide `TypeInfo`.
	#[pallet::storage]
	#[pallet::getter(fn suspended_message)]
	pub type SuspendedMessages<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
		_,
		Identity,
		BridgeId,
		Identity,
		u64,
		BoundedVec<u8, ConstU32<HARD_SUSPENDED_MESSAGE_SIZE_LIMIT>>,
	>;

	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		/// Called when we receive a bridge-suspended signal.
		pub fn on_bridge_suspended(bridge_id: BridgeId) {
			Bridges::<T, I>::mutate_extant(bridge_id, |bridge| {
				bridge.bridge_resumed_at = None;
				RelievingBridges::<T, I>::mutate(|relieving_bridges| {
					if let Some(ref mut queue) = relieving_bridges {
						queue.remove(bridge_id);
					}
					if relieving_bridges.as_ref().map(|q| q.is_empty()).unwrap_or(false) {
						*relieving_bridges = None;
					}
				});
			});
		}

		/// Called when we receive a bridge-resume signal.
		pub fn on_bridge_resumed(bridge_id: BridgeId) {
			Bridges::<T, I>::mutate_extant(bridge_id, |bridge| {
				bridge.bridge_resumed_at = Some(frame_system::Pallet::<T>::block_number());
				if !bridge.suspended_messages().is_empty() {
					RelievingBridges::<T, I>::mutate(|relieving_bridges| {
						// it shall not fail if everything is configured properly, because number
						// of relieving bridges is equal to maximal number of bridges
						match relieving_bridges {
							Some(queue) => {
								let _ = queue.try_push(bridge_id).map_err(|_| {
									log::info!(
										target: LOG_TARGET,
										"Failed to remember relieving bridge {:?}. Suspended messages may \
										never be delivered or delivered out of order",
										bridge_id,
									);
								});
							},
							None =>
								*relieving_bridges = Some(RelievingBridgesQueue::with(bridge_id)),
						}
					});
				}
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
impl<T: Config<I>, I: 'static> SendXcm for Pallet<T, I>
where
	<T::ToBridgeHubSender as SendXcm>::Ticket: Codec,
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
			dest.as_ref().copied().ok_or(SendError::MissingArgument)?;
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
		let mut bridge =
			Bridges::<T, I>::get(bridge_id).ok_or(SendError::Transport("UnknownBridge"))?;
		let xcm_hash = if bridge.is_suspended() {
			// TODO: this is the most worrysome code here - I'm not sure that it is ok to encode and
			// save the **ticket**. Maybe we need to support just some ticket portion and then
			// reconstruct the ticket?
			let encoded_ticket = ticket.encode();
			// TODO: not sure if there's some consistency for computing that?
			let xcm_hash = encoded_ticket.blake2_256();
			let suspended_message = BoundedVec::<_, _>::try_from(encoded_ticket)
				.map_err(|_| SendError::Transport("TooLargeTicket"))?;
			let message_index = bridge.select_next_suspended_message_index();
			Bridges::<T, I>::insert(bridge_id, bridge);
			SuspendedMessages::<T, I>::insert(bridge_id, message_index, suspended_message);
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
			Bridge {
				bridge_fee_factor: MINIMAL_DELIVERY_FEE_FACTOR,
				bridge_resumed_at: Some(0),
				suspended_messages: None,
			},
		);
	}

	fn suspend_bridge() {
		Bridges::<TestRuntime, ()>::mutate_extant(bridge_id(), |bridge| {
			bridge.bridge_resumed_at = None;
		});
	}

	fn resume_bridge() {
		Bridges::<TestRuntime, ()>::mutate_extant(bridge_id(), |bridge| {
			bridge.bridge_resumed_at = Some(System::block_number());
		});
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
	fn congestion_fee_factor_affects_delivery_price() {
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
	fn bridge_fee_factor_affects_delivery_price() {
		run_test(|| {
			insert_bridge();

			// let's say at block 100 the bridge was suspended and then we've sent 10 messages over
			// it
			System::set_block_number(100);
			suspend_bridge();
			for _ in 0..10 {
				let previous_factor =
					XcmBridgeHubRouter::bridge(bridge_id()).unwrap().bridge_fee_factor;
				assert_eq!(
					send_xcm::<XcmBridgeHubRouter>(
						bridge_destination_relative_location(),
						vec![ClearOrigin].into(),
					)
					.map(drop),
					Ok(()),
				);

				// every sent message increases the bridge fee factor
				assert!(
					previous_factor <
						XcmBridgeHubRouter::bridge(bridge_id()).unwrap().bridge_fee_factor
				);
			}

			// the bridge is resumed at block 150
			System::set_block_number(150);
			resume_bridge();
			{
				let previous_factor =
					XcmBridgeHubRouter::bridge(bridge_id()).unwrap().bridge_fee_factor;
				assert_eq!(
					send_xcm::<XcmBridgeHubRouter>(
						bridge_destination_relative_location(),
						vec![ClearOrigin].into(),
					)
					.map(drop),
					Ok(()),
				);

				// every sent message increases the bridge fee factor
				assert_eq!(
					previous_factor,
					XcmBridgeHubRouter::bridge(bridge_id()).unwrap().bridge_fee_factor
				);
			}

			// then at every following block the price is cheaper and eventually reaches the minimal
			let dest = bridge_destination_relative_location();
			let xcm: Xcm<()> = vec![ClearOrigin].into();
			let msg_size = xcm.encoded_size();
			let minimal_price = BASE_FEE + BYTE_FEE * (msg_size as u128) + HRMP_FEE;
			let mut previous_price =
				XcmBridgeHubRouter::validate(&mut Some(dest), &mut Some(xcm.clone()))
					.unwrap()
					.1
					.get(0)
					.unwrap()
					.fun
					.clone();
			loop {
				System::set_block_number(System::block_number() + 1);

				let price = XcmBridgeHubRouter::validate(&mut Some(dest), &mut Some(xcm.clone()))
					.unwrap()
					.1
					.get(0)
					.unwrap()
					.fun
					.clone();
				if price < previous_price {
					previous_price = price
				} else {
					assert_eq!(price, minimal_price.into());
					break
				}
			}
		});
	}

	#[test]
	fn both_factors_affect_price() {
		run_test(|| {
			insert_bridge();

			// increase bridge fee factor
			Bridges::<TestRuntime, ()>::mutate_extant(bridge_id(), |bridge| {
				bridge.bridge_fee_factor = MINIMAL_DELIVERY_FEE_FACTOR.saturating_mul(2u128.into());
			});

			// compute the price with single increased factor
			let dest = bridge_destination_relative_location();
			let xcm: Xcm<()> = vec![ClearOrigin].into();
			let price_with_bridge_factor =
				XcmBridgeHubRouter::validate(&mut Some(dest), &mut Some(xcm.clone()))
					.unwrap()
					.1
					.get(0)
					.unwrap()
					.fun
					.clone();

			// increase congestion fee factor
			CongestionFeeFactor::<TestRuntime, ()>::mutate(|factor| {
				*factor = MINIMAL_DELIVERY_FEE_FACTOR.saturating_mul(2u128.into());
			});

			// compute the price with two increased factor
			let price_with_two_factors =
				XcmBridgeHubRouter::validate(&mut Some(dest), &mut Some(xcm.clone()))
					.unwrap()
					.1
					.get(0)
					.unwrap()
					.fun
					.clone();

			assert!(price_with_two_factors > price_with_bridge_factor);
		});
	}

	#[test]
	fn message_is_not_sent_if_bridge_is_not_registered() {
		run_test(|| {
			assert_eq!(
				send_xcm::<XcmBridgeHubRouter>(
					bridge_destination_relative_location(),
					vec![ClearOrigin].into(),
				)
				.map(drop)
				.map_err(drop),
				Err(()),
			);
		});
	}

	#[test]
	fn sent_message_doesnt_increase_congestion_factor_if_queue_is_uncongested() {
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
	fn sent_message_increases_congestion_factor_if_queue_is_congested() {
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

	#[test]
	fn sent_message_doesnt_increase_bridge_fee_factor_factor_if_bridge_is_not_suspended() {
		run_test(|| {
			insert_bridge();

			let old_bridge_fee_factor =
				XcmBridgeHubRouter::bridge(bridge_id()).unwrap().bridge_fee_factor;
			assert_eq!(
				send_xcm::<XcmBridgeHubRouter>(
					bridge_destination_relative_location(),
					vec![ClearOrigin].into(),
				)
				.map(drop),
				Ok(()),
			);

			assert_eq!(
				old_bridge_fee_factor,
				XcmBridgeHubRouter::bridge(bridge_id()).unwrap().bridge_fee_factor
			);
		});
	}

	#[test]
	fn sent_message_increases_bridge_fee_factor_if_bridge_is_suspended() {
		run_test(|| {
			insert_bridge();
			suspend_bridge();

			let old_bridge_fee_factor =
				XcmBridgeHubRouter::bridge(bridge_id()).unwrap().bridge_fee_factor;
			assert_eq!(
				send_xcm::<XcmBridgeHubRouter>(
					bridge_destination_relative_location(),
					vec![ClearOrigin].into(),
				)
				.map(drop),
				Ok(()),
			);

			assert!(
				old_bridge_fee_factor <
					XcmBridgeHubRouter::bridge(bridge_id()).unwrap().bridge_fee_factor
			);
		});
	}

	#[test]
	fn sent_message_is_suspended_if_bridge_is_suspended() {
		run_test(|| {
			insert_bridge();
			suspend_bridge();

			assert_eq!(
				send_xcm::<XcmBridgeHubRouter>(
					bridge_destination_relative_location(),
					vec![ClearOrigin].into(),
				)
				.map(drop),
				Ok(()),
			);

			assert!(!TestToBridgeHubSender::is_message_sent());
			assert!(matches!(
				XcmBridgeHubRouter::bridge(bridge_id()),
				Some(Bridge { bridge_resumed_at: None, suspended_messages: Some((1, 1)), .. }),
			));
			assert!(XcmBridgeHubRouter::suspended_message(bridge_id(), 1).is_some());
		});
	}

	#[test]
	fn on_bridge_suspended_suspends_the_bridge() {
		run_test(|| {
			insert_bridge();
			RelievingBridges::<TestRuntime, ()>::put(RelievingBridgesQueue::with(bridge_id()));

			XcmBridgeHubRouter::on_bridge_suspended(bridge_id());
			assert!(matches!(
				XcmBridgeHubRouter::bridge(bridge_id()),
				Some(Bridge { bridge_resumed_at: None, .. }),
			));
			assert_eq!(RelievingBridges::<TestRuntime, ()>::get(), None)
		});
	}

	#[test]
	fn on_bridge_resumed_resumes_the_bridge() {
		run_test(|| {
			insert_bridge();
			suspend_bridge();

			XcmBridgeHubRouter::on_bridge_resumed(bridge_id());
			assert!(matches!(
				XcmBridgeHubRouter::bridge(bridge_id()),
				Some(Bridge { bridge_resumed_at: Some(0), .. }),
			));
			assert_eq!(RelievingBridges::<TestRuntime, ()>::get(), None)
		});
	}

	#[test]
	fn on_bridge_resumed_starts_relieving_the_bridge() {
		run_test(|| {
			insert_bridge();
			suspend_bridge();

			// suspend the message
			assert_eq!(
				send_xcm::<XcmBridgeHubRouter>(
					bridge_destination_relative_location(),
					vec![ClearOrigin].into(),
				)
				.map(drop),
				Ok(()),
			);

			// resume the bridge
			XcmBridgeHubRouter::on_bridge_resumed(bridge_id());

			// ensure that it has been added to the relieving bridges set, because
			// it has the suspended messages
			assert_eq!(
				RelievingBridges::<TestRuntime, ()>::get(),
				Some(RelievingBridgesQueue::with(bridge_id()))
			)
		});
	}
}
