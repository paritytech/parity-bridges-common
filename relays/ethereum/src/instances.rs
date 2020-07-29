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

use crate::ethereum_types::QueuedEthereumHeader;
use crate::substrate_types::{into_substrate_ethereum_header, into_substrate_ethereum_receipts};

use bridge_node_runtime::exchange::EthereumTransactionInclusionProof as Proof;
use bridge_node_runtime::Call;

pub trait BridgeInstance: Send + Sync {
	fn build_signed_header_call(&self, headers: Vec<QueuedEthereumHeader>) -> Call;
	fn build_unsigned_header_call(&self, header: QueuedEthereumHeader) -> Call;
	fn build_currency_exchange_call(&self, proof: Proof) -> Call;
}

#[derive(Default, Clone, Debug)]
pub struct RialtoInstance;

impl RialtoInstance {
	pub fn new() -> Self {
		Self
	}
}

impl BridgeInstance for RialtoInstance {
	fn build_signed_header_call(&self, headers: Vec<QueuedEthereumHeader>) -> Call {
		let pallet_call = bridge_node_runtime::BridgeEthPoACall::import_signed_headers(
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

		bridge_node_runtime::Call::BridgeRialto(pallet_call)
	}

	fn build_unsigned_header_call(&self, header: QueuedEthereumHeader) -> Call {
		let pallet_call = bridge_node_runtime::BridgeEthPoACall::import_unsigned_header(
			into_substrate_ethereum_header(header.header()),
			into_substrate_ethereum_receipts(header.extra()),
		);

		bridge_node_runtime::Call::BridgeRialto(pallet_call)
	}

	fn build_currency_exchange_call(&self, proof: Proof) -> Call {
		let pallet_call = bridge_node_runtime::BridgeCurrencyExchangeCall::import_peer_transaction(proof);
		bridge_node_runtime::Call::BridgeRialtoCurrencyExchange(pallet_call)
	}
}

#[derive(Default, Clone, Debug)]
pub struct KovanInstance;

impl KovanInstance {
	pub fn new() -> Self {
		Self
	}
}

impl BridgeInstance for KovanInstance {
	fn build_signed_header_call(&self, headers: Vec<QueuedEthereumHeader>) -> Call {
		let pallet_call = bridge_node_runtime::BridgeEthPoACall::import_signed_headers(
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

		bridge_node_runtime::Call::BridgeKovan(pallet_call)
	}

	fn build_unsigned_header_call(&self, header: QueuedEthereumHeader) -> Call {
		let pallet_call = bridge_node_runtime::BridgeEthPoACall::import_unsigned_header(
			into_substrate_ethereum_header(header.header()),
			into_substrate_ethereum_receipts(header.extra()),
		);

		bridge_node_runtime::Call::BridgeKovan(pallet_call)
	}

	fn build_currency_exchange_call(&self, proof: Proof) -> Call {
		let pallet_call = bridge_node_runtime::BridgeCurrencyExchangeCall::import_peer_transaction(proof);
		bridge_node_runtime::Call::BridgeKovanCurrencyExchange(pallet_call)
	}
}
