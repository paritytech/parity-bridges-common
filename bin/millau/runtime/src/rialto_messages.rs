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

//! Everything required to serve Millau <-> Rialto messages.

use crate::{Runtime, WithRialtoMessagesInstance};

use bp_messages::LaneId;
use bridge_runtime_common::messages_xcm_extension::{
	LaneIdFromChainId, XcmBlobHauler, XcmBlobHaulerAdapter,
};
use frame_support::{parameter_types, weights::Weight};
use pallet_bridge_relayers::WeightInfoExt as _;
use sp_core::Get;
use xcm_builder::HaulBlobExporter;

/// Weight of 2 XCM instructions is for simple `Trap(42)` program, coming through bridge
/// (it is prepended with `UniversalOrigin` instruction). It is used just for simplest manual
/// tests, confirming that we don't break encoding somewhere between.
pub const BASE_XCM_WEIGHT_TWICE: Weight = crate::xcm_config::BaseXcmWeight::get().saturating_mul(2);

parameter_types! {
	/// Weight credit for our test messages.
	///
	/// 2 XCM instructions is for simple `Trap(42)` program, coming through bridge
	/// (it is prepended with `UniversalOrigin` instruction).
	pub const WeightCredit: Weight = BASE_XCM_WEIGHT_TWICE;
}

/// Call-dispatch based message dispatch for Rialto -> Millau messages.
pub type FromRialtoMessageDispatch =
	bridge_runtime_common::messages_xcm_extension::XcmBlobMessageDispatch<
		crate::xcm_config::OnMillauBlobDispatcher,
		(),
	>;

/// Export XCM messages to be relayed to Rialto.
pub type ToRialtoBlobExporter = HaulBlobExporter<
	XcmBlobHaulerAdapter<ToRialtoXcmBlobHauler>,
	crate::xcm_config::RialtoNetwork,
	(),
>;

/// To-Rialto XCM hauler.
pub struct ToRialtoXcmBlobHauler;

impl XcmBlobHauler for ToRialtoXcmBlobHauler {
	type MessageSender = pallet_bridge_messages::Pallet<Runtime, WithRialtoMessagesInstance>;

	fn xcm_lane() -> LaneId {
		LaneIdFromChainId::<Runtime, WithRialtoMessagesInstance>::get()
	}
}

impl pallet_bridge_messages::WeightInfoExt for crate::weights::RialtoMessagesWeightInfo<Runtime> {
	fn expected_extra_storage_proof_size() -> u32 {
		bp_rialto::EXTRA_STORAGE_PROOF_SIZE
	}

	fn receive_messages_proof_overhead_from_runtime() -> Weight {
		pallet_bridge_relayers::weights::BridgeWeight::<Runtime>::receive_messages_proof_overhead_from_runtime()
	}

	fn receive_messages_delivery_proof_overhead_from_runtime() -> Weight {
		pallet_bridge_relayers::weights::BridgeWeight::<Runtime>::receive_messages_delivery_proof_overhead_from_runtime()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{RialtoGrandpaInstance, Runtime, WithRialtoMessagesInstance};

	use bridge_runtime_common::{
		assert_complete_bridge_types,
		integrity::{
			assert_complete_with_relay_chain_bridge_constants, check_message_lane_weights,
			AssertChainConstants, AssertCompleteBridgeConstants,
		},
	};

	#[test]
	fn ensure_millau_message_lane_weights_are_correct() {
		check_message_lane_weights::<bp_millau::Millau, Runtime, WithRialtoMessagesInstance>(
			bp_rialto::EXTRA_STORAGE_PROOF_SIZE,
			bp_millau::MAX_UNREWARDED_RELAYERS_IN_CONFIRMATION_TX,
			bp_millau::MAX_UNCONFIRMED_MESSAGES_IN_CONFIRMATION_TX,
			false,
		);
	}

	#[test]
	fn ensure_bridge_integrity() {
		assert_complete_bridge_types!(
			runtime: Runtime,
			with_bridged_chain_grandpa_instance: RialtoGrandpaInstance,
			with_bridged_chain_messages_instance: WithRialtoMessagesInstance,
			this_chain: bp_millau::Millau,
			bridged_chain: bp_rialto::Rialto,
		);

		assert_complete_with_relay_chain_bridge_constants::<
			Runtime,
			RialtoGrandpaInstance,
			WithRialtoMessagesInstance,
		>(AssertCompleteBridgeConstants {
			this_chain_constants: AssertChainConstants {
				block_length: bp_millau::BlockLength::get(),
				block_weights: bp_millau::BlockWeights::get(),
			},
		});
	}

	#[test]
	fn rialto_millau_bridge_identifier_did_not_changed() {
		// there's nothing criminal if it is changed, but then thou need to fix it across
		// all deployments scripts, alerts and so on
		assert_eq!(
			*ToRialtoXcmBlobHauler::xcm_lane().as_ref(),
			hex_literal::hex!("52011894c856c0c613a2ad2395dfbb509090f6b7a6aef9359adb75aa26a586c7")
				.into(),
		);
	}
}
