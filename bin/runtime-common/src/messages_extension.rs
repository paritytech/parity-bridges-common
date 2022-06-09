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

/// Declares a runtime-specific `BridgeRejectObsoleteMessages` and
/// `BridgeRejectObsoleteMessageConfirmations` signed extensions.
///
/// ## Example
///
/// ```nocompile
/// bridge_runtime_common::declare_bridge_reject_obsolete_messages!{
///     Runtime,
///     Call::BridgeRialtoMessages => WithRialtoMessagesInstance,
///     Call::BridgeRialtoParachainMessages => WithRialtoParachainMessagesInstance,
/// }
/// ```
///
/// The goal of this extension is to avoid "mining" messages delivery and delivery confirmation
/// transactions, that are delivering outdated messages/confirmations. Without that extension,
/// even honest relayers may lose their funds if there are multiple relays running and submitting
/// the same messages/confirmations.
#[macro_export]
macro_rules! declare_bridge_reject_obsolete_messages {
	($runtime:ident, $($call:path => $instance:ty),*) => {
		/// Transaction-with-obsolete-messages check that will reject transaction if
		/// it submits obsolete messages/confirmations.
		#[derive(Clone, codec::Decode, codec::Encode, Eq, PartialEq, frame_support::RuntimeDebug, scale_info::TypeInfo)]
		pub struct BridgeRejectObsoleteMessages;

		impl sp_runtime::traits::SignedExtension for BridgeRejectObsoleteMessages {
			const IDENTIFIER: &'static str = "BridgeRejectObsoleteMessages";
			type AccountId = <$runtime as frame_system::Config>::AccountId;
			type Call = <$runtime as frame_system::Config>::Call;
			type AdditionalSigned = ();
			type Pre = ();

			fn additional_signed(&self) -> sp_std::result::Result<
				(),
				sp_runtime::transaction_validity::TransactionValidityError,
			> {
				Ok(())
			}

			fn validate(
				&self,
				_who: &Self::AccountId,
				call: &Self::Call,
				_info: &sp_runtime::traits::DispatchInfoOf<Self::Call>,
				_len: usize,
			) -> sp_runtime::transaction_validity::TransactionValidity {
				match *call {
					$(
						$call(pallet_bridge_messages::Call::<$runtime, $instance>::receive_messages_proof {
							ref proof,
							..
						}) => {
							let nonces_end = proof.nonces_end;

							let inbound_lane_data = pallet_bridge_messages::InboundLanes::<$runtime, $instance>::get(&proof.lane);
							if proof.nonces_end <= inbound_lane_data.last_delivered_nonce() {
								return sp_runtime::transaction_validity::InvalidTransaction::Stale.into();
							}

							Ok(sp_runtime::transaction_validity::ValidTransaction::default())
						},
						$call(pallet_bridge_messages::Call::<$runtime, $instance>::receive_messages_delivery_proof {
							ref proof,
							ref relayers_state,
							..
						}) => {
							let latest_delivered_nonce = relayers_state.last_delivered_nonce;

							let outbound_lane_data = pallet_bridge_messages::OutboundLanes::<$runtime, $instance>::get(&proof.lane);
							if latest_delivered_nonce <= outbound_lane_data.latest_received_nonce {
								return sp_runtime::transaction_validity::InvalidTransaction::Stale.into();
							}

							Ok(sp_runtime::transaction_validity::ValidTransaction::default())
						}
					)*
					_ => Ok(sp_runtime::transaction_validity::ValidTransaction::default()),
				}
			}

			fn pre_dispatch(
				self,
				who: &Self::AccountId,
				call: &Self::Call,
				info: &sp_runtime::traits::DispatchInfoOf<Self::Call>,
				len: usize,
			) -> Result<Self::Pre, sp_runtime::transaction_validity::TransactionValidityError> {
				self.validate(who, call, info, len).map(drop)
			}

			fn post_dispatch(
				_maybe_pre: Option<Self::Pre>,
				_info: &sp_runtime::traits::DispatchInfoOf<Self::Call>,
				_post_info: &sp_runtime::traits::PostDispatchInfoOf<Self::Call>,
				_len: usize,
				_result: &sp_runtime::DispatchResult,
			) -> Result<(), sp_runtime::transaction_validity::TransactionValidityError> {
				Ok(())
			}
		}
	};
}

// tests for this extension are in the Millau runtime code, since we don't have any runtime here
