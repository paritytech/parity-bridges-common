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
use frame_support::{Parameter, decl_module, decl_storage, ensure};
use sp_std::{vec::Vec, marker::PhantomData};

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
	);
}

/// Peer blockchain lock funds transaction parser.
pub trait MaybeLockFundsTransaction {
	/// Transaction type.
	type Transaction;
	/// Identifier that uniquely identifies this transfer.
	type Id: Encode;
	/// Peer recipient type.
	type Recipient;
	/// Peer currency amount type.
	type Amount;

	/// Parse lock funds transaction of the peer blockchain. Returns None if
	/// transaction format is unknown, or it isn't a lock funds transaction.
	fn parse(tx: &Self::Transaction) -> Option<LockFundsTransaction<Self::Id, Self::Recipient, Self::Amount>>;
}

/// Map that associates peer blockchain recipient with current blockchain recipient.
pub trait RecipientsMap {
	/// Peer blockchain recipient type.
	type PeerRecipient;
	/// Current blockchain recipient type.
	type Recipient;

	/// Lookup current blockchain recipient by peer blockchain recipient.
	fn map(peer_recipient: Self::PeerRecipient) -> Option<Self::Recipient>;
}

/// Conversion between two currencies.
pub trait CurrencyConverter {
	/// Type of the source currency amount.
	type SourceAmount;
	/// Type of the target currency amount.
	type TargetAmount;

	/// Covert from source to target currency.
	fn convert(currency: Self::SourceAmount) -> Option<Self::TargetAmount>;
}

/// Currency airdrop.
pub trait Airdrop {
	/// Recipient type.
	type Recipient;
	/// Currency amount type.
	type Amount;

	/// Grant some money to given account.
	fn drop(recipient: Self::Recipient, amount: Self::Amount) -> Result<(), &'static str>;
}

/// Recipients map which is used when accounts ids are the same on both chains.
pub struct AsIsRecipients<AccountId>(PhantomData<AccountId>);

impl<AccountId> RecipientsMap for AsIsRecipients<AccountId> {
	type PeerRecipient = AccountId;
	type Recipient = AccountId;

	fn map(peer_recipient: Self::PeerRecipient) -> Option<Self::Recipient> {
		Some(peer_recipient)
	}
}

/// Currency converter which is used when currency is the same on both chains.
pub struct AsIsCurrencyConverter<Amount>(PhantomData<Amount>);

impl<Amount> CurrencyConverter for AsIsCurrencyConverter<Amount> {
	type SourceAmount = Amount;
	type TargetAmount = Amount;

	fn convert(currency: Self::SourceAmount) -> Option<Self::TargetAmount> {
		Some(currency)
	}
}

/// Lock funds transaction type.
pub struct LockFundsTransaction<TransferId, Recipient, Amount> {
	/// Something that uniquely identifies this transfer.
	pub id: TransferId,
	/// Funds recipient on the peer chain.
	pub recipient: Recipient,
	/// Amount of the locked funds.
	pub amount: Amount,
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

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		/// Imports lock fund transaction of the peer blockchain.
		#[weight = 0] // TODO: update me (https://github.com/paritytech/parity-bridges-common/issues/78)
		pub fn import_peer_transaction(
			origin,
			transaction: <<T as Trait>::PeerBlockchain as Blockchain>::Transaction,
		) {
			frame_system::ensure_signed(origin)?;

			let transaction = <T as Trait>::PeerMaybeLockFundsTransaction::parse(&transaction)
				.ok_or_else(|| "Unknown transaction type")?;
			let transfer_id = transaction.id.encode();
			ensure!(
				!Transfers::contains_key(&transfer_id),
				"Already claimed"
			);

			let recipient = T::RecipientsMap::map(transaction.recipient)
				.ok_or_else(|| "Failed to lookup recipient")?;
			let amount = T::CurrencyConverter::convert(transaction.amount)
				.ok_or_else(|| "Failed to map between currencies")?;
			T::Airdrop::drop(recipient, amount)?;

			Transfers::insert(transfer_id, ());
		}
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as Bridge {
		/// All transfers that have already been claimed.
		Transfers: map hasher(blake2_128_concat) Vec<u8> => ();
	}
}
