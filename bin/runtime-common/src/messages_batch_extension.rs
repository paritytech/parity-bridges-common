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
//! It also refunds transaction cost if the transaction is an `utility.batchAll()`
//! with calls that are: delivering new messsage and all necessary underlying headers
//! (parachain or relay chain).

use bp_messages::{target_chain::SourceHeaderChain, LaneId, MessageNonce};
use bp_polkadot_core::parachains::ParaId;
use bp_runtime::{Chain, HashOf};
use codec::{Decode, Encode};
use frame_support::{
	dispatch::{CallableCallFor, DispatchInfo, Dispatchable, PostDispatchInfo},
	traits::IsSubType,
	RuntimeDebugNoBound,
};
use pallet_bridge_grandpa::{
	BridgedChain, Call as GrandpaCall, Config as GrandpaConfig, Pallet as GrandpaPallet,
};
use pallet_bridge_messages::{
	Call as MessagesCall, Config as MessagesConfig, Pallet as MessagesPallet,
};
use pallet_bridge_parachains::{
	Call as ParachainsCall, Config as ParachainsConfig, Pallet as ParachainsPallet, RelayBlockHash,
	RelayBlockHasher, RelayBlockNumber,
};
use pallet_bridge_relayers::{Config as RelayersConfig, Pallet as RelayersPallet};
use pallet_transaction_payment::{Config as TransactionPaymentConfig, OnChargeTransaction};
use pallet_utility::{Call as UtilityCall, Config as UtilityConfig, Pallet as UtilityPallet};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{DispatchInfoOf, Get, Header as HeaderT, PostDispatchInfoOf, SignedExtension, Zero},
	transaction_validity::{TransactionValidity, TransactionValidityError, ValidTransaction},
	DispatchResult, FixedPointOperand,
};
use sp_std::marker::PhantomData;

// TODO: is it possible to impl it for several bridges at once? Like what we have in
// `BridgeRejectObsoleteHeadersAndMessages`? If it is hard to do now - just submit an issue

#[derive(Decode, Encode, RuntimeDebugNoBound, TypeInfo)]
#[scale_info(skip_type_params(RT, GI, PI, MI, BE, PID, LID))]
pub struct RefundRelayerForMessagesDeliveryFromParachain<RT, GI, PI, MI, BE, PID, LID>(
	PhantomData<(RT, GI, PI, MI, BE, PID, LID)>,
);

impl<R, GI, PI, MI, BE, PID, LID> Clone
	for RefundRelayerForMessagesDeliveryFromParachain<R, GI, PI, MI, BE, PID, LID>
{
	fn clone(&self) -> Self {
		RefundRelayerForMessagesDeliveryFromParachain(PhantomData)
	}
}

impl<R, GI, PI, MI, BE, PID, LID> Eq
	for RefundRelayerForMessagesDeliveryFromParachain<R, GI, PI, MI, BE, PID, LID>
{
}

impl<R, GI, PI, MI, BE, PID, LID> PartialEq
	for RefundRelayerForMessagesDeliveryFromParachain<R, GI, PI, MI, BE, PID, LID>
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

/// Type of the call that the extension recognizes.
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
	/// Best delivered message nonce.
	pub last_delivered_nonce: MessageNonce,
}

// without this typedef rustfmt fails with internal err
type BalanceOf<R> =
	<<R as TransactionPaymentConfig>::OnChargeTransaction as OnChargeTransaction<R>>::Balance;
type CallOf<R> = <R as frame_system::Config>::RuntimeCall;

impl<R, GI, PI, MI, BE, PID, LID> SignedExtension
	for RefundRelayerForMessagesDeliveryFromParachain<R, GI, PI, MI, BE, PID, LID>
