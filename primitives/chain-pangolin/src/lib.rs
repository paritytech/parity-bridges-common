// This file is part of Darwinia.
//
// Copyright (C) 2018-2022 Darwinia Network
// SPDX-License-Identifier: GPL-3.0
//
// Darwinia is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Darwinia is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Darwinia. If not, see <https://www.gnu.org/licenses/>.

#![cfg_attr(not(feature = "std"), no_std)]

mod copy_paste_from_darwinia {
	// --- crates.io ---
	use smallvec::smallvec;
	// --- darwinia-network ---
	use bp_darwinia_core::*;
	// --- paritytech ---
	use frame_support::weights::{
		constants::ExtrinsicBaseWeight, WeightToFeeCoefficient, WeightToFeeCoefficients,
		WeightToFeePolynomial,
	};
	use sp_runtime::Perbill;
	use sp_version::RuntimeVersion;

	pub const VERSION: RuntimeVersion = RuntimeVersion {
		spec_name: sp_runtime::create_runtime_str!("Pangolin"),
		impl_name: sp_runtime::create_runtime_str!("Pangolin"),
		authoring_version: 0,
		spec_version: 2_8_06_0,
		impl_version: 0,
		apis: sp_version::create_apis_vec![[]],
		transaction_version: 0,
		state_version: 0,
	};

	pub const TRANSACTION_BYTE_FEE: Balance = 5 * MILLI;
	pub const EXISTENTIAL_DEPOSIT: Balance = 0;

	pub const SESSION_LENGTH: BlockNumber = 30 * MINUTES;

	pub struct WeightToFee;
	impl WeightToFeePolynomial for WeightToFee {
		type Balance = Balance;

		fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
			let p = 100 * MILLI;
			let q = Balance::from(ExtrinsicBaseWeight::get());

			smallvec![WeightToFeeCoefficient {
				degree: 1,
				negative: false,
				coeff_frac: Perbill::from_rational(p % q, q),
				coeff_integer: p / q,
			}]
		}
	}
}
pub use copy_paste_from_darwinia::*;

pub use bp_darwinia_core::*;

// --- paritytech ---
use bp_messages::{LaneId, MessageDetails, MessageNonce};
use frame_support::Parameter;
use sp_runtime::FixedU128;
use sp_std::prelude::*;

/// Pangolin Chain.
pub type Pangolin = DarwiniaLike;

/// Name of the With-Pangolin GRANDPA pallet instance that is deployed at bridged chains.
pub const WITH_PANGOLIN_GRANDPA_PALLET_NAME: &str = "BridgePangolinGrandpa";
/// Name of the With-Pangolin messages pallet instance that is deployed at bridged chains.
pub const WITH_PANGOLIN_MESSAGES_PALLET_NAME: &str = "BridgePangolinMessages";

/// Name of the transaction payment pallet at the Pangolin runtime.
pub const TRANSACTION_PAYMENT_PALLET_NAME: &str = "TransactionPayment";

/// Name of the DOT->KSM conversion rate stored in the Pangolin runtime.
pub const POLKADOT_TO_PANGOLIN_CONVERSION_RATE_PARAMETER_NAME: &str =
	"PolkadotToPangolinConversionRate";
/// Name of the Polkadot fee multiplier parameter, stored in the Polkadot runtime.
pub const POLKADOT_FEE_MULTIPLIER_PARAMETER_NAME: &str = "PolkadotFeeMultiplier";

/// Name of the `PangolinFinalityApi::best_finalized` runtime method.
pub const BEST_FINALIZED_PANGOLIN_HEADER_METHOD: &str = "PangolinFinalityApi_best_finalized";

/// Name of the `ToPangolinOutboundLaneApi::estimate_message_delivery_and_dispatch_fee` runtime
/// method.
pub const TO_PANGOLIN_ESTIMATE_MESSAGE_FEE_METHOD: &str =
	"ToPangolinOutboundLaneApi_estimate_message_delivery_and_dispatch_fee";
/// Name of the `ToPangolinOutboundLaneApi::message_details` runtime method.
pub const TO_PANGOLIN_MESSAGE_DETAILS_METHOD: &str = "ToPangolinOutboundLaneApi_message_details";

sp_api::decl_runtime_apis! {
	/// API for querying information about the finalized Pangolin headers.
	///
	/// This API is implemented by runtimes that are bridging with the Pangolin chain, not the
	/// Pangolin runtime itself.
	pub trait PangolinFinalityApi {
		/// Returns number and hash of the best finalized header known to the bridge module.
		fn best_finalized() -> (BlockNumber, Hash);
	}

	/// Outbound message lane API for messages that are sent to Pangolin chain.
	///
	/// This API is implemented by runtimes that are sending messages to Pangolin chain, not the
	/// Pangolin runtime itself.
	pub trait ToPangolinOutboundLaneApi<OutboundMessageFee: Parameter, OutboundPayload: Parameter> {
		/// Estimate message delivery and dispatch fee that needs to be paid by the sender on
		/// this chain.
		///
		/// Returns `None` if message is too expensive to be sent to Pangolin from this chain.
		///
		/// Please keep in mind that this method returns the lowest message fee required for message
		/// to be accepted to the lane. It may be good idea to pay a bit over this price to account
		/// future exchange rate changes and guarantee that relayer would deliver your message
		/// to the target chain.
		fn estimate_message_delivery_and_dispatch_fee(
			lane_id: LaneId,
			payload: OutboundPayload,
			kusama_to_this_conversion_rate: Option<FixedU128>,
		) -> Option<OutboundMessageFee>;
		/// Returns dispatch weight, encoded payload size and delivery+dispatch fee of all
		/// messages in given inclusive range.
		///
		/// If some (or all) messages are missing from the storage, they'll also will
		/// be missing from the resulting vector. The vector is ordered by the nonce.
		fn message_details(
			lane: LaneId,
			begin: MessageNonce,
			end: MessageNonce,
		) -> Vec<MessageDetails<OutboundMessageFee>>;
	}
}
