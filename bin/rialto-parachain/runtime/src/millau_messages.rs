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

//! Everything required to serve Millau <-> RialtoParachain messages.

// TODO: this is almost exact copy of `millau_messages.rs` from Rialto runtime.
// Should be extracted to a separate crate and reused here.

use crate::{MillauGrandpaInstance, Runtime, RuntimeOrigin, WithMillauMessagesInstance};

use bp_messages::LaneId;
use bridge_runtime_common::{
	messages::{
		self, source::TargetHeaderChainAdapter, target::SourceHeaderChainAdapter, MessageBridge,
	},
	messages_xcm_extension::{XcmBlobHauler, XcmBlobHaulerAdapter},
};
use frame_support::{parameter_types, weights::Weight, RuntimeDebug};
use xcm::latest::prelude::*;
use xcm_builder::HaulBlobExporter;

/// Default lane that is used to send messages to Millau.
pub const XCM_LANE: LaneId = LaneId([0, 0, 0, 0]);
/// Weight of 2 XCM instructions is for simple `Trap(42)` program, coming through bridge
/// (it is prepended with `UniversalOrigin` instruction). It is used just for simplest manual
/// tests, confirming that we don't break encoding somewhere between.
pub const BASE_XCM_WEIGHT_TWICE: Weight = crate::UnitWeightCost::get().saturating_mul(2);

parameter_types! {
	/// Weight credit for our test messages.
	///
	/// 2 XCM instructions is for simple `Trap(42)` program, coming through bridge
	/// (it is prepended with `UniversalOrigin` instruction).
	pub const WeightCredit: Weight = BASE_XCM_WEIGHT_TWICE;
}

/// Message payload for RialtoParachain -> Millau messages.
pub type ToMillauMessagePayload = messages::source::FromThisChainMessagePayload;

/// Message verifier for RialtoParachain -> Millau messages.
pub type ToMillauMessageVerifier =
	messages::source::FromThisChainMessageVerifier<WithMillauMessageBridge>;

/// Message payload for Millau -> RialtoParachain messages.
pub type FromMillauMessagePayload = messages::target::FromBridgedChainMessagePayload;

/// Call-dispatch based message dispatch for Millau -> RialtoParachain messages.
pub type FromMillauMessageDispatch =
	bridge_runtime_common::messages_xcm_extension::XcmBlobMessageDispatch<
		crate::OnRialtoParachainBlobDispatcher,
		(),
		(),
	>;

/// Messages proof for Millau -> RialtoParachain messages.
pub type FromMillauMessagesProof = messages::target::FromBridgedChainMessagesProof<bp_millau::Hash>;

/// Messages delivery proof for RialtoParachain -> Millau messages.
pub type ToMillauMessagesDeliveryProof =
	messages::source::FromBridgedChainMessagesDeliveryProof<bp_millau::Hash>;

/// Maximal outbound payload size of Rialto -> Millau messages.
pub type ToMillauMaximalOutboundPayloadSize =
	messages::source::FromThisChainMaximalOutboundPayloadSize<WithMillauMessageBridge>;

/// Millau <-> RialtoParachain message bridge.
#[derive(RuntimeDebug, Clone, Copy)]
pub struct WithMillauMessageBridge;

impl MessageBridge for WithMillauMessageBridge {
	const BRIDGED_MESSAGES_PALLET_NAME: &'static str =
		bp_rialto_parachain::WITH_RIALTO_PARACHAIN_MESSAGES_PALLET_NAME;

	type ThisChain = RialtoParachain;
	type BridgedChain = Millau;
	type BridgedHeaderChain =
		pallet_bridge_grandpa::GrandpaChainHeaders<Runtime, MillauGrandpaInstance>;
}

/// RialtoParachain chain from message lane point of view.
#[derive(RuntimeDebug, Clone, Copy)]
pub struct RialtoParachain;

impl messages::UnderlyingChainProvider for RialtoParachain {
	type Chain = bp_rialto_parachain::RialtoParachain;
}

impl messages::ThisChainWithMessages for RialtoParachain {
	type RuntimeOrigin = RuntimeOrigin;
}

