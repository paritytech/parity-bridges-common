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

use crate::{Runtime, WithRialtoParachainMessagesInstance};

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

/// Call-dispatch based message dispatch for RialtoParachain -> Millau messages.
pub type FromRialtoParachainMessageDispatch =
	bridge_runtime_common::messages_xcm_extension::XcmBlobMessageDispatch<
		crate::xcm_config::OnMillauBlobDispatcher,
		(),
	>;

/// Export XCM messages to be relayed to Rialto.
pub type ToRialtoParachainBlobExporter = HaulBlobExporter<
	XcmBlobHaulerAdapter<ToRialtoParachainXcmBlobHauler>,
	crate::xcm_config::RialtoParachainNetwork,
	(),
>;

/// To-RialtoParachain XCM hauler.
pub struct ToRialtoParachainXcmBlobHauler;

impl XcmBlobHauler for ToRialtoParachainXcmBlobHauler {
	type MessageSender =
		pallet_bridge_messages::Pallet<Runtime, WithRialtoParachainMessagesInstance>;

	fn xcm_lane() -> LaneId {
		LaneIdFromChainId::<Runtime, WithRialtoParachainMessagesInstance>::get()
	}
}

impl pallet_bridge_messages::WeightInfoExt
	for crate::weights::RialtoParachainMessagesWeightInfo<Runtime>
{
	fn expected_extra_storage_proof_size() -> u32 {
		bp_rialto_parachain::EXTRA_STORAGE_PROOF_SIZE
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
	use crate::{
		PriorityBoostPerMessage, RialtoGrandpaInstance, Runtime,
		WithRialtoParachainMessagesInstance,
	};

	use bridge_runtime_common::{
		assert_complete_bridge_types,
		integrity::{
			assert_complete_with_parachain_bridge_constants, check_message_lane_weights,
			AssertChainConstants, AssertCompleteBridgeConstants,
		},
	};

	#[test]
	fn ensure_millau_message_lane_weights_are_correct() {
		check_message_lane_weights::<bp_millau::Millau, Runtime, WithRialtoParachainMessagesInstance>(
			bp_rialto_parachain::EXTRA_STORAGE_PROOF_SIZE,
			bp_millau::MAX_UNREWARDED_RELAYERS_IN_CONFIRMATION_TX,
			bp_millau::MAX_UNCONFIRMED_MESSAGES_IN_CONFIRMATION_TX,
			true,
		);
	}

	#[test]
	fn ensure_bridge_integrity() {
		assert_complete_bridge_types!(
			runtime: Runtime,
			with_bridged_chain_grandpa_instance: RialtoGrandpaInstance,
			with_bridged_chain_messages_instance: WithRialtoParachainMessagesInstance,
			this_chain: bp_millau::Millau,
			bridged_chain: bp_rialto_parachain::RialtoParachain,
		);

		assert_complete_with_parachain_bridge_constants::<
			Runtime,
			RialtoGrandpaInstance,
			WithRialtoParachainMessagesInstance,
			bp_rialto::Rialto,
		>(AssertCompleteBridgeConstants {
			this_chain_constants: AssertChainConstants {
				block_length: bp_millau::BlockLength::get(),
				block_weights: bp_millau::BlockWeights::get(),
			},
		});

		bridge_runtime_common::priority_calculator::ensure_priority_boost_is_sane::<
			Runtime,
			WithRialtoParachainMessagesInstance,
			PriorityBoostPerMessage,
		>(1_000_000);
	}

	#[test]
	fn rialto_parachain_millau_bridge_identifier_did_not_changed() {
		// there's nothing criminal if it is changed, but then thou need to fix it across
		// all deployments scripts, alerts and so on
		assert_eq!(
			*ToRialtoParachainXcmBlobHauler::xcm_lane().as_ref(),
			hex_literal::hex!("6aa61bff567db6b5d5f0cb815ee6d8f5ac630e222a95700cb3d594134e3805de")
				.into(),
		);
	}
}
