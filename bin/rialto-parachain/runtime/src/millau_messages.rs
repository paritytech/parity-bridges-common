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

use bp_xcm_bridge_hub::BridgeId;
use frame_support::parameter_types;
use xcm::prelude::*;

parameter_types! {
	/// Bridge identifier that is used to bridge with Millau.
	pub Bridge: BridgeId = BridgeId::new(
		&InteriorMultiLocation::from(crate::ThisNetwork::get()).into(),
		&InteriorMultiLocation::from(crate::MillauNetwork::get()).into(),
	);
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{MillauGrandpaInstance, Runtime, WithMillauMessagesInstance};
	use bridge_runtime_common::{
		assert_complete_bridge_types,
		integrity::{
			assert_complete_with_relay_chain_bridge_constants, check_message_lane_weights,
			AssertChainConstants, AssertCompleteBridgeConstants,
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
			this_chain: bp_rialto_parachain::RialtoParachain,
			bridged_chain: bp_millau::Millau,
		);

		assert_complete_with_relay_chain_bridge_constants::<
			Runtime,
			MillauGrandpaInstance,
			WithMillauMessagesInstance,
		>(AssertCompleteBridgeConstants {
			this_chain_constants: AssertChainConstants {
				block_length: bp_rialto_parachain::BlockLength::get(),
				block_weights: bp_rialto_parachain::BlockWeights::get(),
			},
		});
	}

	#[test]
	fn rialto_parachain_millau_bridge_identifier_did_not_changed() {
		// there's nothing criminal if it is changed, but then thou need to fix it across
		// all deployments scripts, alerts and so on
		assert_eq!(
			*Bridge::get().lane_id().as_ref(),
			hex_literal::hex!("ee7158d2a51c3c43853ced550cc25bd00eb2662b231b1ddbb92e495ec882969c")
				.into(),
		);
	}
}
