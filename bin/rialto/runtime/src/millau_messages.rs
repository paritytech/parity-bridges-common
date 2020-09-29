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

//! Everything required to serve Millau <-> Rialto message lanes.

use bp_message_dispatch::MessageDispatch as _;
use bp_message_lane::{
	source_chain::{LaneMessageVerifier, TargetHeaderChain},
	target_chain::{DispatchMessage, MessageDispatch, ProvedMessages, SourceHeaderChain},
	InboundLaneData, LaneId, Message, MessageNonce,
};
use codec::{Compact, Decode, Input};
use frame_support::{
	weights::{Weight, WeightToFeePolynomial},
	RuntimeDebug,
};
use sp_std::vec::Vec;
use sp_trie::StorageProof;

/// Millau bridge instance id.
pub const INSTANCE: bp_runtime::InstanceId = *b"mllu";

/// Encoded Call of Millau chain.
pub type OpaqueMillauCall = Vec<u8>;

/// Message payload for Rialto -> Millau messages.
pub type ToMillauMessagePayload = pallet_bridge_call_dispatch::MessagePayload<
	bp_rialto::AccountSigner,
	bp_millau::AccountSigner,
	bp_millau::Signature,
	OpaqueMillauCall,
>;

/// Call origin for Millau -> Rialto messages.
pub type FromMillauMessageCallOrigin =
	pallet_bridge_call_dispatch::CallOrigin<bp_millau::AccountSigner, bp_rialto::AccountSigner, bp_rialto::Signature>;

/// Message payload for Millau -> Rialto messages.
pub struct FromMillauMessagePayload(
	pallet_bridge_call_dispatch::MessagePayload<
		bp_millau::AccountSigner,
		bp_rialto::AccountSigner,
		bp_rialto::Signature,
		crate::Call,
	>,
);

impl Decode for FromMillauMessagePayload {
	fn decode<I: Input>(input: &mut I) -> Result<Self, codec::Error> {
		// for bridged chain our Calls are opaque - they're encoded to Vec<u8> by submitter
		// => skip encoded vec length here before decoding Call
		let spec_version = pallet_bridge_call_dispatch::SpecVersion::decode(input)?;
		let weight = Weight::decode(input)?;
		let origin = FromMillauMessageCallOrigin::decode(input)?;
		let _skipped_length = Compact::<u32>::decode(input)?;
		let call = crate::Call::decode(input)?;

		Ok(FromMillauMessagePayload(pallet_bridge_call_dispatch::MessagePayload {
			spec_version,
			weight,
			origin,
			call,
		}))
	}
}

/// Millau chain from message lane point of view.
#[derive(RuntimeDebug, Clone, Copy)]
pub struct Millau;

impl TargetHeaderChain<ToMillauMessagePayload, bp_rialto::AccountId> for Millau {
	type Error = &'static str;
	// The proof is:
	// - hash of the header this proof has been created with;
	// - the storage proof or one or several keys;
	// - id of the lane we prove state of.
	type MessagesDeliveryProof = (bp_millau::Hash, StorageProof, LaneId);

	fn verify_message(payload: &ToMillauMessagePayload) -> Result<(), Self::Error> {
		if payload.weight > dynamic::maximal_dispatch_weight_of_millau_message() {
			return Err("Too large weight declared");
		}

		Ok(())
	}

	fn verify_messages_delivery_proof(
		_proof: Self::MessagesDeliveryProof,
	) -> Result<(LaneId, InboundLaneData<bp_millau::AccountId>), Self::Error> {
		unimplemented!("https://github.com/paritytech/parity-bridges-common/issues/397")
	}
}

impl LaneMessageVerifier<bp_rialto::AccountId, ToMillauMessagePayload, bp_rialto::Balance> for Millau {
	type Error = &'static str;

