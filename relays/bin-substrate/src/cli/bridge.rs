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

use structopt::clap::arg_enum;

arg_enum! {
	#[derive(Debug, PartialEq, Eq)]
	/// Supported full bridges (headers + messages).
	pub enum FullBridge {
		MillauToRialto,
		RialtoToMillau,
		CircuitToGateway,
		GatewayToCircuit,
	}
}

impl FullBridge {
	/// Return instance index of the bridge pallet in source runtime.
	pub fn bridge_instance_index(&self) -> u8 {
		match self {
			Self::MillauToRialto => MILLAU_TO_RIALTO_INDEX,
			Self::RialtoToMillau => RIALTO_TO_MILLAU_INDEX,
			Self::CircuitToGateway => CIRCUIT_TO_GATEWAY_INDEX,
			Self::GatewayToCircuit => GATEWAY_TO_CIRCUIT_INDEX,
		}
	}
}

pub const RIALTO_TO_MILLAU_INDEX: u8 = 0;
pub const MILLAU_TO_RIALTO_INDEX: u8 = 0;

pub const GATEWAY_TO_CIRCUIT_INDEX: u8 = 0;
pub const CIRCUIT_TO_GATEWAY_INDEX: u8 = 0;

/// The macro allows executing bridge-specific code without going fully generic.
///
/// It matches on the [`FullBridge`] enum, sets bridge-specific types or imports and injects
/// the `$generic` code at every variant.
#[macro_export]
macro_rules! select_full_bridge {
	($bridge: expr, $generic: tt) => {
		match $bridge {
			FullBridge::MillauToRialto => {
				type Source = relay_millau_client::Millau;
				#[allow(dead_code)]
				type Target = relay_rialto_client::Rialto;

				// Derive-account
				#[allow(unused_imports)]
				use bp_millau::derive_account_from_rialto_id as derive_account;

				// Relay-messages
				#[allow(unused_imports)]
				use crate::chains::millau_messages_to_rialto::run as relay_messages;

				// Send-message / Estimate-fee
				#[allow(unused_imports)]
				use bp_rialto::TO_RIALTO_ESTIMATE_MESSAGE_FEE_METHOD as ESTIMATE_MESSAGE_FEE_METHOD;
				// Send-message
				#[allow(unused_imports)]
				use millau_runtime::rialto_account_ownership_digest as account_ownership_digest;

				$generic
			}
			FullBridge::RialtoToMillau => {
				type Source = relay_rialto_client::Rialto;
				#[allow(dead_code)]
				type Target = relay_millau_client::Millau;

				// Derive-account
				#[allow(unused_imports)]
				use bp_rialto::derive_account_from_millau_id as derive_account;

				// Relay-messages
				#[allow(unused_imports)]
				use crate::chains::rialto_messages_to_millau::run as relay_messages;

				// Send-message / Estimate-fee
				#[allow(unused_imports)]
				use bp_millau::TO_MILLAU_ESTIMATE_MESSAGE_FEE_METHOD as ESTIMATE_MESSAGE_FEE_METHOD;

				// Send-message
				#[allow(unused_imports)]
				use rialto_runtime::millau_account_ownership_digest as account_ownership_digest;

				$generic
			}
			FullBridge::CircuitToGateway => {
				type Source = relay_circuit_client::Circuit;
				#[allow(dead_code)]
				type Target = relay_gateway_client::Gateway;

				// Derive-account
				#[allow(unused_imports)]
				use bp_circuit::derive_account_from_gateway_id as derive_account;

				// Relay-messages
				#[allow(unused_imports)]
				use crate::chains::circuit_messages_to_gateway::run as relay_messages;

				// Send-message / Estimate-fee
				#[allow(unused_imports)]
				use bp_gateway::TO_GATEWAY_ESTIMATE_MESSAGE_FEE_METHOD as ESTIMATE_MESSAGE_FEE_METHOD;
				// Send-message
				#[allow(unused_imports)]
				use circuit_runtime::gateway_account_ownership_digest as account_ownership_digest;

				$generic
			}
			FullBridge::GatewayToCircuit => {
				type Source = relay_gateway_client::Gateway;
				#[allow(dead_code)]
				type Target = relay_circuit_client::Circuit;

				// Derive-account
				#[allow(unused_imports)]
				use bp_gateway::derive_account_from_circuit_id as derive_account;

				// Relay-messages
				#[allow(unused_imports)]
				use crate::chains::gateway_messages_to_circuit::run as relay_messages;

				// Send-message / Estimate-fee
				#[allow(unused_imports)]
				use bp_circuit::TO_CIRCUIT_ESTIMATE_MESSAGE_FEE_METHOD as ESTIMATE_MESSAGE_FEE_METHOD;

				// Send-message
				#[allow(unused_imports)]
				use gateway_runtime::circuit_account_ownership_digest as account_ownership_digest;

				$generic
			}
		}
	};
}
