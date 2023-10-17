// Copyright (C) Parity Technologies (UK) Ltd.
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

//! Bridge transaction priority calculator.
//!
//! We want to prioritize message delivery transactions with more messages over
//! transactions with less messages. That's because we reject delivery transactions
//! if it contains already delivered message. And if some transaction delivers
//! single message with nonce `N`, then the transaction with nonces `N..=N+100` will
//! be rejected. This can lower bridge throughput down to one message per block.

use crate::{Config as RelayersConfig, Pallet as RelayersPallet};

use bp_messages::{LaneId, MessageNonce};
use frame_support::traits::Get;
use frame_system::{pallet_prelude::BlockNumberFor, Pallet as SystemPallet};
use sp_runtime::{
	traits::{One, Zero},
	transaction_validity::TransactionPriority,
	Saturating,
};

// reexport everything from `integrity_tests` module
pub use integrity_tests::*;

/// Compute total priority boost for message delivery transaction.
pub fn compute_priority_boost<R: RelayersConfig>(
	lane_id: LaneId,
	messages: MessageNonce,
	relayer: &R::AccountId,
) -> TransactionPriority
where
	usize: TryFrom<BlockNumberFor<R>>,
{
	compute_per_message_priority_boost::<R::PriorityBoostPerMessage>(messages)
		.saturating_add(compute_per_lane_priority_boost::<R>(lane_id, relayer))
}

/// Compute priority boost for message delivery transaction, that depends on number
/// of bundled messages.
fn compute_per_message_priority_boost<PriorityBoostPerMessage: Get<TransactionPriority>>(
	messages: MessageNonce,
) -> TransactionPriority {
	// we don't want any boost for transaction with single message => minus one
	PriorityBoostPerMessage::get().saturating_mul(messages.saturating_sub(1))
}

/// Compute priority boost for message delivery transaction, that depends on
/// the set of lane relayers and current slot.
fn compute_per_lane_priority_boost<R: RelayersConfig>(
	lane_id: LaneId,
	relayer: &R::AccountId,
) -> TransactionPriority
where
	usize: TryFrom<BlockNumberFor<R>>,
{
	// if there are no relayers, explicitly registered at this lane, noone gets additional
	// priority boost
	let lane_relayers = RelayersPallet::<R>::active_lane_relayers(&lane_id);
	let active_lane_relayers = lane_relayers.relayers();
	let lane_relayers_len: BlockNumberFor<R> = (active_lane_relayers.len() as u32).into();
	if lane_relayers_len.is_zero() {
		return 0
	}

	// we can't deal with slots shorter than 1 block
	let slot_length: BlockNumberFor<R> = R::SlotLength::get();
	if slot_length < One::one() {
		return 0
	}

	// let's compute current slot number
	let current_block_number = SystemPallet::<R>::block_number();
	let slot = current_block_number.saturating_sub(*lane_relayers.enacted_at()) / slot_length;

	// and then get the relayer for that slot
	let slot_relayer = match usize::try_from(slot % lane_relayers_len) {
		Ok(slot_relayer_index) => &active_lane_relayers[slot_relayer_index],
		Err(_) => return 0,
	};

	// if message delivery transaction is submitted by the relayer, assigned to the current
	// slot, let's boost the transaction priority
	if relayer != slot_relayer.relayer() {
		return 0
	}

	R::PriorityBoostForActiveLaneRelayer::get()
}

#[cfg(not(feature = "integrity-test"))]
mod integrity_tests {}

#[cfg(feature = "integrity-test")]
mod integrity_tests {
	use super::compute_per_message_priority_boost;
	use bp_messages::ChainWithMessages;

	use bp_messages::MessageNonce;
	use bp_runtime::PreComputedSize;
	use frame_support::{
		dispatch::{DispatchClass, DispatchInfo, Pays, PostDispatchInfo},
		traits::Get,
	};
	use pallet_bridge_messages::WeightInfoExt;
	use pallet_transaction_payment::OnChargeTransaction;
	use sp_runtime::{
		traits::{Dispatchable, UniqueSaturatedInto, Zero},
		transaction_validity::TransactionPriority,
		FixedPointOperand, SaturatedConversion, Saturating,
	};

	type BalanceOf<T> =
		<<T as pallet_transaction_payment::Config>::OnChargeTransaction as OnChargeTransaction<
			T,
		>>::Balance;

