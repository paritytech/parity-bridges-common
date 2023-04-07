// Copyright 2021 Parity Technologies (UK) Ltd.
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

use crate::messages::{
	source::FromBridgedChainMessagesDeliveryProof, target::FromBridgedChainMessagesProof,
};
use bp_messages::{LaneId, MessageNonce, UnrewardedRelayer};
use frame_support::{
	dispatch::CallableCallFor,
	traits::{Get, IsSubType},
	RuntimeDebug,
};
use pallet_bridge_messages::{Config, Pallet};
use sp_runtime::transaction_validity::TransactionValidity;
use sp_std::{collections::vec_deque::VecDeque, ops::RangeInclusive};

/// Generic info about a messages delivery/confirmation proof.
#[derive(PartialEq, RuntimeDebug)]
pub struct BaseMessagesProofInfo {
	/// Message lane, used by the call.
	pub lane_id: LaneId,
	/// Nonces of messages, included in the call.
	///
	/// For delivery transaction, it is nonces of bundled messages. For confirmation
	/// transaction, it is nonces that are to be confirmed during the call.
	pub bundled_range: RangeInclusive<MessageNonce>,
	/// Nonce of the best message, stored by this chain before the call is dispatched.
	///
	/// For delivery transaction, it is the nonce of best delivered message before the call.
	/// For confirmation transaction, it is the nonce of best confirmed message before the call.
	pub best_stored_nonce: MessageNonce,
	/// For message delivery transactions, the number of free entries in the unrewarded relayers
	/// vector before call is dispatched.
	pub stored_free_unrewarded_entries: Option<MessageNonce>,
}

impl BaseMessagesProofInfo {
	/// Returns true if `bundled_range` cannot be directly appended to the `best_stored_nonce`
	/// or if the `bundled_range` is empty (unless we're confirming rewards when unrewarded
	/// relayers vector is full).
	fn is_obsolete(&self) -> bool {
		// we allow delivery transactions without messages only when no free entries
		// left in unrewarded relayers vector
		if self.bundled_range.is_empty() {
			let empty_transactions_allowed = self.stored_free_unrewarded_entries == Some(0);
			if empty_transactions_allowed {
				return false
			}

			return true
		}

		// otherwise we require bundled messages to continue stored range
		*self.bundled_range.start() != self.best_stored_nonce + 1
	}
}

/// Info about a `ReceiveMessagesProof` call which tries to update a single lane.
#[derive(PartialEq, RuntimeDebug)]
pub struct ReceiveMessagesProofInfo(pub BaseMessagesProofInfo);

/// Info about a `ReceiveMessagesDeliveryProof` call which tries to update a single lane.
#[derive(PartialEq, RuntimeDebug)]
pub struct ReceiveMessagesDeliveryProofInfo(pub BaseMessagesProofInfo);

/// Info about a `ReceiveMessagesProof` or a `ReceiveMessagesDeliveryProof` call
/// which tries to update a single lane.
#[derive(PartialEq, RuntimeDebug)]
pub enum CallInfo {
	ReceiveMessagesProof(ReceiveMessagesProofInfo),
	ReceiveMessagesDeliveryProof(ReceiveMessagesDeliveryProofInfo),
}

/// Helper struct that provides methods for working with a call supported by `CallInfo`.
pub struct CallHelper<T: Config<I>, I: 'static> {
	pub _phantom_data: sp_std::marker::PhantomData<(T, I)>,
}

impl<T: Config<I>, I: 'static> CallHelper<T, I> {
	/// Returns true if:
	///
	/// - call is `receive_messages_proof` and all messages have been delivered;
	///
	/// - call is `receive_messages_delivery_proof` and all messages confirmations have been
	///   received.
	pub fn was_successful(info: &CallInfo) -> bool {
		match info {
			CallInfo::ReceiveMessagesProof(info) => {
				let inbound_lane_data =
					pallet_bridge_messages::InboundLanes::<T, I>::get(info.0.lane_id);
				if info.0.bundled_range.is_empty() {
					if let Some(stored_free_unrewarded_entries) =
						info.0.stored_free_unrewarded_entries
					{
						return free_unrewarded_entries::<T, I>(&inbound_lane_data.relayers) >
							stored_free_unrewarded_entries
					}
				}

				inbound_lane_data.last_delivered_nonce() == *info.0.bundled_range.end()
			},
			CallInfo::ReceiveMessagesDeliveryProof(info) => {
				let outbound_lane_data =
					pallet_bridge_messages::OutboundLanes::<T, I>::get(info.0.lane_id);
				outbound_lane_data.latest_received_nonce == *info.0.bundled_range.end()
			},
		}
	}
}