	fn verify_message(
		_submitter: &bp_rialto::AccountId,
		delivery_and_dispatch_fee: &bp_rialto::Balance,
		_lane: &LaneId,
		payload: &ToMillauMessagePayload,
	) -> Result<(), Self::Error> {
		// compute all components of total fee in Millau tokens
		let millau_delivery_base_fee = dynamic::millau_weight_to_fee(dynamic::millau_delivery_base_weight());
		let millau_dispatch_fee = dynamic::millau_weight_to_fee(payload.weight);
		let millau_confirmation_fee = dynamic::rialto_balance_into_millau_balance(
			<crate::Runtime as pallet_transaction_payment::Trait>::WeightToFee::calc(
				&dynamic::rialto_confirmation_weight(),
			),
		);

		// compute total expected fee and add relayers interest (10%)
		let millau_minimal_fee = millau_delivery_base_fee + millau_dispatch_fee + millau_confirmation_fee;
		let millau_minimal_expected_fee = millau_minimal_fee + millau_minimal_fee / 10;

		// compare with actual fee paid
		let millau_actual_fee = dynamic::rialto_balance_into_millau_balance(*delivery_and_dispatch_fee);
		if millau_actual_fee < millau_minimal_expected_fee {
			return Err("Too low fee paid");
		}

		Ok(())
	}
}

impl SourceHeaderChain<bp_millau::Balance> for Millau {
	type Error = &'static str;
	// The proof is:
	// - hash of the header this proof has been created with;
	// - the storage proof or one or several keys;
	// - id of the lane we prove messages for;
	// - inclusive range of messages nonces that are proved.
	type MessagesProof = (bp_millau::Hash, StorageProof, LaneId, MessageNonce, MessageNonce);

	fn verify_messages_proof(
		_proof: Self::MessagesProof,
	) -> Result<ProvedMessages<Message<bp_millau::Balance>>, Self::Error> {
		unimplemented!("https://github.com/paritytech/parity-bridges-common/issues/397")
	}
}

/// Call-dispatch based message dispatch for Millau -> Rialto messages.
#[derive(RuntimeDebug, Clone, Copy)]
pub struct FromMillauMessageDispatch;

impl MessageDispatch<bp_millau::Balance> for FromMillauMessageDispatch {
	type DispatchPayload = FromMillauMessagePayload;

	fn dispatch_weight(message: &DispatchMessage<Self::DispatchPayload, bp_millau::Balance>) -> Weight {
		message
			.data
			.payload
			.as_ref()
			.map(|payload| payload.0.weight)
			.unwrap_or(0)
	}

	fn dispatch(message: DispatchMessage<Self::DispatchPayload, bp_millau::Balance>) {
		if let Ok(payload) = message.data.payload {
			pallet_bridge_call_dispatch::Module::<crate::Runtime>::dispatch(
				INSTANCE,
				(message.key.lane_id, message.key.nonce),
				payload.0,
			);
		}
	}
}

mod dynamic {
	use frame_support::weights::{Weight, WeightToFeePolynomial};

	/// Maximal dispatch weight of the call we are able to dispatch.
	pub fn maximal_dispatch_weight_of_millau_message() -> Weight {
		// this should be maximal weight on Millau minus millau_delivery_base_weight()
		// or even less to support future upgrardes
		1_000_000_000_000 - millau_delivery_base_weight()
	}

	/// Return maximal weight of delivery confirmation transaction on Rialto chain.
	pub fn rialto_confirmation_weight() -> Weight {
		1_000_000
	}

	/// Return maximal base weight of delivery transaction on Millau chain.
	pub fn millau_delivery_base_weight() -> Weight {
		1_000_000
	}

	/// Estimate cost (in Millau tokens) of dispatching call with given weight on Millau chain.
	pub fn millau_weight_to_fee(weight: Weight) -> bp_millau::Balance {
		<crate::Runtime as pallet_transaction_payment::Trait>::WeightToFee::calc(&weight)
	}

	/// Convert from Rialto to Millau fee tokens.
	pub fn rialto_balance_into_millau_balance(balance: bp_rialto::Balance) -> bp_millau::Balance {
		balance * 10
	}
}
