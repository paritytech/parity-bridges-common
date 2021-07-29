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

//! Runtime module that allows token swap between two parties acting on different chains.
//!
//! The swap is made using message lanes between This (where `pallet-bridge-token-swap` pallet
//! is deployed) and some other Bridged chain. No other assumptions about the Bridged chain are
//! made, so we don't need it to have an instance of the `pallet-bridge-token-swap` pallet deployed.
//!
//! There are four accounts participating in the swap:
//!
//! 1) account of This chain that has signed the `create_swap` transaction and has balance on This chain.
//!    We'll be referring to this account as `source_account_at_this_chain`;
//! 2) account of the Bridged chain that is sending the `claim_swap` message from the Bridged to This chain.
//!    This account has balance on Bridged chain and is willing to swap these tokens to This chain tokens of
//!    the `source_account_at_this_chain`. We'll be referring to this account as `target_account_at_bridged_chain`;
//! 3) account of the Bridged chain that is indirectly controlled by the `source_account_at_this_chain`. We'll be
//!    referring this account as `source_account_at_bridged_chain`;
//! 4) account of This chain that is indirectly controlled by the `target_account_at_bridged_chain`. We'll be
//!    referring this account as `target_account_at_this_chain`.
//!
//! So the tokens swap is an intention of `source_account_at_this_chain` to swap his `source_balance_at_this_chain`
//! tokens to the `target_balance_at_bridged_chain` tokens owned by `target_account_at_bridged_chain`. The swap
//! process goes as follows:
//!
//! 1) the `source_account_at_this_chain` account submits the `create_swap` transaction on This chain;
//! 2) the tokens transfer message that would transfer `target_balance_at_bridged_chain` tokens from the
//!    `target_account_at_bridged_chain` to the `source_account_at_bridged_chain`, is sent over the bridge;
//! 3) when transfer message is delivered and dispatched, the pallet receives notification;
//! 4) if message has been successfully dispatched, the `target_account_at_bridged_chain` sends the message
//!    that would transfer `source_balance_at_this_chain` tokens to his `target_account_at_this_chain`
//!    account;
//! 5) if message dispatch has failed, the `source_account_at_this_chain` may submit the `cancel_swap`
//!    transaction and return his `source_balance_at_this_chain` back to his account.
//!
//! While swap is pending, the `source_balance_at_this_chain` tokens are owned by the special
//! temporary `swap_account_at_this_chain` account. It is destroyed upon swap completion.

use bp_messages::{
	source_chain::{MessagesBridge, OnDeliveryConfirmed},
	DeliveredMessages, LaneId, MessageNonce,
};
use bp_runtime::{messages::DispatchFeePayment, ChainId};
use bp_token_swap::{TokenSwap, TokenSwapType};
use codec::{Decode, Encode};
use frame_support::{
	fail,
	traits::{Currency, ExistenceRequirement},
	RuntimeDebug,
};
use sp_core::H256;
use sp_io::hashing::blake2_256;
use sp_runtime::traits::{Convert, Saturating};

#[cfg(test)]
mod mock;

/// Pending token swap state.
#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq)]
pub enum TokenSwapState {
	/// The swap has been started using the `start_claim` call, but we have no proof that it has
	/// happened at the Bridged chain.
	Started,
	/// Ths swap has happened at the Bridged chain and may be claimed by the Bridged chain party using
	/// the `claim_swap` call.
	Confirmed,
	/// The swap has failed at the Bridged chain and This chain party may cancel it using the
	/// `cancel_swap` call.
	Failed,
}

pub use pallet::*;