/// Millau chain from message lane point of view.
#[derive(RuntimeDebug, Clone, Copy)]
pub struct Millau;
/// Millau as source header chain.
pub type MillauAsSourceHeaderChain = SourceHeaderChainAdapter<WithMillauMessageBridge>;
/// Millau as target header chain.
pub type MillauAsTargetHeaderChain = TargetHeaderChainAdapter<WithMillauMessageBridge>;

impl messages::UnderlyingChainProvider for Millau {
	type Chain = bp_millau::Millau;
}

impl messages::BridgedChainWithMessages for Millau {}

/// Export XCM messages to be relayed to Millau.
pub type ToMillauBlobExporter =
	HaulBlobExporter<XcmBlobHaulerAdapter<ToMillauXcmBlobHauler>, crate::MillauNetwork, ()>;

/// To-Millau XCM hauler.
pub struct ToMillauXcmBlobHauler;

impl XcmBlobHauler for ToMillauXcmBlobHauler {
	type MessageSender = pallet_bridge_messages::Pallet<Runtime, WithMillauMessagesInstance>;
	type MessageSenderOrigin = RuntimeOrigin;

	fn message_sender_origin() -> RuntimeOrigin {
		pallet_xcm::Origin::from(MultiLocation::new(1, crate::UniversalLocation::get())).into()
	}

	fn sending_chain_location() -> MultiLocation {
		Here.into()
	}

	fn xcm_lane() -> LaneId {
		XCM_LANE
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{MillauGrandpaInstance, Runtime, WithMillauMessagesInstance};
	use bridge_runtime_common::{
		assert_complete_bridge_types,
		integrity::{
			assert_complete_bridge_constants, check_message_lane_weights,
			AssertBridgeMessagesPalletConstants, AssertBridgePalletNames, AssertChainConstants,
			AssertCompleteBridgeConstants,
		},
	};

	#[test]
	fn ensure_millau_message_lane_weights_are_correct() {
		check_message_lane_weights::<
			bp_rialto_parachain::RialtoParachain,
			Runtime,
			WithMillauMessagesInstance,
		>(
			bp_millau::EXTRA_STORAGE_PROOF_SIZE,
			bp_rialto_parachain::MAX_UNREWARDED_RELAYERS_IN_CONFIRMATION_TX,
			bp_rialto_parachain::MAX_UNCONFIRMED_MESSAGES_IN_CONFIRMATION_TX,
			false,
		);
	}

	#[test]
	fn ensure_bridge_integrity() {
		assert_complete_bridge_types!(
			runtime: Runtime,
			with_bridged_chain_grandpa_instance: MillauGrandpaInstance,
			with_bridged_chain_messages_instance: WithMillauMessagesInstance,
			bridge: WithMillauMessageBridge,
			this_chain: bp_rialto_parachain::RialtoParachain,
			bridged_chain: bp_millau::Millau,
		);

		assert_complete_bridge_constants::<
			Runtime,
			MillauGrandpaInstance,
			WithMillauMessagesInstance,
			WithMillauMessageBridge,
		>(AssertCompleteBridgeConstants {
			this_chain_constants: AssertChainConstants {
				block_length: bp_rialto_parachain::BlockLength::get(),
				block_weights: bp_rialto_parachain::BlockWeights::get(),
			},
			messages_pallet_constants: AssertBridgeMessagesPalletConstants {
				max_unrewarded_relayers_in_bridged_confirmation_tx:
					bp_millau::MAX_UNREWARDED_RELAYERS_IN_CONFIRMATION_TX,
				max_unconfirmed_messages_in_bridged_confirmation_tx:
					bp_millau::MAX_UNCONFIRMED_MESSAGES_IN_CONFIRMATION_TX,
				bridged_chain_id: bp_runtime::MILLAU_CHAIN_ID,
			},
			pallet_names: AssertBridgePalletNames {
				with_this_chain_messages_pallet_name:
					bp_rialto_parachain::WITH_RIALTO_PARACHAIN_MESSAGES_PALLET_NAME,
				with_bridged_chain_grandpa_pallet_name: bp_millau::WITH_MILLAU_GRANDPA_PALLET_NAME,
				with_bridged_chain_messages_pallet_name:
					bp_millau::WITH_MILLAU_MESSAGES_PALLET_NAME,
			},
		});
	}
}
