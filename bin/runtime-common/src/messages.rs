// Copyright 2019-2020 Parity Technologies (UK) Ltd.
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

//! Types that allow runtime to act as a source/target endpoint of message lanes.
//!
//! Messages are assumed to be encoded `Call`s of the target chain. Call-dispatch
//! pallet is used to dispatch incoming messages.

use bp_message_dispatch::MessageDispatch as _;
use bp_message_lane::{
	source_chain::LaneMessageVerifier,
	target_chain::{DispatchMessage, MessageDispatch},
	LaneId, MessageNonce,
};
use bp_runtime::InstanceId;
use codec::{Compact, Decode, Input};
use frame_support::RuntimeDebug;
use sp_runtime::traits::{CheckedAdd, CheckedDiv, CheckedMul};
use sp_std::{cmp::PartialOrd, vec::Vec};

/// Bidirectional message bridge.
pub trait MessageBridge {
	/// Instance id of this bridge.
	const INSTANCE: InstanceId;

	/// Relayer interest (in percents).
	const RELAYER_INTEREST_PERCENT: u32;

	/// This chain in context of message bridge.
	type ThisChain: ChainWithMessageLanes;
	/// Bridged chain in context of message bridge.
	type BridgedChain: ChainWithMessageLanes;

	/// Maximal (dispatch) weight of the message that we are able to send to Bridged chain.
	fn maximal_dispatch_weight_of_message_on_bridged_chain() -> WeightOf<BridgedChain<Self>>;

	/// Maximal weight of single message delivery transaction on Bridged chain.
	fn weight_of_delivery_transaction() -> WeightOf<BridgedChain<Self>>;

	/// Maximal weight of single message delivery confirmation transaction on This chain.
	fn weight_of_delivery_confirmation_transaction_on_this_chain() -> WeightOf<ThisChain<Self>>;

	/// Weight of single message reward confirmation on the Bridged chain. This confirmation
	/// is a part of delivery transaction, so this weight is added to the delivery
	/// transaction weight.
	fn weight_of_reward_confirmation_transaction_on_target_chain() -> WeightOf<BridgedChain<Self>>;

	/// Convert weight of This chain to the fee (paid in Balance) of This chain.
	fn this_weight_to_balance(weight: WeightOf<ThisChain<Self>>) -> BalanceOf<ThisChain<Self>>;

	/// Convert weight of the Bridged chain to the fee (paid in Balance) of the Bridged chain.
	fn bridged_weight_to_balance(weight: WeightOf<BridgedChain<Self>>) -> BalanceOf<BridgedChain<Self>>;

	/// Convert This chain Balance into Bridged chain Balance.
	fn this_chain_balance_to_bridged_chain_balance(
		this_balance: BalanceOf<ThisChain<Self>>,
	) -> BalanceOf<BridgedChain<Self>>;
}

/// Chain that has `message-lane` and `call-dispatch` modules.
pub trait ChainWithMessageLanes {
	/// Accound id on the chain.
	type AccountId;
	/// Public key of the chain account that may be used to verify signatures.
	type Signer: Decode;
	/// Signature type used on the chain.
	type Signature: Decode;
	/// Call type on the chain.
	type Call: Decode;
	/// Type of weight that is used on the chain.
	type Weight: From<frame_support::weights::Weight>;
	/// Type of balances that is used on the chain.
	type Balance: CheckedAdd + CheckedDiv + CheckedMul + PartialOrd + From<u32> + Copy;
}

pub(crate) type ThisChain<B> = <B as MessageBridge>::ThisChain;
pub(crate) type BridgedChain<B> = <B as MessageBridge>::BridgedChain;
pub(crate) type AccountIdOf<C> = <C as ChainWithMessageLanes>::AccountId;
pub(crate) type WeightOf<C> = <C as ChainWithMessageLanes>::Weight;
pub(crate) type BalanceOf<C> = <C as ChainWithMessageLanes>::Balance;
pub(crate) type CallOf<C> = <C as ChainWithMessageLanes>::Call;