// comes from #[pallet::event]
#[allow(clippy::unused_unit)]
#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self, I>> + IsType<<Self as frame_system::Config>::Event>;

		/// Id of the bridge with the Bridged chain.
		type BridgeChainId: Get<ChainId>;
		/// The identifier of outbound message lane on This chain used to send token transfer
		/// messages to the Bridged chain.
		///
		/// It is highly recommended to use dedicated lane for every instance of token swap
		/// pallet. Messages delivery confirmation callback is implemented in the way that
		type OutboundMessageLaneId: Get<LaneId>;
		/// Messages bridge with Bridged chain.
		type MessagesBridge: MessagesBridge<
			Self::AccountId,
			<Self::ThisCurrency as Currency<Self::AccountId>>::Balance,
			MessagePayloadOf<Self, I>,
		>;
		/// Message delivery and dispatch fee for the tokens transfer message heading to the Bridged chain.
		type MessageDeliveryAndDispatchFee: Get<<Self::ThisCurrency as Currency<Self::AccountId>>::Balance>;

		/// This chain Currency used in the tokens swap.
		type ThisCurrency: Currency<Self::AccountId>;
		/// Converter from raw hash (derived from swap) to This chain account.
		type FromSwapToThisAccountIdConverter: Convert<H256, Self::AccountId>;

		/// Current `spec_version` of the Bridged chain.
		type BridgedChainSpecVersion: Get<u32>;
		/// Current weight of the transfer call at the Bridged chain.
		type BridgedChainTransferWeight: Get<Weight>;
		/// Tokens balance type at the Bridged chain.
		type BridgedBalance: Parameter;
		/// Account identifier type at the Bridged chain.
		type BridgedAccountId: Parameter;
		/// Account public key type at the Bridged chain.
		type BridgedAccountPublic: Parameter;
		/// Account signature type at the Bridged chain.
		type BridgedAccountSignature: Parameter;
		/// Converter from raw hash (derived from Bridged chain account) to This chain account.
		type FromBridgedToThisAccountIdConverter: Convert<H256, Self::AccountId>;
	}

	/// SCALE-encoded `Currency::transfer` call on the bridged chain.
	pub type RawBridgedTransferCall = Vec<u8>;
	/// Bridge message payload used by the pallet.
	pub type MessagePayloadOf<T, I> = bp_message_dispatch::MessagePayload<
		<T as frame_system::Config>::AccountId,
		<T as Config<I>>::BridgedAccountPublic,
		<T as Config<I>>::BridgedAccountSignature,
		RawBridgedTransferCall,
	>;
	/// Type of `TokenSwap` used by the pallet.
	pub type TokenSwapOf<T, I> = TokenSwap<
		BlockNumberFor<T>,
		<<T as Config<I>>::ThisCurrency as Currency<<T as frame_system::Config>::AccountId>>::Balance,
		<T as frame_system::Config>::AccountId,
		<T as Config<I>>::BridgedBalance,
		<T as Config<I>>::BridgedAccountId,
	>;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::hooks]
	impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I> {}

	#[pallet::call]
	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		/// Start token swap procedure.
		///
		/// The dispatch origin for this call must be exactly the `swap.source_account_at_this_chain` account.
		///
		/// Method arguments are:
		///
		/// - `swap` - token swap intention;
		/// - `target_public_at_bridged_chain` - the public key of the `swap.target_account_at_bridged_chain`
		///   account used to verify `bridged_currency_transfer_signature`;
		/// - `bridged_currency_transfer` - the SCALE-encoded tokens transfer call at the Bridged chain;
		/// - `bridged_currency_transfer_signature` - the signature of the `swap.target_account_at_bridged_chain`
		///   for the message returned by the `pallet_bridge_dispatch::account_ownership_digest()` function call.
		///
		/// The `source_account_at_this_chain` MUST have enough balance to cover both token swap and message
		/// transfer. Message fee may be estimated using corresponding `OutboundLaneApi` of This runtime.
		///
		/// **WARNING**: the submitter of this transaction is responsible for verifying:
		///
		/// 1) that the `bridged_currency_transfer` represents a valid token transfer call that transfers
		///    `swap.target_balance_at_bridged_chain` to his `source_account_at_bridged_chain` account;
		/// 2) that either the `source_account_at_bridged_chain` already exists, or the
		///    `swap.target_balance_at_bridged_chain` is above existential deposit of the Bridged chain;
		/// 3) the `target_public_at_bridged_chain` matches the `swap.target_account_at_bridged_chain`;
		/// 4) the `bridged_currency_transfer_signature` is valid and generated by the owner of the
		///    `target_public_at_bridged_chain` account.
		///
		/// Violating rule#1 will lead to losing your `source_balance_at_this_chain` tokens. Violating other
		/// rules will lead to losing message fees for this and other transactions + losing fees for message
		/// transfer.
		#[pallet::weight(0)]
		pub fn create_swap(
			origin: OriginFor<T>,
			swap: TokenSwapOf<T, I>,
			target_public_at_bridged_chain: T::BridgedAccountPublic,
			bridged_currency_transfer: RawBridgedTransferCall,
			bridged_currency_transfer_signature: T::BridgedAccountSignature,
		) -> DispatchResultWithPostInfo {
			// ensure that the `origin` is the same account that is mentioned in the `swap` intention
			let origin_account = ensure_signed(origin)?;
			ensure!(
				origin_account == swap.source_account_at_this_chain,
				Error::<T, I>::MismatchedSwapSourceOrigin,
			);

			// we can't exchange less than existential deposit (the temporary `swap_account` account
			// won't be created then)
			//
			// the same can also happen with the `swap.bridged_balance`, but we can't check it
			// here (without additional knowledge of the Bridged chain). So it is the `origin`
			// responsibility to check that the swap is valid.
			ensure!(
				swap.source_balance_at_this_chain >= T::ThisCurrency::minimum_balance(),
				Error::<T, I>::TooLowBalanceOnThisChain,
			);

			let swap_account = swap_account_id::<T, I>(&swap);
			frame_support::storage::with_transaction(|| {
				// funds are transferred from This account to the temporary Swap account
				let message_delivery_and_dispatch_fee = T::MessageDeliveryAndDispatchFee::get();
				let transfer_result = T::ThisCurrency::transfer(
					&swap.source_account_at_this_chain,
					&swap_account,
					// saturating_add is ok, or we have the chain where single holder owns all tokens
					swap.source_balance_at_this_chain
						.saturating_add(message_delivery_and_dispatch_fee),
					// if we'll allow account to die, then he'll be unable to `cancel_claim`
					// if something won't work
					ExistenceRequirement::KeepAlive,
				);
				if let Err(err) = transfer_result {
					log::error!(
						target: "runtime::bridge-token-swap",
						"Failed to transfer This chain tokens for the swap {:?} to Swap account ({:?}): {:?}",
						swap,
						swap_account,
						err,
					);

					return sp_runtime::TransactionOutcome::Rollback(Err(
						Error::<T, I>::FailedToTransferToSwapAccount.into()
					));
				}

				// the transfer message is sent over the bridge. The message is supposed to be a
				// `Currency::transfer` call on the bridged chain, but no checks are made - it is
				// the transaction submitter to ensure it is valid.
				let send_message_result = T::MessagesBridge::send_message(
					swap_account.clone(),
					T::OutboundMessageLaneId::get(),
					bp_message_dispatch::MessagePayload {
						spec_version: T::BridgedChainSpecVersion::get(),
						weight: T::BridgedChainTransferWeight::get(),
						origin: bp_message_dispatch::CallOrigin::TargetAccount(
							swap_account,
							target_public_at_bridged_chain,
							bridged_currency_transfer_signature,
						),
						dispatch_fee_payment: DispatchFeePayment::AtTargetChain,
						call: bridged_currency_transfer,
					},
					message_delivery_and_dispatch_fee,
				);
				let transfer_message_nonce = match send_message_result {
					Ok(transfer_message_nonce) => transfer_message_nonce,
					Err(err) => {
						log::error!(
							target: "runtime::bridge-token-swap",
							"Failed to send token transfer message for swap {:?} to the Bridged chain: {:?}",
							swap,
							err,
						);

						return sp_runtime::TransactionOutcome::Rollback(Err(
							Error::<T, I>::FailedToSendTransferMessage.into(),
						));
					}
				};

				// remember that we have started the swap
				let swap_hash = swap.using_encoded(blake2_256).into();
				let insert_swap_result = PendingSwaps::<T, I>::try_mutate(swap_hash, |maybe_state| {
					if maybe_state.is_some() {
						return Err(());
					}

					*maybe_state = Some(TokenSwapState::Started);
					Ok(())
				});
				if insert_swap_result.is_err() {
					log::error!(
						target: "runtime::bridge-token-swap",
						"Failed to start token swap {:?}: the swap is already started",
						swap,
					);

					return sp_runtime::TransactionOutcome::Rollback(Err(Error::<T, I>::SwapAlreadyStarted.into()));
				}

				// remember that we're waiting for the transfer message delivery confirmation
				PendingMessages::<T, I>::insert(transfer_message_nonce, swap_hash);

				// finally - emit the event
				Self::deposit_event(Event::SwapStarted(swap_hash, transfer_message_nonce));

				sp_runtime::TransactionOutcome::Commit(Ok(().into()))
			})
		}

		/// Claim previously reserved `source_balance_at_this_chain` by `target_account_at_this_chain`.
		///
		/// **WARNING**: the correct way to call this function is to call it over the messages bridge with
		/// dispatch origin set to `pallet_bridge_dispatch::CallOrigin::SourceAccount(target_account_at_bridged_chain)`.
		///
		/// This should be called only when successful transfer confirmation has been received.
		#[pallet::weight(0)]
		pub fn claim_swap(origin: OriginFor<T>, swap: TokenSwapOf<T, I>) -> DispatchResultWithPostInfo {
			// ensure that the `origin` is controlled by the `swap.target_account_at_bridged_chain`
			let origin_account = ensure_signed(origin)?;
			let target_account_at_this_chain = target_account_at_this_chain::<T, I>(&swap);
			ensure!(
				origin_account == target_account_at_this_chain,
				Error::<T, I>::InvalidClaimant,
			);

			// ensure that the swap is confirmed
			let swap_hash = swap.using_encoded(blake2_256).into();
			let swap_state = PendingSwaps::<T, I>::get(swap_hash);
			match swap_state {
				Some(TokenSwapState::Started) => fail!(Error::<T, I>::SwapIsPending),
				Some(TokenSwapState::Confirmed) => {
					let is_claim_allowed = match swap.swap_type {
						TokenSwapType::TemporaryTargetAccountAtBridgedChain => true,
						TokenSwapType::LockClaimUntilBlock(block_number, _) => {
							block_number < frame_system::Pallet::<T>::block_number()
						}
					};

					ensure!(is_claim_allowed, Error::<T, I>::SwapIsTemporaryLocked);
				}
				Some(TokenSwapState::Failed) => fail!(Error::<T, I>::SwapIsFailed),
				None => fail!(Error::<T, I>::SwapIsInactive),
			}

			complete_claim::<T, I>(swap, swap_hash, origin_account, Event::SwapClaimed(swap_hash))
		}

		/// Return previously reserved `source_balance_at_this_chain` back to the `source_account_at_this_chain`.
		///
		/// This should be called only when transfer has failed at Bridged chain and we have received
		/// notification about thate.
		#[pallet::weight(0)]
		pub fn cancel_swap(origin: OriginFor<T>, swap: TokenSwapOf<T, I>) -> DispatchResultWithPostInfo {
			// ensure that the `origin` is the same account that is mentioned in the `swap` intention
			let origin_account = ensure_signed(origin)?;
			ensure!(
				origin_account == swap.source_account_at_this_chain,
				Error::<T, I>::MismatchedSwapSourceOrigin,
			);

			// ensure that the swap has failed
			let swap_hash = swap.using_encoded(blake2_256).into();
			let swap_state = PendingSwaps::<T, I>::get(swap_hash);
			match swap_state {
				Some(TokenSwapState::Started) => fail!(Error::<T, I>::SwapIsPending),
				Some(TokenSwapState::Confirmed) => fail!(Error::<T, I>::SwapIsConfirmed),
				Some(TokenSwapState::Failed) => {
					// we allow cancelling swap even before lock period is over - the `source_account_at_this_chain`
					// has already paid for nothing and it is up to him to decide whether he want to try again
				}
				None => fail!(Error::<T, I>::SwapIsInactive),
			}

			complete_claim::<T, I>(swap, swap_hash, origin_account, Event::SwapCancelled(swap_hash))
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config<I>, I: 'static = ()> {
		/// Tokens swap has been started and message has been sent to the bridged message.
		///
		/// The payload is the swap hash and the transfer message nonce.
		SwapStarted(H256, MessageNonce),
		/// Token swap has been claimed.
		SwapClaimed(H256),
		/// Token swap has been cancelled.
		SwapCancelled(H256),
	}

	#[pallet::error]
	pub enum Error<T, I = ()> {
		/// The account that has submitted the `start_claim` doesn't match the `TokenSwap::source_account_at_this_chain`.
		MismatchedSwapSourceOrigin,
		/// The swap balance in This chain tokens is below existential deposit and can't be made.
		TooLowBalanceOnThisChain,
		/// Transfer from This chain account to temporary Swap account has failed.
		FailedToTransferToSwapAccount,
		/// Transfer from the temporary Swap account to the derived account of Bridged account has failed.
		FailedToTransferFromSwapAccount,
		/// The message to transfer tokens on Target chain can't be sent.
		FailedToSendTransferMessage,
		/// The same swap is already started.
		SwapAlreadyStarted,
		/// Swap outcome is not yet received.
		SwapIsPending,
		/// Someone is trying to claim swap that has failed.
		SwapIsFailed,
		/// Claiming swap is not allowed.
		///
		/// Now the only possible case when you may get this error, is when you're trying to claim swap with
		/// `TokenSwapType::LockClaimUntilBlock` before lock period is over.
		SwapIsTemporaryLocked,
		/// Someone is trying to cancel swap that has been confirmed.
		SwapIsConfirmed,
		/// Someone is trying to claim/cancel swap that is either not started or already claimed/cancelled.
		SwapIsInactive,
		/// The swap claimant is invalid.
		InvalidClaimant,
	}

	/// Pending token swaps states.
	#[pallet::storage]
	pub type PendingSwaps<T: Config<I>, I: 'static = ()> = StorageMap<_, Identity, H256, TokenSwapState>;

	/// Pending transfer messages.
	#[pallet::storage]
	pub type PendingMessages<T: Config<I>, I: 'static = ()> = StorageMap<_, Identity, MessageNonce, H256>;

	impl<T: Config<I>, I: 'static> OnDeliveryConfirmed for Pallet<T, I> {
		fn on_messages_delivered(lane: &LaneId, delivered_messages: &DeliveredMessages) -> Weight {
			// we're only interested in our lane messages
			if *lane != T::OutboundMessageLaneId::get() {
				return 0;
			}

			// so now we're dealing with our lane messages. Ideally we'll have dedicated lane
			// and every message from `delivered_messages` is actually our transfer message.
			// But it may be some shared lane (which is not recommended).
			let mut reads = 0;
			let mut writes = 0;
			for message_nonce in delivered_messages.begin..=delivered_messages.end {
				reads += 1;
				if let Some(swap_hash) = PendingMessages::<T, I>::take(message_nonce) {
					writes += 1;
					PendingSwaps::<T, I>::insert(
						swap_hash,
						if delivered_messages.message_dispatch_result(message_nonce) {
							TokenSwapState::Confirmed
						} else {
							TokenSwapState::Failed
						},
					);
				}
			}

			<T as frame_system::Config>::DbWeight::get().reads_writes(reads, writes)
		}
	}

	/// Returns temporary account id used to lock funds during swap on This chain.
	pub(crate) fn swap_account_id<T: Config<I>, I: 'static>(swap: &TokenSwapOf<T, I>) -> T::AccountId {
		T::FromSwapToThisAccountIdConverter::convert(swap.using_encoded(blake2_256).into())
	}

	/// Expected target account representation on This chain (aka `target_account_at_this_chain`).
	pub(crate) fn target_account_at_this_chain<T: Config<I>, I: 'static>(swap: &TokenSwapOf<T, I>) -> T::AccountId {
		T::FromBridgedToThisAccountIdConverter::convert(bp_runtime::derive_account_id(
			T::BridgeChainId::get(),
			bp_runtime::SourceAccount::Account(swap.target_account_at_bridged_chain.clone()),
		))
	}

	/// Complete claim with given outcome.
	pub(crate) fn complete_claim<T: Config<I>, I: 'static>(
		swap: TokenSwapOf<T, I>,
		swap_hash: H256,
		destination_account: T::AccountId,
		event: Event<T, I>,
	) -> DispatchResultWithPostInfo {
		let swap_account = swap_account_id::<T, I>(&swap);
		frame_support::storage::with_transaction(|| {
			// funds are transferred from the temporary Swap account to the destination account
			let transfer_result = T::ThisCurrency::transfer(
				&swap_account,
				&destination_account,
				swap.source_balance_at_this_chain,
				ExistenceRequirement::AllowDeath,
			);
			if let Err(err) = transfer_result {
				log::error!(
					target: "runtime::bridge-token-swap",
					"Failed to transfer This chain tokens for the swap {:?} from the Swap account {:?} to {:?}: {:?}",
					swap,
					swap_account,
					destination_account,
					err,
				);

				return sp_runtime::TransactionOutcome::Rollback(Err(
					Error::<T, I>::FailedToTransferFromSwapAccount.into()
				));
			}

			// forget about swap
			PendingSwaps::<T, I>::remove(swap_hash);

			// finally - emit the event
			Pallet::<T, I>::deposit_event(event);

			sp_runtime::TransactionOutcome::Commit(Ok(().into()))
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::*;
	use frame_support::{assert_noop, assert_ok};

	const CAN_CLAIM_BLOCK_NUMBER: u64 = 11;

	const BRIDGED_CHAIN_ACCOUNT_PUBLIC: BridgedAccountPublic = 1;
	const BRIDGED_CHAIN_ACCOUNT_SIGNATURE: BridgedAccountSignature = 2;
	const BRIDGED_CHAIN_ACCOUNT: BridgedAccountId = 3;

	fn test_swap() -> TokenSwapOf<TestRuntime, ()> {
		bp_token_swap::TokenSwap {
			swap_type: TokenSwapType::LockClaimUntilBlock(CAN_CLAIM_BLOCK_NUMBER - 1, 0.into()),
			source_balance_at_this_chain: 100,
			source_account_at_this_chain: THIS_CHAIN_ACCOUNT,
			target_balance_at_bridged_chain: 200,
			target_account_at_bridged_chain: BRIDGED_CHAIN_ACCOUNT,
		}
	}

	fn test_swap_hash() -> H256 {
		test_swap().using_encoded(blake2_256).into()
	}

	fn test_transfer() -> RawBridgedTransferCall {
		vec![OK_TRANSFER_CALL]
	}

	fn start_test_swap() {
		assert_ok!(Pallet::<TestRuntime>::create_swap(
			Origin::signed(THIS_CHAIN_ACCOUNT),
			test_swap(),
			BRIDGED_CHAIN_ACCOUNT_PUBLIC,
			test_transfer(),
			BRIDGED_CHAIN_ACCOUNT_SIGNATURE,
		));
	}

	fn receive_test_swap_confirmation(success: bool) {
		Pallet::<TestRuntime, ()>::on_messages_delivered(
			&OutboundMessageLaneId::get(),
			&DeliveredMessages::new(MESSAGE_NONCE, success),
		);
	}

	#[test]
	fn create_swap_fails_if_origin_is_incorrect() {
		run_test(|| {
			assert_noop!(
				Pallet::<TestRuntime>::create_swap(
					Origin::signed(THIS_CHAIN_ACCOUNT + 1),
					test_swap(),
					BRIDGED_CHAIN_ACCOUNT_PUBLIC,
					test_transfer(),
					BRIDGED_CHAIN_ACCOUNT_SIGNATURE,
				),
				Error::<TestRuntime, ()>::MismatchedSwapSourceOrigin
			);
		});
	}

	#[test]
	fn create_swap_fails_if_this_chain_balance_is_below_existential_deposit() {
		run_test(|| {
			let mut swap = test_swap();
			swap.source_balance_at_this_chain = ExistentialDeposit::get() - 1;
			assert_noop!(
				Pallet::<TestRuntime>::create_swap(
					Origin::signed(THIS_CHAIN_ACCOUNT),
					swap,
					BRIDGED_CHAIN_ACCOUNT_PUBLIC,
					test_transfer(),
					BRIDGED_CHAIN_ACCOUNT_SIGNATURE,
				),
				Error::<TestRuntime, ()>::TooLowBalanceOnThisChain
			);
		});
	}

	#[test]
	fn create_swap_fails_if_currency_transfer_to_swap_account_fails() {
		run_test(|| {
			let mut swap = test_swap();
			swap.source_balance_at_this_chain = THIS_CHAIN_ACCOUNT_BALANCE + 1;
			assert_noop!(
				Pallet::<TestRuntime>::create_swap(
					Origin::signed(THIS_CHAIN_ACCOUNT),
					swap,
					BRIDGED_CHAIN_ACCOUNT_PUBLIC,
					test_transfer(),
					BRIDGED_CHAIN_ACCOUNT_SIGNATURE,
				),
				Error::<TestRuntime, ()>::FailedToTransferToSwapAccount
			);
		});
	}

	#[test]
	fn create_swap_fails_if_send_message_fails() {
		run_test(|| {
			let mut transfer = test_transfer();
			transfer[0] = BAD_TRANSFER_CALL;
			assert_noop!(
				Pallet::<TestRuntime>::create_swap(
					Origin::signed(THIS_CHAIN_ACCOUNT),
					test_swap(),
					BRIDGED_CHAIN_ACCOUNT_PUBLIC,
					transfer,
					BRIDGED_CHAIN_ACCOUNT_SIGNATURE,
				),
				Error::<TestRuntime, ()>::FailedToSendTransferMessage
			);
		});
	}

	#[test]
	fn create_swap_fails_swap_is_active() {
		run_test(|| {
			assert_ok!(Pallet::<TestRuntime>::create_swap(
				Origin::signed(THIS_CHAIN_ACCOUNT),
				test_swap(),
				BRIDGED_CHAIN_ACCOUNT_PUBLIC,
				test_transfer(),
				BRIDGED_CHAIN_ACCOUNT_SIGNATURE,
			));

			assert_noop!(
				Pallet::<TestRuntime>::create_swap(
					Origin::signed(THIS_CHAIN_ACCOUNT),
					test_swap(),
					BRIDGED_CHAIN_ACCOUNT_PUBLIC,
					test_transfer(),
					BRIDGED_CHAIN_ACCOUNT_SIGNATURE,
				),
				Error::<TestRuntime, ()>::SwapAlreadyStarted
			);
		});
	}

	#[test]
	fn create_swap_succeeds() {
		run_test(|| {
			frame_system::Pallet::<TestRuntime>::set_block_number(1);
			frame_system::Pallet::<TestRuntime>::reset_events();

			assert_ok!(Pallet::<TestRuntime>::create_swap(
				Origin::signed(THIS_CHAIN_ACCOUNT),
				test_swap(),
				BRIDGED_CHAIN_ACCOUNT_PUBLIC,
				test_transfer(),
				BRIDGED_CHAIN_ACCOUNT_SIGNATURE,
			));

			let swap_hash = test_swap_hash();
			assert_eq!(
				PendingSwaps::<TestRuntime>::get(swap_hash),
				Some(TokenSwapState::Started)
			);
			assert_eq!(PendingMessages::<TestRuntime>::get(MESSAGE_NONCE), Some(swap_hash));
			assert_eq!(
				pallet_balances::Pallet::<TestRuntime>::free_balance(&swap_account_id::<TestRuntime, ()>(&test_swap())),
				test_swap().source_balance_at_this_chain + MessageDeliveryAndDispatchFee::get(),
			);
			assert!(
				frame_system::Pallet::<TestRuntime>::events()
					.iter()
					.any(|e| e.event
						== crate::mock::Event::TokenSwap(crate::Event::SwapStarted(swap_hash, MESSAGE_NONCE,))),
				"Missing SwapStarted event: {:?}",
				frame_system::Pallet::<TestRuntime>::events(),
			);
		});
	}

	#[test]
	fn claim_swap_fails_if_origin_is_incorrect() {
		run_test(|| {
			assert_noop!(
				Pallet::<TestRuntime>::claim_swap(
					Origin::signed(1 + target_account_at_this_chain::<TestRuntime, ()>(&test_swap())),
					test_swap(),
				),
				Error::<TestRuntime, ()>::InvalidClaimant
			);
		});
	}

	#[test]
	fn claim_swap_fails_if_swap_is_pending() {
		run_test(|| {
			PendingSwaps::<TestRuntime, ()>::insert(test_swap_hash(), TokenSwapState::Started);

			assert_noop!(
				Pallet::<TestRuntime>::claim_swap(
					Origin::signed(target_account_at_this_chain::<TestRuntime, ()>(&test_swap())),
					test_swap(),
				),
				Error::<TestRuntime, ()>::SwapIsPending
			);
		});
	}

	#[test]
	fn claim_swap_fails_if_swap_is_failed() {
		run_test(|| {
			PendingSwaps::<TestRuntime, ()>::insert(test_swap_hash(), TokenSwapState::Failed);

			assert_noop!(
				Pallet::<TestRuntime>::claim_swap(
					Origin::signed(target_account_at_this_chain::<TestRuntime, ()>(&test_swap())),
					test_swap(),
				),
				Error::<TestRuntime, ()>::SwapIsFailed
			);
		});
	}

	#[test]
	fn claim_swap_fails_if_swap_is_inactive() {
		run_test(|| {
			assert_noop!(
				Pallet::<TestRuntime>::claim_swap(
					Origin::signed(target_account_at_this_chain::<TestRuntime, ()>(&test_swap())),
					test_swap(),
				),
				Error::<TestRuntime, ()>::SwapIsInactive
			);
		});
	}

	#[test]
	fn claim_swap_fails_if_currency_transfer_from_swap_account_fails() {
		run_test(|| {
			frame_system::Pallet::<TestRuntime>::set_block_number(CAN_CLAIM_BLOCK_NUMBER);
			PendingSwaps::<TestRuntime, ()>::insert(test_swap_hash(), TokenSwapState::Confirmed);

			assert_noop!(
				Pallet::<TestRuntime>::claim_swap(
					Origin::signed(target_account_at_this_chain::<TestRuntime, ()>(&test_swap())),
					test_swap(),
				),
				Error::<TestRuntime, ()>::FailedToTransferFromSwapAccount
			);
		});
	}

	#[test]
	fn claim_swap_fails_before_lock_period_is_completed() {
		run_test(|| {
			start_test_swap();
			receive_test_swap_confirmation(true);

			frame_system::Pallet::<TestRuntime>::set_block_number(CAN_CLAIM_BLOCK_NUMBER - 1);

			assert_noop!(
				Pallet::<TestRuntime>::claim_swap(
					Origin::signed(target_account_at_this_chain::<TestRuntime, ()>(&test_swap())),
					test_swap(),
				),
				Error::<TestRuntime, ()>::SwapIsTemporaryLocked
			);
		});
	}

	#[test]
	fn claim_swap_succeeds() {
		run_test(|| {
			start_test_swap();
			receive_test_swap_confirmation(true);

			frame_system::Pallet::<TestRuntime>::set_block_number(CAN_CLAIM_BLOCK_NUMBER);
			frame_system::Pallet::<TestRuntime>::reset_events();

			assert_ok!(Pallet::<TestRuntime>::claim_swap(
				Origin::signed(target_account_at_this_chain::<TestRuntime, ()>(&test_swap())),
				test_swap(),
			));

			let swap_hash = test_swap_hash();
			assert_eq!(PendingSwaps::<TestRuntime>::get(swap_hash), None);
			assert_eq!(
				pallet_balances::Pallet::<TestRuntime>::free_balance(&swap_account_id::<TestRuntime, ()>(&test_swap())),
				0,
			);
			assert_eq!(
				pallet_balances::Pallet::<TestRuntime>::free_balance(&target_account_at_this_chain::<TestRuntime, ()>(
					&test_swap()
				),),
				test_swap().source_balance_at_this_chain,
			);
			assert!(
				frame_system::Pallet::<TestRuntime>::events()
					.iter()
					.any(|e| e.event == crate::mock::Event::TokenSwap(crate::Event::SwapClaimed(swap_hash,))),
				"Missing SwapClaimed event: {:?}",
				frame_system::Pallet::<TestRuntime>::events(),
			);
		});
	}

	#[test]
	fn cancel_swap_fails_if_origin_is_incorrect() {
		run_test(|| {
			start_test_swap();
			receive_test_swap_confirmation(false);

			assert_noop!(
				Pallet::<TestRuntime>::cancel_swap(Origin::signed(THIS_CHAIN_ACCOUNT + 1), test_swap()),
				Error::<TestRuntime, ()>::MismatchedSwapSourceOrigin
			);
		});
	}

	#[test]
	fn cancel_swap_fails_if_swap_is_pending() {
		run_test(|| {
			start_test_swap();

			assert_noop!(
				Pallet::<TestRuntime>::cancel_swap(Origin::signed(THIS_CHAIN_ACCOUNT), test_swap()),
				Error::<TestRuntime, ()>::SwapIsPending
			);
		});
	}

	#[test]
	fn cancel_swap_fails_if_swap_is_confirmed() {
		run_test(|| {
			start_test_swap();
			receive_test_swap_confirmation(true);

			assert_noop!(
				Pallet::<TestRuntime>::cancel_swap(Origin::signed(THIS_CHAIN_ACCOUNT), test_swap()),
				Error::<TestRuntime, ()>::SwapIsConfirmed
			);
		});
	}

	#[test]
	fn cancel_swap_fails_if_swap_is_inactive() {
		run_test(|| {
			assert_noop!(
				Pallet::<TestRuntime>::cancel_swap(Origin::signed(THIS_CHAIN_ACCOUNT), test_swap()),
				Error::<TestRuntime, ()>::SwapIsInactive
			);
		});
	}

	#[test]
	fn cancel_swap_fails_if_currency_transfer_from_swap_account_fails() {
		run_test(|| {
			start_test_swap();
			receive_test_swap_confirmation(false);
			let _ = pallet_balances::Pallet::<TestRuntime>::slash(
				&swap_account_id::<TestRuntime, ()>(&test_swap()),
				test_swap().source_balance_at_this_chain,
			);

			assert_noop!(
				Pallet::<TestRuntime>::cancel_swap(Origin::signed(THIS_CHAIN_ACCOUNT), test_swap()),
				Error::<TestRuntime, ()>::FailedToTransferFromSwapAccount
			);
		});
	}

	#[test]
	fn cancel_swap_succeeds() {
		run_test(|| {
			start_test_swap();
			receive_test_swap_confirmation(false);

			frame_system::Pallet::<TestRuntime>::set_block_number(1);
			frame_system::Pallet::<TestRuntime>::reset_events();

			assert_ok!(Pallet::<TestRuntime>::cancel_swap(
				Origin::signed(THIS_CHAIN_ACCOUNT),
				test_swap()
			));

			let swap_hash = test_swap_hash();
			assert_eq!(PendingSwaps::<TestRuntime>::get(swap_hash), None);
			assert_eq!(
				pallet_balances::Pallet::<TestRuntime>::free_balance(&swap_account_id::<TestRuntime, ()>(&test_swap())),
				0,
			);
			assert_eq!(
				pallet_balances::Pallet::<TestRuntime>::free_balance(&THIS_CHAIN_ACCOUNT),
				THIS_CHAIN_ACCOUNT_BALANCE - MessageDeliveryAndDispatchFee::get(),
			);
			assert!(
				frame_system::Pallet::<TestRuntime>::events()
					.iter()
					.any(|e| e.event == crate::mock::Event::TokenSwap(crate::Event::SwapCancelled(swap_hash,))),
				"Missing SwapCancelled event: {:?}",
				frame_system::Pallet::<TestRuntime>::events(),
			);
		});
	}

	#[test]
	fn messages_delivery_confirmations_are_accepted() {
		run_test(|| {
			start_test_swap();
			assert_eq!(
				PendingMessages::<TestRuntime, ()>::get(MESSAGE_NONCE),
				Some(test_swap_hash())
			);
			assert_eq!(
				PendingSwaps::<TestRuntime, ()>::get(test_swap_hash()),
				Some(TokenSwapState::Started)
			);

			// when unrelated messages are delivered
			let mut messages = DeliveredMessages::new(MESSAGE_NONCE - 2, true);
			messages.note_dispatched_message(false);
			Pallet::<TestRuntime, ()>::on_messages_delivered(&OutboundMessageLaneId::get(), &messages);
			assert_eq!(
				PendingMessages::<TestRuntime, ()>::get(MESSAGE_NONCE),
				Some(test_swap_hash())
			);
			assert_eq!(
				PendingSwaps::<TestRuntime, ()>::get(test_swap_hash()),
				Some(TokenSwapState::Started)
			);

			// when message we're interested in is accompanied by a bunch of other messages
			let mut messages = DeliveredMessages::new(MESSAGE_NONCE - 1, false);
			messages.note_dispatched_message(true);
			messages.note_dispatched_message(false);
			Pallet::<TestRuntime, ()>::on_messages_delivered(&OutboundMessageLaneId::get(), &messages);
			assert_eq!(PendingMessages::<TestRuntime, ()>::get(MESSAGE_NONCE), None);
			assert_eq!(
				PendingSwaps::<TestRuntime, ()>::get(test_swap_hash()),
				Some(TokenSwapState::Confirmed)
			);
		});
	}
}
