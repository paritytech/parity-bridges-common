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

		let original_remaining_weight = remaining_weight.clone();
		let db_weight = T::DbWeight::get();
		let minimal_required_weight_for_bridge =
			T::WeightInfo::minimal_weight_to_process_relieving_bridge(&db_weight);
		loop {
			// select next relieving bridge
			let bridge_id = match queue.current() {
				Some(bridge_id) => bridge_id.clone(),
				None => {
					// corrupted storage, let's restart iteration
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
		let original_remaining_weight = remaining_weight.clone();
		let db_weight = T::DbWeight::get();
		Bridges::<T, I>::mutate(bridge_id, |bridge| {
			// if there's no such bridge, we don't need to service it
			let mut mut_bridge =
				bridge.take().ok_or_else(|| T::WeightInfo::bridge_read_weight())?;
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
		let mut used_weight = T::WeightInfo::suspended_message_read_weight()
			.saturating_add(T::DbWeight::get().writes(1));

		// let's read remove message from the runtime storage
		let message = SuspendedMessages::<T, I>::take(bridge_id, message_index);
		let message_len = message.as_ref().map(|message| message.len()).unwrap_or(0);

		// the `HARD_SUSPENDED_MESSAGE_SIZE_LIMIT` is quite large, so we want to decrease
		// the size of PoV if actual message is smaller than this limit
		let message_len_len = Compact::compact_len(&(message_len as u32)) as u32;
		used_weight.saturating_reduce(Weight::from_parts(
			0,
			HARD_SUSPENDED_MESSAGE_SIZE_LIMIT
				.saturating_sub((message_len as u32).saturating_add(message_len_len)) as u64,
		));

		// if message is missing from the storage (meaning something is corrupted), we are
		// not doing anything else
		let message = if let Some(message) = message { message } else { return used_weight };

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

	#[test]
	fn relieving_bridges_are_not_serviced_if_weight_is_not_enough() {}

	#[test]
	fn relieving_bridges_are_deleted_if_empty() {}

	#[test]
	fn used_weight_when_no_relieved_bridges() {}

	#[test]
	fn iteration_restarts_when_current_index_is_invalid() {}

	#[test]
	fn bridge_is_removed_from_relieving_if_it_is_missing_from_the_storage() {}

	#[test]
	fn used_weight_when_message_is_missing_from_the_storage() {}

	#[test]
	fn used_weight_when_we_fail_to_decode_ticket() {}

	#[test]
	fn relieving_bridges_are_serviced() {}
}
