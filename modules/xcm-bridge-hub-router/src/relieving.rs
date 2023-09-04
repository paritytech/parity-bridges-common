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

//! Code used to process relieving bridges and their suspended messages.

use crate::{
	Bridges, Config, Pallet, SuspendedMessages, ToBridgeHubTicket, WeightInfo, WeightInfoExt,
	HARD_SUSPENDED_MESSAGE_SIZE_LIMIT, LOG_TARGET,
};

use bp_xcm_bridge_hub_router::{BridgeId, RelievingBridgesQueue};
use codec::{Compact, CompactLen, Decode};
use frame_support::weights::Weight;
use sp_core::Get;
use xcm::latest::prelude::*;

impl<T: Config<I>, I: 'static> Pallet<T, I>
where
	ToBridgeHubTicket<T, I>: Decode,
{
	/// Service all relieving bridges.
	pub(crate) fn service_relieving_bridges(
		mut queue: RelievingBridgesQueue<T::MaxBridges>,
		mut remaining_weight: Weight,
	) -> (Weight, Option<RelievingBridgesQueue<T::MaxBridges>>) {
		// it is already checked in `on_idle`, but let's repeat
		if queue.is_empty() {
			return (Weight::zero(), None)
		}

		let original_remaining_weight = remaining_weight;
		let db_weight = T::DbWeight::get();
		let minimal_required_weight_for_bridge =
			T::WeightInfo::minimal_weight_to_process_relieving_bridge(&db_weight);
		loop {
			// select next relieving bridge
			let bridge_id = match queue.current() {
				Some(bridge_id) => bridge_id,
				None => {
					// corrupted storage? let's restart iteration
					log::debug!(
						target: LOG_TARGET,
						"Index of current relieving bridge is invalid: {} vs max {}. Restarting iteration",
						queue.current,
						queue.bridges.len(),
					);

					queue.reset_current();
					continue
				},
			};

			// deliver suspended messages of selected bridge
			let (has_suspended_messages, used_weight) =
				Self::service_relieving_bridge(bridge_id, remaining_weight).unwrap_or_else(
					|used_weight|
				// it means that the bridge has been deleted. We won't try to service this bridge
				// again AND if there are suspended messages in the `SuspendedMessages` map, they'll
				// live there forever - it is the incorrect upgrade and we can't fight that
				(false, used_weight),
				);

			// remember that we have spent `used_weight` on servicing the `bridge_id`
			remaining_weight.saturating_reduce(used_weight);

			// update the `next_index` and/or remove the bridge
			if !has_suspended_messages {
				queue.remove_current();
			} else {
				queue.advance();
			}

			// if remaining weight is less than the weight required to service at least one
			// bridge => let's stop iteration
			if queue.is_empty() || remaining_weight.any_lt(minimal_required_weight_for_bridge) {
				return (original_remaining_weight.saturating_sub(remaining_weight), Some(queue))
			}
		}
	}

	/// Send suspended messages of given relieving bridge.
	///
	/// Returns used weight, wrapped into `Result` type. The return value is `Ok(_)` if we have
	/// found the bridge in the storage. It is `Err(_)` otherwise.
	pub(crate) fn service_relieving_bridge(
		bridge_id: BridgeId,
		mut remaining_weight: Weight,
	) -> Result<(bool, Weight), Weight> {
		let original_remaining_weight = remaining_weight;
		let db_weight = T::DbWeight::get();
		Bridges::<T, I>::mutate(bridge_id, |bridge| {
			// if there's no such bridge, we don't need to service it
			let mut mut_bridge = bridge.take().ok_or_else(|| {
				log::debug!(
					target: LOG_TARGET,
					"Relieving bridge {:?} is missing from the storage. Removing from the queue",
					bridge_id,
				);

				T::WeightInfo::bridge_read_weight()
			})?;
			let suspended_messages = mut_bridge.suspended_messages();

			// remember that we have read and later will update the bridge
			remaining_weight.saturating_reduce(
				T::WeightInfo::bridge_read_weight().saturating_add(db_weight.writes(1)),
			);

			// send suspended messages
			let mininal_required_weight_for_message =
				T::WeightInfo::minimal_weight_to_process_suspended_message(&db_weight);
			let mut message_index = *suspended_messages.start();
			let messages_end = *suspended_messages.end();
			loop {
				let weight_used_by_message = Self::send_suspended_message(bridge_id, message_index);
				remaining_weight.saturating_reduce(weight_used_by_message);

				message_index += 1;
				if message_index > messages_end {
					break
				}
				if remaining_weight.any_lt(mininal_required_weight_for_message) {
					break
				}
			}

			// update the storage value
			let has_suspended_messages = !(message_index..=messages_end).is_empty();
			mut_bridge.suspended_messages =
				if has_suspended_messages { Some((message_index, messages_end)) } else { None };

			// update the actual bridge
			*bridge = Some(mut_bridge);

			// weight that we have spent on servicing suspended messages
			Ok((has_suspended_messages, original_remaining_weight.saturating_sub(remaining_weight)))
		})
	}

	/// Send suspended message with given index to the sibling/child bridge hub. Returns used
	/// weight.
	pub(crate) fn send_suspended_message(bridge_id: BridgeId, message_index: u64) -> Weight {
		// by default we read maximal size message and remove it from the storage
		let mut used_weight = T::WeightInfo::suspended_message_read_weight();

		// let's read remove message from the runtime storage
		let message = SuspendedMessages::<T, I>::take(bridge_id, message_index);
		let message_len = message.as_ref().map(|message| message.len()).unwrap_or(0);
		let message_len_len = message
			.as_ref()
			.map(|message| Compact::compact_len(&(message.len() as u32)))
			.unwrap_or(0);

		// the `HARD_SUSPENDED_MESSAGE_SIZE_LIMIT` is quite large, so we want to decrease
		// the size of PoV if actual message is smaller than this limit
		used_weight.saturating_reduce(Weight::from_parts(
			0,
			HARD_SUSPENDED_MESSAGE_SIZE_LIMIT
				.saturating_sub((message_len as u32).saturating_add(message_len_len as u32)) as u64,
		));

		// if message is missing from the storage (meaning something is corrupted), we are
		// not doing anything else
		let message = if let Some(message) = message { message } else { return used_weight };
		used_weight.saturating_accrue(T::DbWeight::get().writes(1));

		// the message is the encoded ticket for `T::ToBridgeHubSender`. Let's try to decode it
		let ticket = match Decode::decode(&mut &message[..]) {
			Ok(ticket) => ticket,
			Err(e) => {
				log::debug!(
					target: LOG_TARGET,
					"Failed to decode relieving bridge {:?} message {} to {:?}: {:?}. The message is dropped",
					bridge_id,
					message_index,
					T::SiblingBridgeHubLocation::get(),
					e,
				);

				return used_weight
			},
		};

		// finally - deliver the ticket
		match T::ToBridgeHubSender::deliver(ticket) {
			Ok(_) => {
				log::debug!(
					target: LOG_TARGET,
					"Sending relieving bridge {:?} message {} to {:?}",
					bridge_id,
					message_index,
					T::SiblingBridgeHubLocation::get(),
				);
			},
			Err(e) => {
				log::debug!(
					target: LOG_TARGET,
					"Failed to deliver relieving bridge {:?} message {} to {:?}: {:?}",
					bridge_id,
					message_index,
					T::SiblingBridgeHubLocation::get(),
					e,
				);
			},
		}

		used_weight.saturating_add(T::WeightInfo::to_bridge_hub_deliver_weight())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{mock::*, RelievingBridges, MINIMAL_DELIVERY_FEE_FACTOR};

	use bp_xcm_bridge_hub_router::Bridge;
	use codec::Encode;
	use frame_support::{traits::Hooks, BoundedVec};

	fn bridge1_id() -> BridgeId {
		BridgeId::new(&Here.into(), &X2(GlobalConsensus(Kusama), Parachain(1)).into())
	}

	fn bridge2_id() -> BridgeId {
		BridgeId::new(&Here.into(), &X2(GlobalConsensus(Kusama), Parachain(2)).into())
	}

	fn insert_suspended_message(bridge_id: BridgeId, index: u64, size: u32) {
		SuspendedMessages::<TestRuntime, ()>::insert(
			bridge_id,
			index,
			BoundedVec::try_from(vec![0u8; size as usize]).unwrap(),
		);
	}

	fn insert_relieving_bridge(bridge_id: BridgeId, suspended_messages: u64) {
		Bridges::<TestRuntime, ()>::insert(
			bridge_id,
			Bridge {
				bridge_fee_factor: MINIMAL_DELIVERY_FEE_FACTOR,
				bridge_resumed_at: Some(0),
				suspended_messages: Some((1, suspended_messages)),
			},
		);

		RelievingBridges::<TestRuntime, ()>::mutate(|queue| {
			*queue = Some(match queue.take() {
				Some(mut queue) => {
					queue.try_push(bridge_id).unwrap();
					queue
				},
				None => RelievingBridgesQueue::with(bridge_id),
			})
		});

		for message_index in 1..=suspended_messages {
			insert_suspended_message(bridge_id, message_index, HARD_SUSPENDED_MESSAGE_SIZE_LIMIT);
		}
	}

	fn run_test_with_relieving_bridges(test: impl Fn()) {
		run_test(|| {
			insert_relieving_bridge(bridge1_id(), 4);
			insert_relieving_bridge(bridge2_id(), 5);

			test();
		});
	}

	#[test]
	fn relieving_bridges_are_not_serviced_if_weight_is_not_enough() {
		run_test_with_relieving_bridges(|| {
			assert_eq!(XcmBridgeHubRouter::on_idle(0, Weight::zero()), Weight::zero(),);

			assert_eq!(
				XcmBridgeHubRouter::bridge(bridge1_id()).map(|b| b.is_relieving()),
				Some(true)
			);
			assert_eq!(
				XcmBridgeHubRouter::bridge(bridge2_id()).map(|b| b.is_relieving()),
				Some(true)
			);
			assert_eq!(
				XcmBridgeHubRouter::relieving_bridges(),
				RelievingBridgesQueue::with_bridges(vec![bridge1_id(), bridge2_id()]),
			);
			assert!(XcmBridgeHubRouter::suspended_message(bridge1_id(), 1).is_some());
			assert!(XcmBridgeHubRouter::suspended_message(bridge1_id(), 2).is_some());
			assert!(XcmBridgeHubRouter::suspended_message(bridge1_id(), 3).is_some());
			assert!(XcmBridgeHubRouter::suspended_message(bridge1_id(), 4).is_some());
			assert!(XcmBridgeHubRouter::suspended_message(bridge2_id(), 1).is_some());
			assert!(XcmBridgeHubRouter::suspended_message(bridge2_id(), 2).is_some());
			assert!(XcmBridgeHubRouter::suspended_message(bridge2_id(), 3).is_some());
			assert!(XcmBridgeHubRouter::suspended_message(bridge2_id(), 4).is_some());
			assert!(XcmBridgeHubRouter::suspended_message(bridge2_id(), 5).is_some());
		});
	}

	#[test]
	fn relieving_bridges_are_deleted_if_empty() {
		run_test_with_relieving_bridges(|| {
			RelievingBridges::<TestRuntime, ()>::mutate(|queue| {
				*queue = Some({
					let mut queue = queue.take().unwrap();
					while !queue.is_empty() {
						queue.remove_current();
					}
					queue
				});
			});

			assert_eq!(
				XcmBridgeHubRouter::on_idle(0, Weight::MAX),
				TestWeightInfo::relieving_bridges_read_weight()
					.saturating_add(DbWeight::get().writes(1)),
			);
		});
	}

	#[test]
	fn used_weight_when_no_relieved_bridges() {
		run_test_with_relieving_bridges(|| {
			RelievingBridges::<TestRuntime, ()>::set(None);

			assert_eq!(
				XcmBridgeHubRouter::on_idle(0, Weight::MAX),
				TestWeightInfo::relieving_bridges_read_weight(),
			);
		});
	}

	#[test]
	fn bridge_is_removed_from_relieving_if_it_is_missing_from_the_storage() {
		run_test_with_relieving_bridges(|| {
			Bridges::<TestRuntime, ()>::remove(bridge1_id());

			let db_weight = DbWeight::get();
			assert_eq!(
				XcmBridgeHubRouter::on_idle(
					0,
					TestWeightInfo::minimal_weight_to_process_suspended_messages(&db_weight)
				),
				TestWeightInfo::relieving_bridges_read_weight()
					.saturating_add(TestWeightInfo::bridge_read_weight())
					.saturating_add(db_weight.writes(1)),
			);
		});
	}

	#[test]
	fn used_weight_when_message_is_missing_from_the_storage() {
		run_test_with_relieving_bridges(|| {
			SuspendedMessages::<TestRuntime, ()>::remove(bridge1_id(), 1);

			let db_weight = DbWeight::get();
			assert_eq!(
				XcmBridgeHubRouter::on_idle(
					0,
					TestWeightInfo::minimal_weight_to_process_suspended_messages(&db_weight)
				),
				TestWeightInfo::relieving_bridges_read_weight()
					// update RelievingBridges
					.saturating_add(db_weight.writes(1))
					// read bridge
					.saturating_add(TestWeightInfo::bridge_read_weight())
					// update bridge
					.saturating_add(db_weight.writes(1))
					// read zero-size message
					.saturating_add(TestWeightInfo::suspended_message_read_weight())
					.saturating_sub(Weight::from_parts(
						0,
						HARD_SUSPENDED_MESSAGE_SIZE_LIMIT as u64
					)),
			);
		});
	}

	#[test]
	fn used_weight_when_we_fail_to_decode_ticket() {
		run_test_with_relieving_bridges(|| {
			let mut invalid_message = vec![42].encode();
			invalid_message.insert(0, 0xFF);
			let invalid_message_len =
				invalid_message.len() + Compact::compact_len(&(invalid_message.len() as u32));
			SuspendedMessages::<TestRuntime, ()>::insert(
				bridge1_id(),
				1,
				BoundedVec::try_from(invalid_message).unwrap(),
			);

			let db_weight = DbWeight::get();
			assert_eq!(
				XcmBridgeHubRouter::on_idle(
					0,
					TestWeightInfo::minimal_weight_to_process_suspended_messages(&db_weight)
				),
				TestWeightInfo::relieving_bridges_read_weight()
					// update RelievingBridges
					.saturating_add(db_weight.writes(1))
					// read bridge
					.saturating_add(TestWeightInfo::bridge_read_weight())
					// update bridge
					.saturating_add(db_weight.writes(1))
					// read zero-size message
					.saturating_add(TestWeightInfo::suspended_message_read_weight())
					.saturating_sub(Weight::from_parts(
						0,
						(HARD_SUSPENDED_MESSAGE_SIZE_LIMIT - invalid_message_len as u32) as u64
					))
					// remove message
					.saturating_add(db_weight.writes(1)),
			);

			assert!(XcmBridgeHubRouter::suspended_message(bridge1_id(), 1).is_none());
		});
	}

	#[test]
	fn relieving_bridges_are_fully_serviced() {
		run_test_with_relieving_bridges(|| {
			XcmBridgeHubRouter::on_idle(0, Weight::MAX);

			assert_eq!(
				XcmBridgeHubRouter::bridge(bridge1_id()).map(|b| b.is_relieving()),
				Some(false)
			);
			assert_eq!(
				XcmBridgeHubRouter::bridge(bridge2_id()).map(|b| b.is_relieving()),
				Some(false)
			);
			assert_eq!(RelievingBridges::<TestRuntime, ()>::get(), None);
			assert!(XcmBridgeHubRouter::suspended_message(bridge1_id(), 1).is_none());
			assert!(XcmBridgeHubRouter::suspended_message(bridge1_id(), 2).is_none());
			assert!(XcmBridgeHubRouter::suspended_message(bridge1_id(), 3).is_none());
			assert!(XcmBridgeHubRouter::suspended_message(bridge1_id(), 4).is_none());
			assert!(XcmBridgeHubRouter::suspended_message(bridge2_id(), 1).is_none());
			assert!(XcmBridgeHubRouter::suspended_message(bridge2_id(), 2).is_none());
			assert!(XcmBridgeHubRouter::suspended_message(bridge2_id(), 3).is_none());
			assert!(XcmBridgeHubRouter::suspended_message(bridge2_id(), 4).is_none());
			assert!(XcmBridgeHubRouter::suspended_message(bridge2_id(), 5).is_none());
		})
	}

	#[test]
	fn relieving_bridges_are_partially_serviced() {
		run_test_with_relieving_bridges(|| {
			let db_weight = DbWeight::get();

			// call `on_idle` with weight that covers two messages delivery
			let weight_for_first_two_messages =
				TestWeightInfo::minimal_weight_to_process_suspended_messages(&db_weight)
					.saturating_add(TestWeightInfo::minimal_weight_to_process_suspended_message(
						&db_weight,
					));
			assert_eq!(
				XcmBridgeHubRouter::on_idle(0, weight_for_first_two_messages),
				weight_for_first_two_messages,
			);

			// we have been able to deliver first two messages only and no bridges are removed
			// from relieving set
			assert_eq!(
				XcmBridgeHubRouter::bridge(bridge1_id()).map(|b| b.is_relieving()),
				Some(true)
			);
			assert_eq!(
				XcmBridgeHubRouter::bridge(bridge2_id()).map(|b| b.is_relieving()),
				Some(true)
			);
			assert_eq!(
				RelievingBridges::<TestRuntime, ()>::get(),
				RelievingBridgesQueue::with_bridges(vec![bridge1_id(), bridge2_id()]).map(
					|mut queue| {
						queue.current = 1;
						queue
					}
				),
			);
			assert!(XcmBridgeHubRouter::suspended_message(bridge1_id(), 1).is_none());
			assert!(XcmBridgeHubRouter::suspended_message(bridge1_id(), 2).is_none());
			assert!(XcmBridgeHubRouter::suspended_message(bridge1_id(), 3).is_some());
			assert!(XcmBridgeHubRouter::suspended_message(bridge1_id(), 4).is_some());
			assert!(XcmBridgeHubRouter::suspended_message(bridge2_id(), 1).is_some());
			assert!(XcmBridgeHubRouter::suspended_message(bridge2_id(), 2).is_some());
			assert!(XcmBridgeHubRouter::suspended_message(bridge2_id(), 3).is_some());
			assert!(XcmBridgeHubRouter::suspended_message(bridge2_id(), 4).is_some());
			assert!(XcmBridgeHubRouter::suspended_message(bridge2_id(), 5).is_some());

			// now call `on_idle` with weight that covers 6 messages delivery
			let weight_for_first_six_messages =
				TestWeightInfo::minimal_weight_to_process_suspended_messages(&db_weight)
					.saturating_add(
						TestWeightInfo::minimal_weight_to_process_suspended_message(&db_weight)
							.saturating_mul(5),
					)
					.saturating_add(TestWeightInfo::bridge_read_weight())
					.saturating_add(db_weight.writes(1));
			assert_eq!(
				XcmBridgeHubRouter::on_idle(0, weight_for_first_six_messages),
				weight_for_first_six_messages,
			);

			// 2nd bridge is removed from relieving set
			assert_eq!(
				XcmBridgeHubRouter::bridge(bridge1_id()).map(|b| b.is_relieving()),
				Some(true)
			);
			assert_eq!(
				XcmBridgeHubRouter::bridge(bridge2_id()).map(|b| b.is_relieving()),
				Some(false)
			);
			assert_eq!(
				RelievingBridges::<TestRuntime, ()>::get(),
				RelievingBridgesQueue::with_bridges(vec![bridge1_id()]),
			);
			assert!(XcmBridgeHubRouter::suspended_message(bridge1_id(), 1).is_none());
			assert!(XcmBridgeHubRouter::suspended_message(bridge1_id(), 2).is_none());
			assert!(XcmBridgeHubRouter::suspended_message(bridge1_id(), 3).is_none());
			assert!(XcmBridgeHubRouter::suspended_message(bridge1_id(), 4).is_some());
			assert!(XcmBridgeHubRouter::suspended_message(bridge2_id(), 1).is_none());
			assert!(XcmBridgeHubRouter::suspended_message(bridge2_id(), 2).is_none());
			assert!(XcmBridgeHubRouter::suspended_message(bridge2_id(), 3).is_none());
			assert!(XcmBridgeHubRouter::suspended_message(bridge2_id(), 4).is_none());
			assert!(XcmBridgeHubRouter::suspended_message(bridge2_id(), 5).is_none());
		});
	}

	#[test]
	fn able_to_continue_when_current_index_is_invalid() {
		run_test_with_relieving_bridges(|| {
			RelievingBridges::<TestRuntime, ()>::mutate_exists(|queue| {
				*queue = Some({
					let mut queue = queue.take().unwrap();
					queue.current = queue.bridges.len() as u32 + 100;
					queue
				});
			});

			XcmBridgeHubRouter::on_idle(0, Weight::MAX);
			assert_eq!(RelievingBridges::<TestRuntime, ()>::get(), None);
		});
	}
}