/// Trait representing a call that is a sub type of `pallet_bridge_messages::Call`.
pub trait MessagesCallSubType<T: Config<I, RuntimeCall = Self>, I: 'static>:
	IsSubType<CallableCallFor<Pallet<T, I>, T>>
{
	/// Create a new instance of `ReceiveMessagesProofInfo` from a `ReceiveMessagesProof` call.
	fn receive_messages_proof_info(&self) -> Option<ReceiveMessagesProofInfo>;

	/// Create a new instance of `ReceiveMessagesDeliveryProofInfo` from
	/// a `ReceiveMessagesDeliveryProof` call.
	fn receive_messages_delivery_proof_info(&self) -> Option<ReceiveMessagesDeliveryProofInfo>;

	/// Create a new instance of `CallInfo` from a `ReceiveMessagesProof`
	/// or a `ReceiveMessagesDeliveryProof` call.
	fn call_info(&self) -> Option<CallInfo>;

	/// Create a new instance of `CallInfo` from a `ReceiveMessagesProof`
	/// or a `ReceiveMessagesDeliveryProof` call, if the call is for the provided lane.
	fn call_info_for(&self, lane_id: LaneId) -> Option<CallInfo>;

	/// Check that a `ReceiveMessagesProof` or a `ReceiveMessagesDeliveryProof` call is trying
	/// to deliver/confirm at least some messages that are better than the ones we know of.
	fn check_obsolete_call(&self) -> TransactionValidity;
}

impl<
		BridgedHeaderHash,
		SourceHeaderChain: bp_messages::target_chain::SourceHeaderChain<
			MessagesProof = FromBridgedChainMessagesProof<BridgedHeaderHash>,
		>,
		TargetHeaderChain: bp_messages::source_chain::TargetHeaderChain<
			<T as Config<I>>::OutboundPayload,
			<T as frame_system::Config>::AccountId,
			MessagesDeliveryProof = FromBridgedChainMessagesDeliveryProof<BridgedHeaderHash>,
		>,
		Call: IsSubType<CallableCallFor<Pallet<T, I>, T>>,
		T: frame_system::Config<RuntimeCall = Call>
			+ Config<I, SourceHeaderChain = SourceHeaderChain, TargetHeaderChain = TargetHeaderChain>,
		I: 'static,
	> MessagesCallSubType<T, I> for T::RuntimeCall
{
	fn receive_messages_proof_info(&self) -> Option<ReceiveMessagesProofInfo> {
		if let Some(pallet_bridge_messages::Call::<T, I>::receive_messages_proof {
			ref proof,
			..
		}) = self.is_sub_type()
		{
			let inbound_lane_data = pallet_bridge_messages::InboundLanes::<T, I>::get(proof.lane);

			return Some(ReceiveMessagesProofInfo(BaseMessagesProofInfo {
				lane_id: proof.lane,
				// we want all messages in this range to be new for us. Otherwise transaction will
				// be considered obsolete.
				bundled_range: proof.nonces_start..=proof.nonces_end,
				best_stored_nonce: inbound_lane_data.last_delivered_nonce(),
				stored_free_unrewarded_entries: Some(free_unrewarded_entries::<T, I>(
					&inbound_lane_data.relayers,
				)),
			}))
		}

		None
	}

	fn receive_messages_delivery_proof_info(&self) -> Option<ReceiveMessagesDeliveryProofInfo> {
		if let Some(pallet_bridge_messages::Call::<T, I>::receive_messages_delivery_proof {
			ref proof,
			ref relayers_state,
			..
		}) = self.is_sub_type()
		{
			let outbound_lane_data = pallet_bridge_messages::OutboundLanes::<T, I>::get(proof.lane);

			return Some(ReceiveMessagesDeliveryProofInfo(BaseMessagesProofInfo {
				lane_id: proof.lane,
				// there's a time frame between message delivery, message confirmation and reward
				// confirmation. Because of that, we can't assume that our state has been confirmed
				// to the bridged chain. So we are accepting any proof that brings new
				// confirmations.
				bundled_range: outbound_lane_data.latest_received_nonce + 1..=
					relayers_state.last_delivered_nonce,
				best_stored_nonce: outbound_lane_data.latest_received_nonce,
				stored_free_unrewarded_entries: None,
			}))
		}

		None
	}

	fn call_info(&self) -> Option<CallInfo> {
		if let Some(info) = self.receive_messages_proof_info() {
			return Some(CallInfo::ReceiveMessagesProof(info))
		}

		if let Some(info) = self.receive_messages_delivery_proof_info() {
			return Some(CallInfo::ReceiveMessagesDeliveryProof(info))
		}

		None
	}

	fn call_info_for(&self, lane_id: LaneId) -> Option<CallInfo> {
		self.call_info().filter(|info| {
			let actual_lane_id = match info {
				CallInfo::ReceiveMessagesProof(info) => info.0.lane_id,
				CallInfo::ReceiveMessagesDeliveryProof(info) => info.0.lane_id,
			};
			actual_lane_id == lane_id
		})
	}

	fn check_obsolete_call(&self) -> TransactionValidity {
		match self.call_info() {
			Some(CallInfo::ReceiveMessagesProof(proof_info)) if proof_info.0.is_obsolete() => {
				log::trace!(
					target: pallet_bridge_messages::LOG_TARGET,
					"Rejecting obsolete messages delivery transaction: {:?}",
					proof_info
				);

				return sp_runtime::transaction_validity::InvalidTransaction::Stale.into()
			},
			Some(CallInfo::ReceiveMessagesDeliveryProof(proof_info))
				if proof_info.0.is_obsolete() =>
			{
				log::trace!(
					target: pallet_bridge_messages::LOG_TARGET,
					"Rejecting obsolete messages confirmation transaction: {:?}",
					proof_info,
				);

				return sp_runtime::transaction_validity::InvalidTransaction::Stale.into()
			},
			_ => {},
		}

		Ok(sp_runtime::transaction_validity::ValidTransaction::default())
	}
}