/// Sub-module that is declaring types required for processing This -> Bridged chain messages.
pub mod source {
	use super::*;

	/// Encoded Call of the Bridged chain. We never try to decode it on This chain.
	pub type BridgedChainOpaqueCall = Vec<u8>;

	/// Message payload for This -> Bridged chain messages.
	pub type FromThisChainMessagePayload<B> = pallet_bridge_call_dispatch::MessagePayload<
		<ThisChain<B> as ChainWithMessageLanes>::Signer,
		<BridgedChain<B> as ChainWithMessageLanes>::Signer,
		<BridgedChain<B> as ChainWithMessageLanes>::Signature,
		BridgedChainOpaqueCall,
	>;

	/// Message verifier that requires submitter to pay minimal delivery and dispatch fee.
	#[derive(RuntimeDebug)]
	pub struct FromThisChainMessageVerifier<B>(sp_std::marker::PhantomData<B>);

	impl<B: MessageBridge> LaneMessageVerifier<
		AccountIdOf<ThisChain<B>>,
		FromThisChainMessagePayload<B>,
		BalanceOf<ThisChain<B>>,
	> for FromThisChainMessageVerifier<B> {
		type Error = &'static str;

		fn verify_message(
			_submitter: &AccountIdOf<ThisChain<B>>,
			delivery_and_dispatch_fee: &BalanceOf<ThisChain<B>>,
			_lane: &LaneId,
			payload: &FromThisChainMessagePayload<B>,
		) -> Result<(), Self::Error> {
			let minimal_fee_in_bridged_tokens = estimate_message_dispatch_and_delivery_fee::<B>(
				payload,
				B::RELAYER_INTEREST_PERCENT,
			).ok_or("Overflow when computing minimal required message delivery and dispatch fee")?;

			// compare with actual fee paid
			let actual_fee_in_bridged_tokens = B::this_chain_balance_to_bridged_chain_balance(
				*delivery_and_dispatch_fee,
			);
			if actual_fee_in_bridged_tokens < minimal_fee_in_bridged_tokens {
				return Err("Too low fee paid");
			}

			Ok(())
		}
	}

	/// Estimate delivery and dispatch fee that must be paid for delivering a message to the Bridged chain.
	///
	/// The fee is paid in This chain Balance, but we use Bridged chain balance to avoid additional conversions.
	/// Returns `None` if overflow has happened.
	pub fn estimate_message_dispatch_and_delivery_fee<B: MessageBridge>(
		payload: &FromThisChainMessagePayload<B>,
		relayer_interest_percent: u32,
	) -> Option<BalanceOf<BridgedChain<B>>> {
		// the fee (in Bridged tokens) of all transactions that are made on the Bridged chain
		let delivery_fee = B::bridged_weight_to_balance(B::weight_of_delivery_transaction());
		let dispatch_fee = B::bridged_weight_to_balance(payload.weight.into());
		let reward_confirmation_fee =
			B::bridged_weight_to_balance(B::weight_of_reward_confirmation_transaction_on_target_chain());

		// the fee (in Bridged tokens) of all transactions that are made on This chain
		let delivery_confirmation_fee = B::this_chain_balance_to_bridged_chain_balance(B::this_weight_to_balance(
			B::weight_of_delivery_confirmation_transaction_on_this_chain(),
		));

		// minimal fee (in Bridged tokens) is a sum of all required fees
		let minimal_fee = delivery_fee
			.checked_add(&dispatch_fee)
			.and_then(|fee| fee.checked_add(&reward_confirmation_fee))
			.and_then(|fee| fee.checked_add(&delivery_confirmation_fee));

		// before returning, add extra fee that is paid to the relayer (relayer interest)
		minimal_fee.and_then(|fee|
			// having message with fee that is near the `Balance::MAX_VALUE` of the chain is
			// unlikely and should be treated as an error
			// => let's do multiplication first
			fee
				.checked_mul(&relayer_interest_percent.into())
				.and_then(|interest| interest.checked_div(&100u32.into()))
				.and_then(|interest| fee.checked_add(&interest)))
	}
}