where
	R: 'static
		+ Send
		+ Sync
		+ frame_system::Config
		+ TransactionPaymentConfig
		+ UtilityConfig<RuntimeCall = CallOf<R>>
		+ GrandpaConfig<GI>
		+ ParachainsConfig<PI, BridgesGrandpaPalletInstance = GI>
		+ MessagesConfig<MI>
		+ RelayersConfig<Reward = BalanceOf<R>>,
	GI: 'static + Send + Sync,
	PI: 'static + Send + Sync,
	MI: 'static + Send + Sync,
	BE: 'static
		+ Send
		+ Sync
		+ Default
		+ SignedExtension<AccountId = R::AccountId, Call = CallOf<R>>,
	PID: 'static + Send + Sync + Get<u32>,
	LID: 'static + Send + Sync + Get<LaneId>,
	<R as frame_system::Config>::RuntimeCall:
		Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
	BalanceOf<R>: FixedPointOperand,
	CallOf<R>: IsSubType<CallableCallFor<UtilityPallet<R>, R>>
		+ IsSubType<CallableCallFor<GrandpaPallet<R, GI>, R>>
		+ IsSubType<CallableCallFor<ParachainsPallet<R, PI>, R>>
		+ IsSubType<CallableCallFor<MessagesPallet<R, MI>, R>>,
	<R as GrandpaConfig<GI>>::BridgedChain:
		Chain<BlockNumber = RelayBlockNumber, Hash = RelayBlockHash, Hasher = RelayBlockHasher>,
	<R as MessagesConfig<MI>>::SourceHeaderChain: SourceHeaderChain<
		MessagesProof = crate::messages::target::FromBridgedChainMessagesProof<
			HashOf<BridgedChain<R, GI>>,
		>,
	>,
{
	const IDENTIFIER: &'static str = "RefundRelayerForMessagesDeliveryFromParachain";
	type AccountId = R::AccountId;
	type Call = CallOf<R>;
	type AdditionalSigned = ();
	type Pre = Option<PreDispatchData<R::AccountId>>;

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
		who: &Self::AccountId,
		call: &Self::Call,
		post_info: &DispatchInfoOf<Self::Call>,
		len: usize,
	) -> Result<Self::Pre, TransactionValidityError> {
		// reject batch transactions with obsolete headers
		if let Some(UtilityCall::<R>::batch_all { ref calls }) = call.is_sub_type() {
			for nested_call in calls {
				let reject_obsolete_transactions = BE::default();
				reject_obsolete_transactions.pre_dispatch(who, nested_call, post_info, len)?;
			}
		}

		// now try to check if tx matches one of types we support
		let parse_call_type = || {
			if let Some(UtilityCall::<R>::batch_all { ref calls }) = call.is_sub_type() {
				if calls.len() == 3 {
					return Some(CallType::AllFinalityAndDelivery(
						extract_expected_relay_chain_state::<R, GI>(&calls[0])?,
						extract_expected_parachain_state::<R, GI, PI, PID>(&calls[1])?,
						extract_messages_state::<R, GI, MI, LID>(&calls[2])?,
					))
				}
				if calls.len() == 2 {
					return Some(CallType::ParachainFinalityAndDelivery(
						extract_expected_parachain_state::<R, GI, PI, PID>(&calls[0])?,
						extract_messages_state::<R, GI, MI, LID>(&calls[1])?,
					))
				}
				return None
			}

			Some(CallType::Delivery(extract_messages_state::<R, GI, MI, LID>(call)?))
		};

		Ok(parse_call_type().map(|call_type| PreDispatchData { relayer: who.clone(), call_type }))
	}

	fn post_dispatch(
		pre: Option<Self::Pre>,
		info: &DispatchInfoOf<Self::Call>,
		post_info: &PostDispatchInfoOf<Self::Call>,
		len: usize,
		result: &DispatchResult,
	) -> Result<(), TransactionValidityError> {
		// we never refund anything if it is not bridge transaction or if it is a bridge
		// transaction that we do not support here
		let (relayer, call_type) = match pre {
			Some(Some(pre)) => (pre.relayer, pre.call_type),
			_ => return Ok(()),
		};

		// we never refund anything if transaction has failed
		if result.is_err() {
			return Ok(())
		}

		// check if relay chain state has been updated
		if let CallType::AllFinalityAndDelivery(expected_relay_chain_state, _, _) = call_type {
			let actual_relay_chain_state = relay_chain_state::<R, GI>();
			if actual_relay_chain_state != Some(expected_relay_chain_state) {
				// we only refund relayer if all calls have updated chain state
				return Ok(())
			}

			// there's a conflict between how bridge GRANDPA pallet works and the
			// `AllFinalityAndDelivery` transaction. If relay chain header is mandatory, the GRANDPA
			// pallet returns `Pays::No`, because such transaction is mandatory for operating the
			// bridge. But `utility.batchAll` transaction always requires payment. But in both cases
			// we'll refund relayer - either explicitly here, or using `Pays::No` if he's choosing
			// to submit dedicated transaction.
		}

		// check if parachain state has been updated
		match call_type {
			CallType::AllFinalityAndDelivery(_, expected_parachain_state, _) |
			CallType::ParachainFinalityAndDelivery(expected_parachain_state, _) => {
				let actual_parachain_state = parachain_state::<R, PI, PID>();
				if actual_parachain_state != Some(expected_parachain_state) {
					// we only refund relayer if all calls have updated chain state
					return Ok(())
				}
			},
			_ => (),
		}

		// check if messages have been delivered
		let actual_messages_state = messages_state::<R, MI, LID>();
		let pre_dispatch_messages_state = call_type.pre_dispatch_messages_state();
		if actual_messages_state == Some(pre_dispatch_messages_state) {
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
		let reward = pallet_transaction_payment::Pallet::<R>::compute_actual_fee(
			len as _, info, post_info, tip,
		);

		// finally - register reward in relayers pallet
		RelayersPallet::<R>::register_relayer_reward(LID::get(), &relayer, reward);

		Ok(())
	}
}

