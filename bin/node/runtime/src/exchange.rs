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
use hex_literal::hex;
use pallet_bridge_currency_exchange::Blockchain;
use sp_currency_exchange::{Error as ExchangeError, LockFundsTransaction, MaybeLockFundsTransaction, Result as ExchangeResult};
use sp_bridge_eth_poa::transaction_decode;
use sp_std::vec::Vec;

/// Address where locked PoA funds must be sent to.
const LOCK_FUNDS_ADDRESS: [u8; 20] = hex!("DEADBEEFDEADBEEFDEADBEEFDEADBEEFDEADBEEF");

/// Ethereum transaction inclusion proof.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug)]
pub struct EthereumTransactionInclusionProof {
	/// Hash of the block with transaction.
	pub block: sp_core::H256,
	/// Index of the transaction within the block.
	pub index: u64,
	/// The proof itself (right now it is all RLP-encoded transactions of the block).
	pub proof: Vec<Vec<u8>>,
}

/// We uniquely identify transfer by the pair (sender, nonce).
///
/// The assumption is that this pair will never appear more than once in
/// transactions included into finalized blocks. This is obviously true
/// for any existing eth-like chain (that keep current tx format), because
/// otherwise transaction can be replayed over and over.
#[derive(Encode, Decode, PartialEq, RuntimeDebug)]
pub struct EthereumTransactionTag {
	/// Account that has locked funds.
	pub account: [u8; 20],
	/// Lock transaction nonce.
	pub nonce: sp_core::U256,
}

/// Eth blockchain from runtime perspective.
pub struct EthBlockchain;

impl Blockchain for EthBlockchain {
	type Transaction = Vec<u8>;
	type TransactionInclusionProof = EthereumTransactionInclusionProof;

	fn verify_transaction_inclusion_proof(proof: &Self::TransactionInclusionProof) -> Option<Self::Transaction> {
		let is_transaction_finalized =
			crate::BridgeEthPoA::verify_transaction_finalized(proof.block, proof.index, &proof.proof);

		if !is_transaction_finalized {
			return None;
		}

		proof.proof.get(proof.index as usize).cloned()
	}
}

/// Eth transaction from runtime perspective.
pub struct EthTransaction;

impl MaybeLockFundsTransaction for EthTransaction {
	type Transaction = Vec<u8>;
	type Id = EthereumTransactionTag;
	type Recipient = crate::AccountId;
	type Amount = crate::Balance;

	fn parse(
		raw_tx: &Self::Transaction,
	) -> ExchangeResult<LockFundsTransaction<Self::Id, Self::Recipient, Self::Amount>> {
		let tx = transaction_decode(raw_tx).map_err(|_| ExchangeError::InvalidTransaction)?;

		// we only accept transactions sending funds directly to the pre-configured address
		if tx.to != Some(LOCK_FUNDS_ADDRESS.into()) {
			frame_support::debug::error!(
				target: "runtime",
				"Failed to parse fund locks transaction. Invalid peer recipient: {:?}",
				tx.to,
			);

			return Err(ExchangeError::InvalidTransaction);
		}

		let mut recipient_raw = sp_core::H256::default();
		match tx.payload.len() {
			32 => recipient_raw.as_fixed_bytes_mut().copy_from_slice(&tx.payload),
			len => {
				frame_support::debug::error!(
					target: "runtime",
					"Failed to parse fund locks transaction. Invalid recipient length: {}",
					len,
				);

				return Err(ExchangeError::InvalidRecipient);
			}
		}
		let amount = tx.value.low_u128();

		if tx.value != amount.into() {
			frame_support::debug::error!(
				target: "runtime",
				"Failed to parse fund locks transaction. Invalid amount: {}",
				tx.value,
			);

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

	fn ferdie() -> crate::AccountId {
		hex!("1cbd2d43530a44705ad088af313e18f80b53ef16b36177cd4b77b846f2a5f07c").into()
	}

	fn prepare_ethereum_transaction(editor: impl Fn(&mut ethereum_tx_sign::RawTransaction)) -> Vec<u8> {
		// prepare tx for OpenEthereum private dev chain:
		// chain id is 0x11
		// sender secret is 0x4d5db4107d237df6a3d58ee5f70ae63d73d7658d4026f2eefd2f204c81682cb7
		let chain_id = 0x11_u64;
		let signer = hex!("4d5db4107d237df6a3d58ee5f70ae63d73d7658d4026f2eefd2f204c81682cb7");
		let ferdie_id = ferdie();
		let ferdie_raw: &[u8; 32] = ferdie_id.as_ref();
		let mut eth_tx = ethereum_tx_sign::RawTransaction {
			nonce: 0.into(),
			to: Some(LOCK_FUNDS_ADDRESS.into()),
			value: 100.into(),
			gas: 100_000.into(),
			gas_price: 100_000.into(),
			data: ferdie_raw.to_vec(),
		};
		editor(&mut eth_tx);
		eth_tx.sign(&signer.into(), &chain_id)
	}

	#[test]
	fn valid_transaction_accepted() {
		assert_eq!(
			EthTransaction::parse(&prepare_ethereum_transaction(|_| {})),
			Ok(LockFundsTransaction {
				id: EthereumTransactionTag {
					account: hex!("00a329c0648769a73afac7f9381e08fb43dbea72"),
					nonce: 0.into(),
				},
				recipient: ferdie(),
				amount: 100,
			}),
		);
	}

	#[test]
	fn invalid_transaction_rejected() {
		assert_eq!(
			EthTransaction::parse(&Vec::new()),
			Err(ExchangeError::InvalidTransaction),
		);
	}

	#[test]
	fn invalid_with_invalid_peer_recipient_rejected() {
		assert_eq!(
			EthTransaction::parse(&prepare_ethereum_transaction(|tx| {
				tx.to = None;
			})),
			Err(ExchangeError::InvalidTransaction),
		);
	}

	#[test]
	fn invalid_with_invalid_recipient_rejected() {
		assert_eq!(
			EthTransaction::parse(&prepare_ethereum_transaction(|tx| {
				tx.data.clear();
			})),
			Err(ExchangeError::InvalidRecipient),
		);
	}

	#[test]
	fn invalid_with_invalid_amount_rejected() {
		assert_eq!(
			EthTransaction::parse(&prepare_ethereum_transaction(|tx| {
				tx.value = sp_core::U256::from(u128::max_value()) + sp_core::U256::from(1);
			})),
			Err(ExchangeError::InvalidAmount),
		);
	}
}
