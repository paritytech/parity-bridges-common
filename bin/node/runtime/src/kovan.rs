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

#[derive(Encode, Decode, RuntimeDebug)]
pub struct EthereumTransactionTag {
	pub account: [u8; 20],
	pub nonce: u64,
}

pub struct KovanBlockchain;

impl pallet_bridge_eth_poa_exchange::Blockchain for KovanBlockchain {
	type BlockHash = sp_core::H256;
	type Transaction = Vec<u8>;
	type TransactionInclusionProof = Vec<Self::Transaction>;

	fn verify_transaction_inclusion_proof(
		transaction: &Self::Transaction,
		block: Self::BlockHash,
		proof: &Self::TransactionInclusionProof,
	) {
		unimplemented!()
	}
}

pub struct KovanTransaction;

impl pallet_bridge_eth_poa_exchange::MaybeLockFundsTransaction for KovanTransaction {
	type Transaction = Vec<u8>;
	type Id = EthereumTransactionTag;
	type Recipient = crate::AccountId;
	type Amount = crate::Balance;

	fn parse(tx: &Self::Transaction) -> Option<pallet_bridge_eth_poa_exchange::LockFundsTransaction<Self::Id, Self::Recipient, Self::Amount>> {
		let tx_rlp = Rlp::new(&tx);
		let nonce = tx_rlp.val_at(0)?;
		let value = tx_rlp.val_at(4)?;
		let account_id = tx_rlp.val_at(5);
		
	}
}
