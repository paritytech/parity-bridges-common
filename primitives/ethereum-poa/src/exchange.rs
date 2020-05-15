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

use codec::Encode;
use sp_std::marker::PhantomData;

/// All errors that may happen during exchange.
pub enum Error {
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
}

/// Result of all exchange operations.
pub type Result<T> = sp_std::result::Result<T, Error>;

/// Peer blockchain lock funds transaction.
pub struct LockFundsTransaction<TransferId, Recipient, Amount> {
	/// Something that uniquely identifies this transfer.
	pub id: TransferId,
	/// Funds recipient on the peer chain.
	pub recipient: Recipient,
	/// Amount of the locked funds.
	pub amount: Amount,
}

/// Peer blockchain transaction that may represent lock funds transaction.
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
	fn parse(tx: &Self::Transaction) -> Result<LockFundsTransaction<Self::Id, Self::Recipient, Self::Amount>>;
}

/// Map that maps recipients from peer blockchain to this blockchain recipients.
pub trait RecipientsMap {
	/// Peer blockchain recipient type.
	type PeerRecipient;
	/// Current blockchain recipient type.
	type Recipient;

	/// Lookup current blockchain recipient by peer blockchain recipient.
	fn map(peer_recipient: Self::PeerRecipient) -> Result<Self::Recipient>;
}

/// Conversion between two currencies.
pub trait CurrencyConverter {
	/// Type of the source currency amount.
	type SourceAmount;
	/// Type of the target currency amount.
	type TargetAmount;

	/// Covert from source to target currency.
	fn convert(currency: Self::SourceAmount) -> Result<Self::TargetAmount>;
}

/// Currency airdrop.
pub trait Airdrop {
	/// Recipient type.
	type Recipient;
	/// Currency amount type.
	type Amount;

	/// Grant some money to given account.
	fn drop(recipient: Self::Recipient, amount: Self::Amount) -> Result<()>;
}

/// Recipients map which is used when accounts ids are the same on both chains.
pub struct AsIsRecipients<AccountId>(PhantomData<AccountId>);

impl<AccountId> RecipientsMap for AsIsRecipients<AccountId> {
	type PeerRecipient = AccountId;
	type Recipient = AccountId;

	fn map(peer_recipient: Self::PeerRecipient) -> Result<Self::Recipient> {
		Ok(peer_recipient)
	}
}

/// Currency converter which is used when currency is the same on both chains.
pub struct AsIsCurrencyConverter<Amount>(PhantomData<Amount>);

impl<Amount> CurrencyConverter for AsIsCurrencyConverter<Amount> {
	type SourceAmount = Amount;
	type TargetAmount = Amount;

	fn convert(currency: Self::SourceAmount) -> Result<Self::TargetAmount> {
		Ok(currency)
	}
}
