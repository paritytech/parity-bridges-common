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

//! Signed extension that refunds relayer if he has delivered some new messages.
//! It also refunds transacation cost if the transaction is an `utility.batchAll()`
//! with calls that are: delivering new messsage and all necessary underlying headers
//! (parachain or relay chain).

use bp_messages::{LaneId, MessageNonce};
use bp_polkadot_core::parachains::ParaId;
use codec::{Decode, Encode};
use frame_support::{
	dispatch::{DispatchInfo, Dispatchable, PostDispatchInfo},
	RuntimeDebugNoBound,
};
use pallet_bridge_parachains::RelayBlockNumber;
use pallet_transaction_payment::OnChargeTransaction;
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{DispatchInfoOf, PostDispatchInfoOf, SignedExtension, Zero},
	transaction_validity::{TransactionValidity, TransactionValidityError, ValidTransaction},
	DispatchResult, FixedPointOperand,
};
use sp_std::marker::PhantomData;

// TODO: is it possible to impl it for several bridges at once? Like what we have in
// `BridgeRejectObsoleteHeadersAndMessages`? If it is hard to do now - just submit an issue

#[derive(Decode, Encode, RuntimeDebugNoBound, TypeInfo)]
#[scale_info(skip_type_params(Runtime, GrandpaInstance, ParachainsInstance, MessagesInstance))]
pub struct RefundRelayerForMessagesDeliveryFromParachain<
	Runtime,
	GrandpaInstance,
	ParachainsInstance,
	MessagesInstance,
>(PhantomData<(Runtime, GrandpaInstance, ParachainsInstance, MessagesInstance)>);

impl<Runtime, GrandpaInstance, ParachainsInstance, MessagesInstance> Clone
	for RefundRelayerForMessagesDeliveryFromParachain<
		Runtime,
		GrandpaInstance,
		ParachainsInstance,
		MessagesInstance,
	>
{
	fn clone(&self) -> Self {
		RefundRelayerForMessagesDeliveryFromParachain(PhantomData)
	}
}

impl<Runtime, GrandpaInstance, ParachainsInstance, MessagesInstance> Eq
	for RefundRelayerForMessagesDeliveryFromParachain<
		Runtime,
		GrandpaInstance,
		ParachainsInstance,
		MessagesInstance,
	>
{
}

impl<Runtime, GrandpaInstance, ParachainsInstance, MessagesInstance> PartialEq
	for RefundRelayerForMessagesDeliveryFromParachain<
		Runtime,
		GrandpaInstance,
		ParachainsInstance,
		MessagesInstance,
	>
{
	fn eq(&self, _other: &Self) -> bool {
		true
	}
}

/// Data that is crafted in `pre_dispatch` method and used at `post_dispatch`.
#[derive(PartialEq)]
pub struct PreDispatchData<AccountId> {
	/// Transaction submitter (relayer) account.
	pub relayer: AccountId,
	/// Type of the call.
	pub call_type: CallType,
}

/// Type of the call that the extension recognizing.
#[derive(Clone, Copy, PartialEq)]
pub enum CallType {
	/// Relay chain finality + parachain finality + message delivery calls.
	AllFinalityAndDelivery(ExpectedRelayChainState, ExpectedParachainState, MessagesState),
	/// Parachain finality + message delivery calls.
	ParachainFinalityAndDelivery(ExpectedParachainState, MessagesState),
	/// Standalone message delivery call.
	Delivery(MessagesState),
}

impl CallType {
	/// Returns the pre-dispatch messages pallet state.
	fn pre_dispatch_messages_state(&self) -> MessagesState {
		match *self {
			Self::AllFinalityAndDelivery(_, _, messages_state) => messages_state,
			Self::ParachainFinalityAndDelivery(_, messages_state) => messages_state,
			Self::Delivery(messages_state) => messages_state,
		}
	}
}

/// Expected post-dispatch state of the relay chain pallet.
#[derive(Clone, Copy, PartialEq)]
pub struct ExpectedRelayChainState {
	/// Best known relay chain block number.
	pub best_block_number: RelayBlockNumber,
}

/// Expected post-dispatch state of the parachain pallet.
#[derive(Clone, Copy, PartialEq)]
pub struct ExpectedParachainState {
	/// Parachain identifier.
	pub para: ParaId,
	/// At which relay block the parachain head has been updated?
	pub at_relay_block_number: RelayBlockNumber,
}

/// Pre-dispatch state of messages pallet.
///
/// This struct is for pre-dispatch state of the pallet, not the expected post-dispatch state.
/// That's because message delivery transaction may deliver some of messages that it brings.
/// If this happens, we consider it "helpful" and refund its cost. If transaction fails to
/// deliver at least one message, it is considered wrong and is not refunded.
#[derive(Clone, Copy, PartialEq)]
pub struct MessagesState {
	/// Message lane identifier.
	pub lane: LaneId,
	/// Best delivered message nonce.
	pub last_delivered_nonce: MessageNonce,
}

// without this typedef rustfmt fails with internal err
type BalanceOf<Runtime> =
	<<Runtime as pallet_transaction_payment::Config>::OnChargeTransaction as OnChargeTransaction<
		Runtime,
	>>::Balance;

