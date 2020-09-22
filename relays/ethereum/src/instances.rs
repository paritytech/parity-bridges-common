// Copyright 2020 Parity Technologies (UK) Ltd.
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

//! The PoA Bridge Pallet provides a way to include multiple instances of itself in a runtime. When
//! synchronizing a Substrate chain which can include multiple instances of the bridge pallet we
//! must somehow decide which of the instances to sync.
//!
//! Note that each instance of the bridge pallet is coupled with an instance of the currency exchange
//! pallet. We must also have a way to create `Call`s for the correct currency exchange instance.
//!
//! This module helps by preparing the correct `Call`s for each of the different pallet instances.

use crate::ethereum_sync_loop::QueuedEthereumHeader;
use crate::substrate_types::{into_substrate_ethereum_header, into_substrate_ethereum_receipts};

use rialto_runtime::exchange::EthereumTransactionInclusionProof as Proof;
use rialto_runtime::Call;

/// Interface for `Calls` which are needed to correctly sync the bridge.
///
/// Each instance of the bridge and currency exchange pallets in the bridge runtime requires similar
/// but slightly different `Call` in order to be synchronized.
#[derive(Debug, Clone, Copy)]
pub enum BridgeInstance {
	/// Rialto-Substrate <-> Rialto-TestPoA bridge.
	TestPoA,
	/// Rialto-Substrate <-> Kovan bridge.
	Kovan,
}

impl BridgeInstance {
	pub fn build_signed_header_call(&self, headers: Vec<QueuedEthereumHeader>) -> Call {
		match *self {
			Self::TestPoA => {
				let pallet_call = rialto_runtime::BridgeEthPoACall::import_signed_headers(
					headers
						.into_iter()
						.map(|header| {
							(
								into_substrate_ethereum_header(&header.header()),
								into_substrate_ethereum_receipts(header.extra()),
							)
						})
						.collect(),
				);

				rialto_runtime::Call::BridgeRialto(pallet_call)
			}
			Self::Kovan => {
				let pallet_call = rialto_runtime::BridgeEthPoACall::import_signed_headers(
					headers
						.into_iter()
						.map(|header| {
							(
								into_substrate_ethereum_header(header.header()),
								into_substrate_ethereum_receipts(header.extra()),
							)
						})
						.collect(),
				);

				rialto_runtime::Call::BridgeKovan(pallet_call)
			}
		}
	}

	pub fn build_unsigned_header_call(&self, header: QueuedEthereumHeader) -> Call {
		match *self {
			Self::TestPoA => {
				let pallet_call = rialto_runtime::BridgeEthPoACall::import_unsigned_header(
					into_substrate_ethereum_header(&header.header()),
					into_substrate_ethereum_receipts(header.extra()),
				);

				rialto_runtime::Call::BridgeRialto(pallet_call)
			}
			Self::Kovan => {
				let pallet_call = rialto_runtime::BridgeEthPoACall::import_unsigned_header(
					into_substrate_ethereum_header(header.header()),
					into_substrate_ethereum_receipts(header.extra()),
				);

				rialto_runtime::Call::BridgeKovan(pallet_call)
			}
		}
	}

	pub fn build_currency_exchange_call(&self, proof: Proof) -> Call {
		match *self {
			Self::TestPoA => {
				let pallet_call = rialto_runtime::BridgeCurrencyExchangeCall::import_peer_transaction(proof);
				rialto_runtime::Call::BridgeRialtoCurrencyExchange(pallet_call)
			}
			Self::Kovan => {
				let pallet_call = rialto_runtime::BridgeCurrencyExchangeCall::import_peer_transaction(proof);
				rialto_runtime::Call::BridgeKovanCurrencyExchange(pallet_call)
			}
		}
	}
}
