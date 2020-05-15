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

#![cfg_attr(not(feature = "std"), no_std)]

use codec::Encode;
use frame_support::{Parameter, decl_error, decl_module, decl_storage, ensure, fail};
use sp_runtime::DispatchResult;
use sp_std::vec::Vec;
use primitives::exchange::{
	Error as ExchangeError, MaybeLockFundsTransaction,
	RecipientsMap, CurrencyConverter, Airdrop,
};

/// Peer blockhain interface.
pub trait Blockchain {
	/// Block hash type.
	type BlockHash: Parameter;
	/// Transaction type.
	type Transaction: Parameter;
	/// Transaction inclusion proof type.
	type TransactionInclusionProof: Parameter;

	/// Verify that transaction is a part of given block.
	fn verify_transaction_inclusion_proof(
		transaction: &Self::Transaction,
		block: Self::BlockHash,
		proof: &Self::TransactionInclusionProof,
	) -> bool;
}

/// The module configuration trait
pub trait Trait: frame_system::Trait {
	/// Peer blockchain type.
	type PeerBlockchain: Blockchain;
	/// Peer blockchain transaction parser.
	type PeerMaybeLockFundsTransaction: MaybeLockFundsTransaction<
		Transaction = <Self::PeerBlockchain as Blockchain>::Transaction,
	>;
	/// Map between blockchains recipients.
	type RecipientsMap: RecipientsMap<
		PeerRecipient = <Self::PeerMaybeLockFundsTransaction as MaybeLockFundsTransaction>::Recipient,
		Recipient = Self::AccountId,
	>;
	/// This blockchain currency amount type.
	type Amount;
	/// Converter from peer blockchain currency type into current blockchain currency type.
	type CurrencyConverter: CurrencyConverter<
		SourceAmount = <Self::PeerMaybeLockFundsTransaction as MaybeLockFundsTransaction>::Amount,
		TargetAmount = Self::Amount,
	>;
	/// Something that could grant money.
	type Airdrop: Airdrop<
		Recipient = Self::AccountId,
		Amount = Self::Amount,
	>;
}

decl_error! {
	pub enum Error for Module<T: Trait> {
		/// Invalid peer blockchain transaction provided.
		InvalidTransaction,
		/// Peer transaction has invalid amount.
		InvalidAmount,
		/// Peer transaction has invalid recipient.
		InvalidRecipient,
		/// Cannot map from peer recipient to this blockchain recipient.
		FailedToMapRecipients,
		/// Failed to convert from peer blockchain currency to this blockhain currency.
		FailedToCovertCurrency,
		/// Airdrop has failed.
		AirdropFailed,
		/// Transaction is not finalized.
		UnfinalizedTransaction,
		/// Transaction funds are already claimed.
		AlreadyClaimed,
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		/// Imports lock fund transaction of the peer blockchain.
		#[weight = 0] // TODO: update me (https://github.com/paritytech/parity-bridges-common/issues/78)
		pub fn import_peer_transaction(
			origin,
			transaction: <<T as Trait>::PeerBlockchain as Blockchain>::Transaction,
			block: <<T as Trait>::PeerBlockchain as Blockchain>::BlockHash,
			proof: <<T as Trait>::PeerBlockchain as Blockchain>::TransactionInclusionProof,
		) -> DispatchResult {
			frame_system::ensure_signed(origin)?;

			// ensure that transaction is included in finalized block that we know of
			//
			// note that the transaction itself is actually redundant here, because it
			// will probably be a part of proof itself (like it is now), or could be
			// reconstructed from proof during verification
			//
			// leaving it here for now just for simplicity
			let is_transaction_finalized = <T as Trait>::PeerBlockchain::verify_transaction_inclusion_proof(
				&transaction,
				block,
				&proof,
			);
			if !is_transaction_finalized {
				fail!(Error::<T>::UnfinalizedTransaction);
			}

			// parse transaction
			let transaction = <T as Trait>::PeerMaybeLockFundsTransaction::parse(&transaction)
				.map_err(Error::<T>::from)?;
			let transfer_id = transaction.id.encode();
			ensure!(
				!Transfers::contains_key(&transfer_id),
				Error::<T>::AlreadyClaimed
			);

			// grant recipient
			let recipient = T::RecipientsMap::map(transaction.recipient).map_err(Error::<T>::from)?;
			let amount = T::CurrencyConverter::convert(transaction.amount).map_err(Error::<T>::from)?;
			T::Airdrop::drop(recipient, amount).map_err(Error::<T>::from)?;

			// remember that we have accepted this transfer
			Transfers::insert(transfer_id, ());

			Ok(())
		}
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as Bridge {
		/// All transfers that have already been claimed.
		Transfers: map hasher(blake2_128_concat) Vec<u8> => ();
	}
}

impl<T: Trait> From<ExchangeError> for Error<T> {
	fn from(error: ExchangeError) -> Self {
		match error {
			ExchangeError::InvalidTransaction => Error::InvalidTransaction,
			ExchangeError::InvalidAmount => Error::InvalidAmount,
			ExchangeError::InvalidRecipient => Error::InvalidRecipient,
			ExchangeError::FailedToMapRecipients => Error::FailedToMapRecipients,
			ExchangeError::FailedToCovertCurrency => Error::FailedToCovertCurrency,
			ExchangeError::AirdropFailed => Error::AirdropFailed,
		}
	}
}