/// Returns number of free entries in the unrewarded relayers vector.
fn free_unrewarded_entries<T: Config<I>, I: 'static>(
	relayers: &VecDeque<UnrewardedRelayer<T::InboundRelayer>>,
) -> MessageNonce {
	let max_entries = T::MaxUnrewardedRelayerEntriesAtInboundLane::get();
	max_entries.saturating_sub(relayers.len() as MessageNonce)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{
		messages::{
			source::FromBridgedChainMessagesDeliveryProof, target::FromBridgedChainMessagesProof,
		},
		messages_call_ext::MessagesCallSubType,
		mock::{TestRuntime, ThisChainRuntimeCall},
	};
	use bp_messages::{DeliveredMessages, UnrewardedRelayersState};
	use sp_std::ops::RangeInclusive;

	fn fill_unrewarded_relayers() {
		let mut inbound_lane_state =
			pallet_bridge_messages::InboundLanes::<TestRuntime>::get(LaneId([0, 0, 0, 0]));
		for _ in 0..<TestRuntime as Config>::MaxUnrewardedRelayerEntriesAtInboundLane::get() {
			inbound_lane_state.relayers.push_back(UnrewardedRelayer {
				relayer: Default::default(),
				messages: DeliveredMessages { begin: 0, end: 0 },
			});
		}
		pallet_bridge_messages::InboundLanes::<TestRuntime>::insert(
			LaneId([0, 0, 0, 0]),
			inbound_lane_state,
		);
	}

	fn deliver_message_10() {
		pallet_bridge_messages::InboundLanes::<TestRuntime>::insert(
			LaneId([0, 0, 0, 0]),
			bp_messages::InboundLaneData { relayers: Default::default(), last_confirmed_nonce: 10 },
		);
	}

	fn validate_message_delivery(
		nonces_start: bp_messages::MessageNonce,
		nonces_end: bp_messages::MessageNonce,
	) -> bool {
		ThisChainRuntimeCall::BridgeMessages(
			pallet_bridge_messages::Call::<TestRuntime, ()>::receive_messages_proof {
				relayer_id_at_bridged_chain: 42,
				messages_count: nonces_end.checked_sub(nonces_start).map(|x| x + 1).unwrap_or(0)
					as u32,
				dispatch_weight: frame_support::weights::Weight::zero(),
				proof: FromBridgedChainMessagesProof {
					bridged_header_hash: Default::default(),
					storage_proof: vec![],
					lane: LaneId([0, 0, 0, 0]),
					nonces_start,
					nonces_end,
				},
			},
		)
		.check_obsolete_call()
		.is_ok()
	}

	#[test]
	fn extension_rejects_obsolete_messages() {
		sp_io::TestExternalities::new(Default::default()).execute_with(|| {
			// when current best delivered is message#10 and we're trying to deliver messages 8..=9
			// => tx is rejected
			deliver_message_10();
			assert!(!validate_message_delivery(8, 9));
		});
	}

	#[test]
	fn extension_rejects_same_message() {
		sp_io::TestExternalities::new(Default::default()).execute_with(|| {
			// when current best delivered is message#10 and we're trying to import messages 10..=10
			// => tx is rejected
			deliver_message_10();
			assert!(!validate_message_delivery(8, 10));
		});
	}

	#[test]
	fn extension_rejects_call_with_some_obsolete_messages() {
		sp_io::TestExternalities::new(Default::default()).execute_with(|| {
			// when current best delivered is message#10 and we're trying to deliver messages
			// 10..=15 => tx is rejected
			deliver_message_10();
			assert!(!validate_message_delivery(10, 15));
		});
	}

	#[test]
	fn extension_rejects_call_with_future_messages() {
		sp_io::TestExternalities::new(Default::default()).execute_with(|| {
			// when current best delivered is message#10 and we're trying to deliver messages
			// 13..=15 => tx is rejected
			deliver_message_10();
			assert!(!validate_message_delivery(13, 15));
		});
	}

	#[test]
	fn extension_rejects_empty_delivery_with_rewards_confirmations_if_there_are_free_unrewarded_entries(
	) {
		sp_io::TestExternalities::new(Default::default()).execute_with(|| {
			deliver_message_10();
			assert!(!validate_message_delivery(10, 9));
		});
	}

	#[test]
	fn extension_accepts_empty_delivery_with_rewards_confirmations_if_there_are_no_free_unrewarded_entries(
	) {
		sp_io::TestExternalities::new(Default::default()).execute_with(|| {
			deliver_message_10();
			fill_unrewarded_relayers();
			assert!(validate_message_delivery(10, 9));
		});
	}

	#[test]
	fn extension_accepts_new_messages() {
		sp_io::TestExternalities::new(Default::default()).execute_with(|| {
			// when current best delivered is message#10 and we're trying to deliver message 11..=15
			// => tx is accepted
			deliver_message_10();
			assert!(validate_message_delivery(11, 15));
		});
	}

	fn confirm_message_10() {
		pallet_bridge_messages::OutboundLanes::<TestRuntime>::insert(
			LaneId([0, 0, 0, 0]),
			bp_messages::OutboundLaneData {
				oldest_unpruned_nonce: 0,
				latest_received_nonce: 10,
				latest_generated_nonce: 10,
			},
		);
	}

	fn validate_message_confirmation(last_delivered_nonce: bp_messages::MessageNonce) -> bool {
		ThisChainRuntimeCall::BridgeMessages(
			pallet_bridge_messages::Call::<TestRuntime>::receive_messages_delivery_proof {
				proof: FromBridgedChainMessagesDeliveryProof {
					bridged_header_hash: Default::default(),
					storage_proof: Vec::new(),
					lane: LaneId([0, 0, 0, 0]),
				},
				relayers_state: UnrewardedRelayersState {
					last_delivered_nonce,
					..Default::default()
				},
			},
		)
		.check_obsolete_call()
		.is_ok()
	}

	#[test]
	fn extension_rejects_obsolete_confirmations() {
		sp_io::TestExternalities::new(Default::default()).execute_with(|| {
			// when current best confirmed is message#10 and we're trying to confirm message#5 => tx
			// is rejected
			confirm_message_10();
			assert!(!validate_message_confirmation(5));
		});
	}

	#[test]
	fn extension_rejects_same_confirmation() {
		sp_io::TestExternalities::new(Default::default()).execute_with(|| {
			// when current best confirmed is message#10 and we're trying to confirm message#10 =>
			// tx is rejected
			confirm_message_10();
			assert!(!validate_message_confirmation(10));
		});
	}

	#[test]
	fn extension_rejects_empty_confirmation_even_if_there_are_no_free_unrewarded_entries() {
		sp_io::TestExternalities::new(Default::default()).execute_with(|| {
			confirm_message_10();
			fill_unrewarded_relayers();
			assert!(!validate_message_confirmation(10));
		});
	}

	#[test]
	fn extension_accepts_new_confirmation() {
		sp_io::TestExternalities::new(Default::default()).execute_with(|| {
			// when current best confirmed is message#10 and we're trying to confirm message#15 =>
			// tx is accepted
			confirm_message_10();
			assert!(validate_message_confirmation(15));
		});
	}

	fn was_message_delivery_successful(bundled_range: RangeInclusive<MessageNonce>) -> bool {
		CallHelper::<TestRuntime, ()>::was_successful(&CallInfo::ReceiveMessagesProof(
			ReceiveMessagesProofInfo(BaseMessagesProofInfo {
				lane_id: LaneId([0, 0, 0, 0]),
				bundled_range,
				best_stored_nonce: 0, // doesn't matter for `was_successful`
				stored_free_unrewarded_entries: Some(0),
			}),
		))
	}

	#[test]
	fn was_successful_returns_false_for_failed_reward_confirmation_transaction() {
		sp_io::TestExternalities::new(Default::default()).execute_with(|| {
			fill_unrewarded_relayers();
			assert!(!was_message_delivery_successful(10..=9));
		});
	}

	#[test]
	fn was_successful_returns_true_for_successful_reward_confirmation_transaction() {
		sp_io::TestExternalities::new(Default::default()).execute_with(|| {
			assert!(was_message_delivery_successful(10..=9));
		});
	}

	#[test]
	fn was_successful_returns_false_for_failed_delivery() {
		sp_io::TestExternalities::new(Default::default()).execute_with(|| {
			deliver_message_10();
			assert!(!was_message_delivery_successful(10..=12));
		});
	}

	#[test]
	fn was_successful_returns_false_for_partially_successful_delivery() {
		sp_io::TestExternalities::new(Default::default()).execute_with(|| {
			deliver_message_10();
			assert!(!was_message_delivery_successful(9..=12));
		});
	}

	#[test]
	fn was_successful_returns_true_for_successful_delivery() {
		sp_io::TestExternalities::new(Default::default()).execute_with(|| {
			deliver_message_10();
			assert!(was_message_delivery_successful(9..=10));
		});
	}

	fn was_message_confirmation_successful(bundled_range: RangeInclusive<MessageNonce>) -> bool {
		CallHelper::<TestRuntime, ()>::was_successful(&CallInfo::ReceiveMessagesDeliveryProof(
			ReceiveMessagesDeliveryProofInfo(BaseMessagesProofInfo {
				lane_id: LaneId([0, 0, 0, 0]),
				bundled_range,
				best_stored_nonce: 0, // doesn't matter for `was_successful`
				stored_free_unrewarded_entries: None,
			}),
		))
	}

	#[test]
	fn was_successful_returns_false_for_failed_confirmation() {
		sp_io::TestExternalities::new(Default::default()).execute_with(|| {
			confirm_message_10();
			assert!(!was_message_confirmation_successful(10..=12));
		});
	}

	#[test]
	fn was_successful_returns_false_for_partially_successful_confirmation() {
		sp_io::TestExternalities::new(Default::default()).execute_with(|| {
			confirm_message_10();
			assert!(!was_message_confirmation_successful(9..=12));
		});
	}

	#[test]
	fn was_successful_returns_true_for_successful_confirmation() {
		sp_io::TestExternalities::new(Default::default()).execute_with(|| {
			confirm_message_10();
			assert!(was_message_confirmation_successful(9..=10));
		});
	}
}
