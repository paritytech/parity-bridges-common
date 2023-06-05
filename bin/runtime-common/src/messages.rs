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

//! Types that allow runtime to act as a source/target endpoint of message lanes.
//!
//! Messages are assumed to be encoded `Call`s of the target chain. Call-dispatch
//! pallet is used to dispatch incoming messages. Message identified by a tuple
//! of to elements - message lane id and message nonce.

use bp_header_chain::HeaderChain;
use bp_messages::{
	source_chain::{FromBridgedChainMessagesDeliveryProof, TargetHeaderChain},
	target_chain::FromBridgedChainMessagesProof,
	InboundLaneData, LaneId, VerificationError,
};
pub use bp_runtime::{
	Chain, RangeInclusiveExt, RawStorageProof, Size, TrustedVecDb, UnderlyingChainOf,
	UnderlyingChainProvider, UntrustedVecDb,
};
use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

/// Bidirectional message bridge.
pub trait MessageBridge {
	/// Name of the paired messages pallet instance at the Bridged chain.
	///
	/// Should be the name that is used in the `construct_runtime!()` macro.
	const BRIDGED_MESSAGES_PALLET_NAME: &'static str;

	/// This chain in context of message bridge.
	type ThisChain: ThisChainWithMessages;
	/// Bridged chain in context of message bridge.
	type BridgedChain: BridgedChainWithMessages;
	/// Bridged header chain.
	type BridgedHeaderChain: HeaderChain<UnderlyingChainOf<Self::BridgedChain>>;
}

/// This chain that has `pallet-bridge-messages` module.
pub trait ThisChainWithMessages: UnderlyingChainProvider {
	/// Call origin on the chain.
	type RuntimeOrigin;
}

/// Bridged chain that has `pallet-bridge-messages` module.
pub trait BridgedChainWithMessages: UnderlyingChainProvider {}

/// This chain in context of message bridge.
pub type ThisChain<B> = <B as MessageBridge>::ThisChain;
/// Bridged chain in context of message bridge.
pub type BridgedChain<B> = <B as MessageBridge>::BridgedChain;
/// Hash used on the chain.
pub type HashOf<C> = bp_runtime::HashOf<<C as UnderlyingChainProvider>::Chain>;
/// Hasher used on the chain.
pub type HasherOf<C> = bp_runtime::HasherOf<UnderlyingChainOf<C>>;
/// Account id used on the chain.
pub type AccountIdOf<C> = bp_runtime::AccountIdOf<UnderlyingChainOf<C>>;
/// Type of balances that is used on the chain.
pub type BalanceOf<C> = bp_runtime::BalanceOf<UnderlyingChainOf<C>>;
/// Type of origin that is used on the chain.
pub type OriginOf<C> = <C as ThisChainWithMessages>::RuntimeOrigin;

/// Sub-module that is declaring types required for processing This -> Bridged chain messages.
pub mod source {
	use super::*;

	/// Message payload for This -> Bridged chain messages.
	pub type FromThisChainMessagePayload = crate::messages_xcm_extension::XcmAsPlainPayload;

	/// Maximal size of outbound message payload.
	pub struct FromThisChainMaximalOutboundPayloadSize<B>(PhantomData<B>);

	impl<B: MessageBridge> Get<u32> for FromThisChainMaximalOutboundPayloadSize<B> {
		fn get() -> u32 {
			maximal_message_size::<B>()
		}
	}

	/// 'Parsed' message delivery proof - inbound lane id and its state.
	pub type ParsedMessagesDeliveryProofFromBridgedChain<B> =
		(LaneId, InboundLaneData<AccountIdOf<ThisChain<B>>>);

	/// Return maximal message size of This -> Bridged chain message.
	pub fn maximal_message_size<B: MessageBridge>() -> u32 {
		super::target::maximal_incoming_message_size(
			UnderlyingChainOf::<BridgedChain<B>>::max_extrinsic_size(),
		)
	}

	/// `TargetHeaderChain` implementation that is using default types and perform default checks.
	pub struct TargetHeaderChainAdapter<B>(PhantomData<B>);

	impl<B: MessageBridge> TargetHeaderChain<FromThisChainMessagePayload, AccountIdOf<ThisChain<B>>>
		for TargetHeaderChainAdapter<B>
	{
		type MessagesDeliveryProof = FromBridgedChainMessagesDeliveryProof<HashOf<BridgedChain<B>>>;

		fn verify_message(payload: &FromThisChainMessagePayload) -> Result<(), VerificationError> {
			verify_chain_message::<B>(payload)
		}

		fn verify_messages_delivery_proof(
			proof: Self::MessagesDeliveryProof,
		) -> Result<(LaneId, InboundLaneData<AccountIdOf<ThisChain<B>>>), VerificationError> {
			verify_messages_delivery_proof::<B>(proof)
		}
	}

