// Copyright 2019-2020 Parity Technologies (UK) Ltd.
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

//! Submitting Ethereum -> Substrate exchange transactions.

use crate::ethereum_client::{EthereumConnectionParams, EthereumRpcClient, EthereumSigningParams};
use crate::ethereum_types::{CallRequest, U256};
use crate::rpc::EthereumRpc;

use bridge_node_runtime::exchange::LOCK_FUNDS_ADDRESS;
use hex_literal::hex;
use sp_bridge_eth_poa::{
	signatures::{SecretKey, SignTransaction},
	UnsignedTransaction,
};

/// Ethereum exchange transaction params.
#[derive(Debug)]
pub struct EthereumExchangeSubmitParams {
	/// Ethereum connection params.
	pub eth: EthereumConnectionParams,
	/// Ethereum signing params.
	pub eth_sign: EthereumSigningParams,
	/// Amount of Ethereum tokens to lock.
	pub eth_amount: U256,
	/// Funds recipient on Substrate side.
	pub sub_recipient: [u8; 32],
}

impl Default for EthereumExchangeSubmitParams {
	fn default() -> Self {
		EthereumExchangeSubmitParams {
			eth: Default::default(),
			eth_sign: Default::default(),
			eth_amount: 1_000_000_000_000_000_000_u64.into(), // 1 ETH
			sub_recipient: hex!("1cbd2d43530a44705ad088af313e18f80b53ef16b36177cd4b77b846f2a5f07c"), // ferdie
		}
	}
}

/// Submit single Ethereum -> Substrate exchange transaction.
pub fn run(params: EthereumExchangeSubmitParams) {
	let mut local_pool = futures::executor::LocalPool::new();

	let result: Result<_, String> = local_pool.run_until(async move {
		let eth_client = EthereumRpcClient::new(params.eth);

		let eth_signer_address = params.eth_sign.signer.address();
		let sub_recipient_encoded = params.sub_recipient;
		let nonce = eth_client
			.account_nonce(eth_signer_address)
			.await
			.map_err(|err| format!("error fetching acount nonce: {:?}", err))?;
		let gas = eth_client
			.estimate_gas(CallRequest {
				from: Some(eth_signer_address),
				to: Some(LOCK_FUNDS_ADDRESS.into()),
				value: Some(params.eth_amount),
				data: Some(sub_recipient_encoded.to_vec().into()),
				..Default::default()
			})
			.await
			.map_err(|err| format!("error estimating gas requirements: {:?}", err))?;
		let eth_tx_unsigned = UnsignedTransaction {
			nonce,
			gas_price: params.eth_sign.gas_price,
			gas,
			to: Some(LOCK_FUNDS_ADDRESS.into()),
			value: params.eth_amount,
			payload: sub_recipient_encoded.to_vec(),
		};
		let eth_tx_signed = eth_tx_unsigned.clone().sign_by(
			&SecretKey::parse(params.eth_sign.signer.secret().as_fixed_bytes())
				.expect("key is accepted by secp256k1::KeyPair and thus is valid; qed"),
			Some(params.eth_sign.chain_id),
		);
		eth_client
			.submit_transaction(eth_tx_signed)
			.await
			.map_err(|err| format!("error submitting transaction: {:?}", err))?;

		Ok(eth_tx_unsigned)
	});

	match result {
		Ok(eth_tx_unsigned) => {
			log::info!(
				target: "bridge",
				"Exchange transaction has been submitted to Ethereum node: {:?}",
				eth_tx_unsigned,
			);
		}
		Err(err) => {
			log::error!(
				target: "bridge",
				"Error submitting exchange transaction to Ethereum node: {}",
				err,
			);
		}
	}
}
