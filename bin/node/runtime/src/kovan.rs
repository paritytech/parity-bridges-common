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

use codec::{Encode, Decode};
use frame_support::RuntimeDebug;
use sp_std::vec::Vec;
use pallet_bridge_eth_poa_exchange::Blockchain;
use sp_bridge_eth_poa::{
	rlp_decode, transaction_decode,
	exchange::{
		MaybeLockFundsTransaction, LockFundsTransaction,
		Error as ExchangeError, Result as ExchangeResult,
	},
};

/// Address where locked PoA funds must be sent to (0xDEADBEEFDEADBEEFDEADBEEFDEADBEEFDEADBEEF).
const LOCK_FUNDS_ADDRESS: [u8; 20] = [
	0xDE, 0xAD, 0xBE, 0xEF, 0xDE, 0xAD, 0xBE, 0xEF, 0xDE, 0xAD,
	0xBE, 0xEF, 0xDE, 0xAD, 0xBE, 0xEF, 0xDE, 0xAD, 0xDE, 0xAD,
];

/// We're uniquely identify transfer by pair (sender, nonce).
#[derive(Encode, Decode, RuntimeDebug)]
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
		crate::BridgeEthPoA::verify_transaction_included(
			transaction,
			block,
			proof,
		)
	}
}

/// Kovan transaction from runtime perspective.
pub struct KovanTransaction;

impl MaybeLockFundsTransaction for KovanTransaction {
	type Transaction = Vec<u8>;
	type Id = EthereumTransactionTag;
	type Recipient = crate::AccountId;
	type Amount = crate::Balance;

	fn parse(raw_tx: &Self::Transaction) -> ExchangeResult<LockFundsTransaction<Self::Id, Self::Recipient, Self::Amount>> {
		let tx = transaction_decode(raw_tx).map_err(|_| ExchangeError::InvalidTransaction)?;

		// we only accept transactions sending funds to pre-configured address
		if tx.to != Some(LOCK_FUNDS_ADDRESS.into()) {
			return Err(ExchangeError::InvalidTransaction);
		}

		let recipient: sp_core::H256 = rlp_decode(&tx.payload).map_err(|_| ExchangeError::InvalidRecipient)?;
		let amount = tx.value.low_u128();

		if tx.value != amount.into() {
			return Err(ExchangeError::InvalidAmount);
		}

		Ok(LockFundsTransaction {
			id: EthereumTransactionTag {
				account: *tx.sender.as_fixed_bytes(),
				nonce: tx.nonce,
			},
			recipient: crate::AccountId::from(*recipient.as_fixed_bytes()),
			amount,
		})
	}
}