/// Extracts expected relay chain state from the call.
fn extract_expected_relay_chain_state<R, GI>(call: &CallOf<R>) -> Option<ExpectedRelayChainState>
where
	R: GrandpaConfig<GI>,
	GI: 'static,
	<R as GrandpaConfig<GI>>::BridgedChain: Chain<BlockNumber = RelayBlockNumber>,
	CallOf<R>: IsSubType<CallableCallFor<GrandpaPallet<R, GI>, R>>,
{
	if let Some(GrandpaCall::<R, GI>::submit_finality_proof { ref finality_target, .. }) =
		call.is_sub_type()
	{
		return Some(ExpectedRelayChainState { best_block_number: *finality_target.number() })
	}
	None
}

/// Extracts expected parachain state from the call.
fn extract_expected_parachain_state<R, GI, PI, PID>(
	call: &CallOf<R>,
) -> Option<ExpectedParachainState>
where
	R: GrandpaConfig<GI> + ParachainsConfig<PI, BridgesGrandpaPalletInstance = GI>,
	GI: 'static,
	PI: 'static,
	PID: Get<u32>,
	<R as GrandpaConfig<GI>>::BridgedChain:
		Chain<BlockNumber = RelayBlockNumber, Hash = RelayBlockHash, Hasher = RelayBlockHasher>,
	CallOf<R>: IsSubType<CallableCallFor<ParachainsPallet<R, PI>, R>>,
{
	if let Some(ParachainsCall::<R, PI>::submit_parachain_heads {
		ref at_relay_block,
		ref parachains,
		..
	}) = call.is_sub_type()
	{
		if parachains.len() == 1 && parachains[0].0 == ParaId(PID::get()) {
			return None
		}

		return Some(ExpectedParachainState { at_relay_block_number: at_relay_block.0 })
	}
	None
}

/// Extracts messages state from the call.
fn extract_messages_state<R, GI, MI, LID>(call: &CallOf<R>) -> Option<MessagesState>
where
	R: GrandpaConfig<GI> + MessagesConfig<MI>,
	GI: 'static,
	MI: 'static,
	LID: Get<LaneId>,
	CallOf<R>: IsSubType<CallableCallFor<MessagesPallet<R, MI>, R>>,
	<R as MessagesConfig<MI>>::SourceHeaderChain: SourceHeaderChain<
		MessagesProof = crate::messages::target::FromBridgedChainMessagesProof<
			HashOf<BridgedChain<R, GI>>,
		>,
	>,
{
	if let Some(MessagesCall::<R, MI>::receive_messages_proof { ref proof, .. }) =
		call.is_sub_type()
	{
		if LID::get() != proof.lane {
			return None
		}

		return Some(MessagesState {
			last_delivered_nonce: MessagesPallet::<R, MI>::inbound_lane_data(proof.lane)
				.last_delivered_nonce(),
		})
	}
	None
}

/// Returns relay chain state that we are interested in.
fn relay_chain_state<R, GI>() -> Option<ExpectedRelayChainState>
where
	R: GrandpaConfig<GI>,
	GI: 'static,
	<R as GrandpaConfig<GI>>::BridgedChain: Chain<BlockNumber = RelayBlockNumber>,
{
	GrandpaPallet::<R, GI>::best_finalized_number()
		.map(|best_block_number| ExpectedRelayChainState { best_block_number })
}

/// Returns parachain state that we are interested in.
fn parachain_state<R, PI, PID>() -> Option<ExpectedParachainState>
where
	R: ParachainsConfig<PI>,
	PI: 'static,
	PID: Get<u32>,
{
	ParachainsPallet::<R, PI>::best_parachain_info(ParaId(PID::get())).map(|para_info| {
		ExpectedParachainState {
			at_relay_block_number: para_info.best_head_hash.at_relay_block_number,
		}
	})
}

/// Returns messages state that we are interested in.
fn messages_state<R, MI, LID>() -> Option<MessagesState>
where
	R: MessagesConfig<MI>,
	MI: 'static,
	LID: Get<LaneId>,
{
	Some(MessagesState {
		last_delivered_nonce: MessagesPallet::<R, MI>::inbound_lane_data(LID::get())
			.last_delivered_nonce(),
	})
}

#[cfg(test)]
mod tests {

}
