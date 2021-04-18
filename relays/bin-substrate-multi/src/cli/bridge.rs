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
		CircuitToGateway,
		GatewayToCircuit,
	}
}

impl FullBridge {
	/// Return instance index of the bridge pallet in source runtime.
	pub fn bridge_instance_index(&self) -> u8 {
		match self {
			Self::CircuitToGateway => CIRCUIT_TO_GATEWAY_INDEX,
			Self::GatewayToCircuit => GATEWAY_TO_CIRCUIT_INDEX,
		}
	}
}

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
