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

use codec::{Decode, Encode};
use frame_support::RuntimeDebug;
use pallet_bridge_currency_exchange::Blockchain;
use sp_bridge_eth_poa::{
	exchange::{Error as ExchangeError, LockFundsTransaction, MaybeLockFundsTransaction, Result as ExchangeResult},
	transaction_decode,
};
use sp_std::vec::Vec;

/// Address where locked PoA funds must be sent to (0xDEADBEEFDEADBEEFDEADBEEFDEADBEEFDEADBEEF).
const LOCK_FUNDS_ADDRESS: [u8; 20] = [
	0xDE, 0xAD, 0xBE, 0xEF, 0xDE, 0xAD, 0xBE, 0xEF, 0xDE, 0xAD, 0xBE, 0xEF, 0xDE, 0xAD, 0xBE, 0xEF, 0xDE, 0xAD, 0xDE,
	0xAD,
];

/// We're uniquely identify transfer by pair (sender, nonce).
#[derive(Encode, Decode, PartialEq, RuntimeDebug)]
pub struct EthereumTransactionTag {
	/// Account that has locked funds.
	pub account: [u8; 20],
	/// Lock transaction nonce.
	pub nonce: sp_core::U256,
}

/// Kovan blockchain from runtime perspective.
pub struct KovanBlockchain;

impl Blockchain for KovanBlockchain {
	type BlockHash = sp_core::H256;
	type Transaction = Vec<u8>;
	type TransactionInclusionProof = Vec<Self::Transaction>;

	fn verify_transaction_inclusion_proof(
		transaction: &Self::Transaction,
		block: Self::BlockHash,
		proof: &Self::TransactionInclusionProof,
	) -> bool {
		crate::BridgeEthPoA::verify_transaction_finalized(transaction, block, proof)
	}
}

/// Kovan transaction from runtime perspective.
pub struct KovanTransaction;

impl MaybeLockFundsTransaction for KovanTransaction {
	type Transaction = Vec<u8>;
	type Id = EthereumTransactionTag;
	type Recipient = crate::AccountId;
	type Amount = crate::Balance;

	fn parse(
		raw_tx: &Self::Transaction,
	) -> ExchangeResult<LockFundsTransaction<Self::Id, Self::Recipient, Self::Amount>> {
		let tx = transaction_decode(raw_tx).map_err(|_| ExchangeError::InvalidTransaction)?;

		// we only accept transactions sending funds to pre-configured address
		if tx.to != Some(LOCK_FUNDS_ADDRESS.into()) {
			return Err(ExchangeError::InvalidTransaction);
		}

		let mut recipient_raw = sp_core::H256::default();
		match tx.payload.len() {
			32 => recipient_raw.as_fixed_bytes_mut().copy_from_slice(&tx.payload),
			_ => return Err(ExchangeError::InvalidRecipient),
		}
		let amount = tx.value.low_u128();

		if tx.value != amount.into() {
			return Err(ExchangeError::InvalidAmount);
		}

		Ok(LockFundsTransaction {
			id: EthereumTransactionTag {
				account: *tx.sender.as_fixed_bytes(),
				nonce: tx.nonce,
			},
			recipient: crate::AccountId::from(*recipient_raw.as_fixed_bytes()),
			amount,
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use hex_literal::hex;

	#[test]
	fn fund_locks_transaction_decode_works() {
		// prepare tx for OpenEthereum private dev chain:
		// chain id is 0x11
		// sender secret is 0x4d5db4107d237df6a3d58ee5f70ae63d73d7658d4026f2eefd2f204c81682cb7
		let chain_id = 0x11_u64;
		let signer = hex!("4d5db4107d237df6a3d58ee5f70ae63d73d7658d4026f2eefd2f204c81682cb7");
		let signer_addr = hex!("00a329c0648769a73afac7f9381e08fb43dbea72");
		let ferdie_id: crate::AccountId =
			hex!("1cbd2d43530a44705ad088af313e18f80b53ef16b36177cd4b77b846f2a5f07c").into();
		let ferdie_raw: &[u8; 32] = ferdie_id.as_ref();
		let signed_tx = ethereum_tx_sign::RawTransaction {
			nonce: 0.into(),
			to: Some(LOCK_FUNDS_ADDRESS.into()),
			value: 100.into(),
			gas: 100_000.into(),
			gas_price: 100_000.into(),
			data: ferdie_raw.to_vec(),
		}
		.sign(&signer.into(), &chain_id);

		assert_eq!(
			KovanTransaction::parse(&signed_tx),
			Ok(LockFundsTransaction {
				id: EthereumTransactionTag {
					account: signer_addr,
					nonce: 0.into(),
				},
				recipient: ferdie_id,
				amount: 100,
			}),
		);
	}
}