	/// Ensures that the value of `PriorityBoostPerMessage` matches the value of
	/// `tip_boost_per_message`.
	///
	/// We want two transactions, `TX1` with `N` messages and `TX2` with `N+1` messages, have almost
	/// the same priority if we'll add `tip_boost_per_message` tip to the `TX1`. We want to be sure
	/// that if we add plain `PriorityBoostPerMessage` priority to `TX1`, the priority will be close
	/// to `TX2` as well.
	pub fn ensure_priority_boost_is_sane<Runtime, MessagesInstance, PriorityBoostPerMessage>(
		tip_boost_per_message: BalanceOf<Runtime>,
	) where
		Runtime:
			pallet_transaction_payment::Config + pallet_bridge_messages::Config<MessagesInstance>,
		MessagesInstance: 'static,
		PriorityBoostPerMessage: Get<TransactionPriority>,
		Runtime::RuntimeCall: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
		BalanceOf<Runtime>: Send + Sync + FixedPointOperand,
	{
		let priority_boost_per_message = PriorityBoostPerMessage::get();
		let maximal_messages_in_delivery_transaction =
			Runtime::BridgedChain::MAX_UNCONFIRMED_MESSAGES_IN_CONFIRMATION_TX;
		for messages in 1..=maximal_messages_in_delivery_transaction {
			let base_priority = estimate_message_delivery_transaction_priority::<
				Runtime,
				MessagesInstance,
			>(messages, Zero::zero());
			let priority_boost =
				compute_per_message_priority_boost::<PriorityBoostPerMessage>(messages);
			let priority_with_boost = base_priority + priority_boost;

			let tip = tip_boost_per_message.saturating_mul((messages - 1).unique_saturated_into());
			let priority_with_tip =
				estimate_message_delivery_transaction_priority::<Runtime, MessagesInstance>(1, tip);

			const ERROR_MARGIN: TransactionPriority = 5; // 5%
			if priority_with_boost.abs_diff(priority_with_tip).saturating_mul(100) /
				priority_with_tip >
				ERROR_MARGIN
			{
				panic!(
					"The PriorityBoostPerMessage value ({}) must be fixed to: {}",
					priority_boost_per_message,
					compute_per_message_priority_boost_per_message::<Runtime, MessagesInstance>(
						tip_boost_per_message
					),
				);
			}
		}
	}

	/// Compute priority boost that we give to message delivery transaction for additional message.
	#[cfg(feature = "integrity-test")]
	fn compute_per_message_priority_boost_per_message<Runtime, MessagesInstance>(
		tip_boost_per_message: BalanceOf<Runtime>,
	) -> TransactionPriority
	where
		Runtime:
			pallet_transaction_payment::Config + pallet_bridge_messages::Config<MessagesInstance>,
		MessagesInstance: 'static,
		Runtime::RuntimeCall: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
		BalanceOf<Runtime>: Send + Sync + FixedPointOperand,
	{
		// esimate priority of transaction that delivers one message and has large tip
		let maximal_messages_in_delivery_transaction =
			Runtime::BridgedChain::MAX_UNCONFIRMED_MESSAGES_IN_CONFIRMATION_TX;
		let small_with_tip_priority =
			estimate_message_delivery_transaction_priority::<Runtime, MessagesInstance>(
				1,
				tip_boost_per_message
					.saturating_mul(maximal_messages_in_delivery_transaction.saturated_into()),
			);
		// estimate priority of transaction that delivers maximal number of messages, but has no tip
		let large_without_tip_priority = estimate_message_delivery_transaction_priority::<
			Runtime,
			MessagesInstance,
		>(maximal_messages_in_delivery_transaction, Zero::zero());

		small_with_tip_priority
			.saturating_sub(large_without_tip_priority)
			.saturating_div(maximal_messages_in_delivery_transaction - 1)
	}