/// Sub-module that is declaring types required for processing Bridged -> This chain messages.
pub mod target {
	use super::*;

	/// Call origin for Bridged -> This chain messages.
	pub type FromBridgedChainMessageCallOrigin<B> = pallet_bridge_call_dispatch::CallOrigin<
		<BridgedChain<B> as ChainWithMessageLanes>::Signer,
		<ThisChain<B> as ChainWithMessageLanes>::Signer,
		<ThisChain<B> as ChainWithMessageLanes>::Signature,
	>;

	/// Decoded Bridged -> This message payload.
	pub type FromBridgedChainDecodedMessagePayload<B> = pallet_bridge_call_dispatch::MessagePayload<
		<BridgedChain<B> as ChainWithMessageLanes>::Signer,
		<ThisChain<B> as ChainWithMessageLanes>::Signer,
		<ThisChain<B> as ChainWithMessageLanes>::Signature,
		<ThisChain<B> as ChainWithMessageLanes>::Call,
	>;

	/// Message payload for Bridged -> This messages.
	pub struct FromBridgedChainMessagePayload<B: MessageBridge>(FromBridgedChainDecodedMessagePayload<B>);

	impl<B: MessageBridge> Decode for FromBridgedChainMessagePayload<B> {
		fn decode<I: Input>(input: &mut I) -> Result<Self, codec::Error> {
			// for bridged chain our Calls are opaque - they're encoded to Vec<u8> by submitter
			// => skip encoded vec length here before decoding Call
			let spec_version = pallet_bridge_call_dispatch::SpecVersion::decode(input)?;
			let weight = frame_support::weights::Weight::decode(input)?;
			let origin = FromBridgedChainMessageCallOrigin::<B>::decode(input)?;
			let _skipped_length = Compact::<u32>::decode(input)?;
			let call = CallOf::<ThisChain<B>>::decode(input)?;

			Ok(FromBridgedChainMessagePayload(
				pallet_bridge_call_dispatch::MessagePayload {
					spec_version,
					weight,
					origin,
					call,
				},
			))
		}
	}

	/// Dispatching Bridged -> This chain messages.
	#[derive(RuntimeDebug, Clone, Copy)]
	pub struct FromBridgedChainMessageDispatch<B, ThisRuntime, ThisCallDispatchInstance> {
		_marker: sp_std::marker::PhantomData<(B, ThisRuntime, ThisCallDispatchInstance)>,
	}

	impl<B: MessageBridge, ThisRuntime, ThisCallDispatchInstance>
		MessageDispatch<<BridgedChain<B> as ChainWithMessageLanes>::Balance>
		for FromBridgedChainMessageDispatch<B, ThisRuntime, ThisCallDispatchInstance>
	where
		ThisCallDispatchInstance: frame_support::traits::Instance,
		ThisRuntime: pallet_bridge_call_dispatch::Trait<ThisCallDispatchInstance>,
		pallet_bridge_call_dispatch::Module<ThisRuntime, ThisCallDispatchInstance>:
			bp_message_dispatch::MessageDispatch<
				(LaneId, MessageNonce),
				Message = FromBridgedChainDecodedMessagePayload<B>,
			>,
	{
		type DispatchPayload = FromBridgedChainMessagePayload<B>;

		fn dispatch_weight(
			message: &DispatchMessage<Self::DispatchPayload, BalanceOf<BridgedChain<B>>>,
		) -> frame_support::weights::Weight {
			message
				.data
				.payload
				.as_ref()
				.map(|payload| payload.0.weight)
				.unwrap_or(0)
		}

		fn dispatch(message: DispatchMessage<Self::DispatchPayload, BalanceOf<BridgedChain<B>>>) {
			if let Ok(payload) = message.data.payload {
				pallet_bridge_call_dispatch::Module::<ThisRuntime, ThisCallDispatchInstance>::dispatch(
					B::INSTANCE,
					(message.key.lane_id, message.key.nonce),
					payload.0,
				);
			}
		}
	}
}