	/// Do basic Bridged-chain specific verification of This -> Bridged chain message.
	///
	/// Ok result from this function means that the delivery transaction with this message
	/// may be 'mined' by the target chain.
	pub fn verify_chain_message<B: MessageBridge>(
		payload: &FromThisChainMessagePayload,
	) -> Result<(), VerificationError> {
		// IMPORTANT: any error that is returned here is fatal for the bridge, because
		// this code is executed at the bridge hub and message sender actually lives
		// at some sibling parachain. So we are failing **after** the message has been
		// sent and we can't report it back to sender (unless error report mechanism is
		// embedded into message and its dispatcher).

		// apart from maximal message size check (see below), we should also check the message
		// dispatch weight here. But we assume that the bridged chain will just push the message
		// to some queue (XCMP, UMP, DMP), so the weight is constant and fits the block.

		// The maximal size of extrinsic at Substrate-based chain depends on the
		// `frame_system::Config::MaximumBlockLength` and
		// `frame_system::Config::AvailableBlockRatio` constants. This check is here to be sure that
		// the lane won't stuck because message is too large to fit into delivery transaction.
		//
		// **IMPORTANT NOTE**: the delivery transaction contains storage proof of the message, not
		// the message itself. The proof is always larger than the message. But unless chain state
		// is enormously large, it should be several dozens/hundreds of bytes. The delivery
		// transaction also contains signatures and signed extensions. Because of this, we reserve
		// 1/3 of the the maximal extrinsic size for this data.
		if payload.len() > maximal_message_size::<B>() as usize {
			return Err(VerificationError::MessageTooLarge)
		}

		Ok(())
	}

	/// Verify proof of This -> Bridged chain messages delivery.
	///
	/// This function is used when Bridged chain is directly using GRANDPA finality. For Bridged
	/// parachains, please use the `verify_messages_delivery_proof_from_parachain`.
	pub fn verify_messages_delivery_proof<B: MessageBridge>(
		proof: FromBridgedChainMessagesDeliveryProof<HashOf<BridgedChain<B>>>,
	) -> Result<ParsedMessagesDeliveryProofFromBridgedChain<B>, VerificationError> {
		let FromBridgedChainMessagesDeliveryProof { bridged_header_hash, storage_proof, lane } =
			proof;
		let mut storage =
			B::BridgedHeaderChain::verify_vec_db_storage(bridged_header_hash, storage_proof)
				.map_err(VerificationError::HeaderChain)?;
		// Messages delivery proof is just proof of single storage key read => any error
		// is fatal.
		let storage_inbound_lane_data_key = bp_messages::storage_keys::inbound_lane_data_key(
			B::BRIDGED_MESSAGES_PALLET_NAME,
			&lane,
		);
		let inbound_lane_data = storage
			.get_and_decode_mandatory(&storage_inbound_lane_data_key)
			.map_err(VerificationError::InboundLaneStorage)?;

		// check that the storage proof doesn't have any untouched trie nodes
		storage.ensure_no_unused_keys().map_err(VerificationError::VecDb)?;

		Ok((lane, inbound_lane_data))
	}
}

/// Sub-module that is declaring types required for processing Bridged -> This chain messages.
pub mod target {
	use super::*;

	/// Decoded Bridged -> This message payload.
	pub type FromBridgedChainMessagePayload = crate::messages_xcm_extension::XcmAsPlainPayload;

	/// Return maximal dispatch weight of the message we're able to receive.
	pub fn maximal_incoming_message_dispatch_weight(maximal_extrinsic_weight: Weight) -> Weight {
		maximal_extrinsic_weight / 2
	}

	/// Return maximal message size given maximal extrinsic size.
	pub fn maximal_incoming_message_size(maximal_extrinsic_size: u32) -> u32 {
		maximal_extrinsic_size / 3 * 2
	}
}

/// The `BridgeMessagesCall` used by a chain.
pub type BridgeMessagesCallOf<C> = bp_messages::BridgeMessagesCall<
	bp_runtime::AccountIdOf<C>,
	FromBridgedChainMessagesProof<bp_runtime::HashOf<C>>,
	bp_messages::source_chain::FromBridgedChainMessagesDeliveryProof<bp_runtime::HashOf<C>>,
>;

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::*;

	#[test]
	fn verify_chain_message_rejects_message_with_too_large_declared_weight() {
		assert!(source::verify_chain_message::<OnThisChainBridge>(&vec![
			42;
			BRIDGED_CHAIN_MAX_EXTRINSIC_WEIGHT -
				1
		])
		.is_err());
	}

	#[test]
	fn verify_chain_message_rejects_message_too_large_message() {
		assert!(source::verify_chain_message::<OnThisChainBridge>(&vec![
			0;
			source::maximal_message_size::<OnThisChainBridge>()
				as usize + 1
		],)
		.is_err());
	}

	#[test]
	fn verify_chain_message_accepts_maximal_message() {
		assert_eq!(
			source::verify_chain_message::<OnThisChainBridge>(&vec![
				0;
				source::maximal_message_size::<OnThisChainBridge>()
					as _
			],),
			Ok(()),
		);
	}
}