	/// Estimate message delivery transaction priority.
	#[cfg(feature = "integrity-test")]
	fn estimate_message_delivery_transaction_priority<Runtime, MessagesInstance>(
		messages: MessageNonce,
		tip: BalanceOf<Runtime>,
	) -> TransactionPriority
	where
		Runtime:
			pallet_transaction_payment::Config + pallet_bridge_messages::Config<MessagesInstance>,
		MessagesInstance: 'static,
		Runtime::RuntimeCall: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
		BalanceOf<Runtime>: Send + Sync + FixedPointOperand,
	{
		// just an estimation of extra transaction bytes that are added to every transaction
		// (including signature, signed extensions extra and etc + in our case it includes
		// all call arguments extept the proof itself)
		let base_tx_size = 512;
		// let's say we are relaying similar small messages and for every message we add more trie
		// nodes to the proof (x0.5 because we expect some nodes to be reused)
		let estimated_message_size = 512;
		// let's say all our messages have the same dispatch weight
		let estimated_message_dispatch_weight =
			Runtime::WeightInfo::message_dispatch_weight(estimated_message_size);
		// messages proof argument size is (for every message) messages size + some additional
		// trie nodes. Some of them are reused by different messages, so let's take 2/3 of default
		// "overhead" constant
		let messages_proof_size = Runtime::WeightInfo::expected_extra_storage_proof_size()
			.saturating_mul(2)
			.saturating_div(3)
			.saturating_add(estimated_message_size)
			.saturating_mul(messages as _);

		// finally we are able to estimate transaction size and weight
		let transaction_size = base_tx_size.saturating_add(messages_proof_size);
		let transaction_weight = Runtime::WeightInfo::receive_messages_proof_weight(
			&PreComputedSize(transaction_size as _),
			messages as _,
			estimated_message_dispatch_weight.saturating_mul(messages),
		);

		pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::get_priority(
			&DispatchInfo {
				weight: transaction_weight,
				class: DispatchClass::Normal,
				pays_fee: Pays::Yes,
			},
			transaction_size as _,
			tip,
			Zero::zero(),
		)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{mock::*, ActiveLaneRelayers};
	use bp_relayers::{ActiveLaneRelayersSet, NextLaneRelayersSet};
	use sp_runtime::traits::ConstU32;

	#[test]
	fn compute_per_lane_priority_boost_works() {
		run_test(|| {
			// insert 3 relayers to the queue
			let lane_id = LaneId::new(1, 2);
			let relayer1 = 100;
			let relayer2 = 200;
			let relayer3 = 300;
			let mut next_set: NextLaneRelayersSet<_, _, ConstU32<3>> =
				NextLaneRelayersSet::empty(5);
			assert!(next_set.try_push(relayer1, 0));
			assert!(next_set.try_push(relayer2, 0));
			assert!(next_set.try_push(relayer3, 0));
			let mut active_set = ActiveLaneRelayersSet::default();
			active_set.activate_next_set(7, next_set, |_| true);
			ActiveLaneRelayers::<TestRuntime>::insert(lane_id, active_set);

			// at blocks 7..=7+SlotLength relayer1 gets the boost
			System::set_block_number(6);
			for _ in 7..SlotLength::get() + 7 {
				System::set_block_number(System::block_number() + 1);
				assert_eq!(
					compute_per_lane_priority_boost::<TestRuntime>(lane_id, &relayer1),
					PriorityBoostForActiveLaneRelayer::get(),
				);
				assert_eq!(compute_per_lane_priority_boost::<TestRuntime>(lane_id, &relayer2), 0,);
				assert_eq!(compute_per_lane_priority_boost::<TestRuntime>(lane_id, &relayer3), 0,);
			}

			// at next slot, relayer2 gets the boost
			for _ in 1..=SlotLength::get() {
				System::set_block_number(System::block_number() + 1);
				assert_eq!(compute_per_lane_priority_boost::<TestRuntime>(lane_id, &relayer1), 0,);
				assert_eq!(
					compute_per_lane_priority_boost::<TestRuntime>(lane_id, &relayer2),
					PriorityBoostForActiveLaneRelayer::get(),
				);
				assert_eq!(compute_per_lane_priority_boost::<TestRuntime>(lane_id, &relayer3), 0,);
			}

			// at next slot, relayer3 gets the boost
			for _ in 1..=SlotLength::get() {
				System::set_block_number(System::block_number() + 1);
				assert_eq!(compute_per_lane_priority_boost::<TestRuntime>(lane_id, &relayer1), 0,);
				assert_eq!(compute_per_lane_priority_boost::<TestRuntime>(lane_id, &relayer2), 0,);
				assert_eq!(
					compute_per_lane_priority_boost::<TestRuntime>(lane_id, &relayer3),
					PriorityBoostForActiveLaneRelayer::get(),
				);
			}
		});
	}
}