impl<Runtime, GrandpaInstance, ParachainsInstance, MessagesInstance> SignedExtension
	for RefundRelayerForMessagesDeliveryFromParachain<
		Runtime,
		GrandpaInstance,
		ParachainsInstance,
		MessagesInstance,
	> where
	Runtime: 'static
		+ Send
		+ Sync
		+ frame_system::Config
		+ pallet_transaction_payment::Config
		+ pallet_bridge_grandpa::Config<GrandpaInstance>
		+ pallet_bridge_parachains::Config<ParachainsInstance>
		+ pallet_bridge_messages::Config<MessagesInstance>
		+ pallet_bridge_relayers::Config<Reward = BalanceOf<Runtime>>,
	GrandpaInstance: 'static + Send + Sync,
	ParachainsInstance: 'static + Send + Sync,
	MessagesInstance: 'static + Send + Sync,
	<Runtime as frame_system::Config>::RuntimeCall:
		Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
	BalanceOf<Runtime>: FixedPointOperand,
{
	const IDENTIFIER: &'static str = "RefundRelayerForMessagesDeliveryFromParachain";
	type AccountId = Runtime::AccountId;
	type Call = Runtime::RuntimeCall;
	type AdditionalSigned = ();
	type Pre = PreDispatchData<Runtime::AccountId>;

	fn additional_signed(&self) -> Result<(), TransactionValidityError> {
		Ok(())
	}

	fn validate(
		&self,
		_who: &Self::AccountId,
		_call: &Self::Call,
		_info: &DispatchInfoOf<Self::Call>,
		_len: usize,
	) -> TransactionValidity {
		Ok(ValidTransaction::default())
	}

	fn pre_dispatch(
		self,
		_who: &Self::AccountId,
		_call: &Self::Call,
		_post_info: &DispatchInfoOf<Self::Call>,
		_len: usize,
	) -> Result<Self::Pre, TransactionValidityError> {
		// TODO: for every call from the batch - call the `BridgesExtension` to ensure that every
		// call transaction brings something new and reject obsolete transactions
		unimplemented!("TODO") // TODO: return actual call type
	}

	fn post_dispatch(
		pre: Option<Self::Pre>,
		info: &DispatchInfoOf<Self::Call>,
		post_info: &PostDispatchInfoOf<Self::Call>,
		len: usize,
		result: &DispatchResult,
	) -> Result<(), TransactionValidityError> {
		// we never refund anything if that is not bridge transaction or if it is a bridge
		// transaction that we do not support here
		let (relayer, call_type) = match pre {
			Some(pre) => (pre.relayer, pre.call_type),
			None => return Ok(()),
		};

		// we never refund anything if transaction has failed
		if result.is_err() {
			return Ok(())
		}

		// check if relay chain state has been updated
		if let CallType::AllFinalityAndDelivery(expected_relay_chain_state, _, _) = call_type {
			let actual_relay_chain_state = relay_chain_state::<Runtime, GrandpaInstance>();
			if actual_relay_chain_state != expected_relay_chain_state {
				// we only refund relayer if all calls have updated chain state
				return Ok(())
			}

			// there's a conflict between how bridge GRANDPA pallet works and the
			// `AllFinalityAndDelivery` transaction. If relay cahin header is mandatory, the GRANDPA
			// pallet returns `Pays::No`, because such transaction is mandatory for operating the
			// bridge. But `utility.batchAll` transaction always requires payment. But in both cases
			// we'll refund relayer - either explicitly here, or using `Pays::No` if he's choosing
			// to submit dedicated transaction.
		}

		// check if parachain state has been updated
		match call_type {
			CallType::AllFinalityAndDelivery(_, expected_parachain_state, _) |
			CallType::ParachainFinalityAndDelivery(expected_parachain_state, _) => {
				let actual_parachain_state = parachain_state::<Runtime, ParachainsInstance>();
				if expected_parachain_state != actual_parachain_state {
					// we only refund relayer if all calls have updated chain state
					return Ok(())
				}
			},
			_ => (),
		}

		// check if messages have been delivered
		let actual_messages_state = messages_state::<Runtime, MessagesInstance>();
		let pre_dispatch_messages_state = call_type.pre_dispatch_messages_state();
		if actual_messages_state == pre_dispatch_messages_state {
			// we only refund relayer if all calls have updated chain state
			return Ok(())
		}

		// regarding the tip - refund that happens here (at this side of the bridge) isn't the whole
		// relayer compensation. He'll receive some amount at the other side of the bridge. It shall
		// (in theory) cover the tip here. Otherwise, if we'll be compensating tip here, some
		// malicious relayer may use huge tips, effectively depleting account that pay rewards. The
		// cost of this attack is nothing. Hence we use zero as tip here.
		let tip = Zero::zero();

		// compute the relayer reward
		let reward = pallet_transaction_payment::Pallet::<Runtime>::compute_actual_fee(
			len as _, info, post_info, tip,
		);

		// finally - regiater reward in relayers pallet
		pallet_bridge_relayers::Pallet::<Runtime>::register_relayer_reward(&relayer, reward);

		Ok(())
	}
}

/// Returns relay chain state that we are interested in.
fn relay_chain_state<Runtime, GrandpaInstance>() -> ExpectedRelayChainState {
	unimplemented!("TODO")
}

/// Returns parachain state that we are interested in.
fn parachain_state<Runtime, ParachainsInstance>() -> ExpectedParachainState {
	unimplemented!("TODO")
}

/// Returns messages state that we are interested in.
fn messages_state<Runtime, MessagesInstance>() -> MessagesState {
	unimplemented!("TODO")
}
