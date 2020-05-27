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

use frame_support::{decl_error, decl_module, decl_storage, ensure, Parameter};
use primitives::exchange::{
	CurrencyConverter, DepositInto, Error as ExchangeError, MaybeLockFundsTransaction, RecipientsMap,
};
use sp_runtime::DispatchResult;

/// Called when transaction is submitted to the exchange module.
pub trait OnTransactionSubmitted<AccountId> {
	/// Called when valid transaction is submitted and accepted by the module.
	fn on_valid_transaction_submitted(submitter: AccountId);
}

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
	/// Handler for transaction submission result.
	type OnTransactionSubmitted: OnTransactionSubmitted<Self::AccountId>;
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
	type DepositInto: DepositInto<Recipient = Self::AccountId, Amount = Self::Amount>;
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
		/// Deposit has failed.
		DepositFailed,
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
			let submitter = frame_system::ensure_signed(origin)?;

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
			ensure!(is_transaction_finalized, Error::<T>::UnfinalizedTransaction);

			// parse transaction
			let transaction = <T as Trait>::PeerMaybeLockFundsTransaction::parse(&transaction)
				.map_err(Error::<T>::from)?;
			let transfer_id = transaction.id;
			ensure!(
				!Transfers::<T>::contains_key(&transfer_id),
				Error::<T>::AlreadyClaimed
			);

			// grant recipient
			let recipient = T::RecipientsMap::map(transaction.recipient).map_err(Error::<T>::from)?;
			let amount = T::CurrencyConverter::convert(transaction.amount).map_err(Error::<T>::from)?;
			T::DepositInto::deposit_into(recipient, amount).map_err(Error::<T>::from)?;

			// remember that we have accepted this transfer
			Transfers::<T>::insert(transfer_id, ());

			// reward submitter for providing valid message
			T::OnTransactionSubmitted::on_valid_transaction_submitted(submitter);

			Ok(())
		}
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as Bridge {
		/// All transfers that have already been claimed.
		Transfers: map hasher(blake2_128_concat) <T::PeerMaybeLockFundsTransaction as MaybeLockFundsTransaction>::Id => ();
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
			ExchangeError::DepositFailed => Error::DepositFailed,
		}
	}
}

impl<AccountId> OnTransactionSubmitted<AccountId> for () {
	fn on_valid_transaction_submitted(_: AccountId) {}
}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::{assert_noop, assert_ok, impl_outer_origin, parameter_types, weights::Weight};
	use primitives::exchange::LockFundsTransaction;
	use sp_core::H256;
	use sp_runtime::{
		testing::Header,
		traits::{BlakeTwo256, IdentityLookup},
		Perbill,
	};

	type AccountId = u64;

	const INVALID_TRANSACTION_ID: u64 = 100;
	const ALREADY_CLAIMED_TRANSACTION_ID: u64 = 101;
	const UNKNOWN_RECIPIENT_ID: u64 = 0;
	const INVALID_AMOUNT: u64 = 0;
	const MAX_DEPOSIT_AMOUNT: u64 = 1000;
	const SUBMITTER: u64 = 2000;

	type RawTransaction = LockFundsTransaction<u64, u64, u64>;

	pub struct DummyTransactionSubmissionHandler;

	impl OnTransactionSubmitted<AccountId> for DummyTransactionSubmissionHandler {
		fn on_valid_transaction_submitted(submitter: AccountId) {
			Transfers::<TestRuntime>::insert(submitter, ());
		}
	}

	pub struct DummyBlockchain;

	impl Blockchain for DummyBlockchain {
		type BlockHash = u64;
		type Transaction = RawTransaction;
		type TransactionInclusionProof = bool;

		fn verify_transaction_inclusion_proof(
			_transaction: &Self::Transaction,
			_block: Self::BlockHash,
			proof: &Self::TransactionInclusionProof,
		) -> bool {
			*proof
		}
	}

	pub struct DummyTransaction;

	impl MaybeLockFundsTransaction for DummyTransaction {
		type Transaction = RawTransaction;
		type Id = u64;
		type Recipient = AccountId;
		type Amount = u64;

		fn parse(tx: &Self::Transaction) -> primitives::exchange::Result<RawTransaction> {
			match tx.id {
				INVALID_TRANSACTION_ID => Err(primitives::exchange::Error::InvalidTransaction),
				_ => Ok(tx.clone()),
			}
		}
	}

	pub struct DummyRecipientsMap;

	impl RecipientsMap for DummyRecipientsMap {
		type PeerRecipient = AccountId;
		type Recipient = AccountId;

		fn map(peer_recipient: Self::PeerRecipient) -> primitives::exchange::Result<Self::Recipient> {
			match peer_recipient {
				UNKNOWN_RECIPIENT_ID => Err(primitives::exchange::Error::FailedToMapRecipients),
				_ => Ok(peer_recipient * 10),
			}
		}
	}

	pub struct DummyCurrencyConverter;

	impl CurrencyConverter for DummyCurrencyConverter {
		type SourceAmount = u64;
		type TargetAmount = u64;

		fn convert(amount: Self::SourceAmount) -> primitives::exchange::Result<Self::TargetAmount> {
			match amount {
				INVALID_AMOUNT => Err(primitives::exchange::Error::FailedToCovertCurrency),
				_ => Ok(amount * 10),
			}
		}
	}

	pub struct DummyDepositInto;

	impl DepositInto for DummyDepositInto {
		type Recipient = AccountId;
		type Amount = u64;

		fn deposit_into(_recipient: Self::Recipient, amount: Self::Amount) -> primitives::exchange::Result<()> {
			match amount > MAX_DEPOSIT_AMOUNT {
				true => Err(primitives::exchange::Error::DepositFailed),
				_ => Ok(()),
			}
		}
	}

	#[derive(Clone, Eq, PartialEq)]
	pub struct TestRuntime;

	impl_outer_origin! {
		pub enum Origin for TestRuntime where system = frame_system {}
	}

	parameter_types! {
		pub const BlockHashCount: u64 = 250;
		pub const MaximumBlockWeight: Weight = 1024;
		pub const MaximumBlockLength: u32 = 2 * 1024;
		pub const AvailableBlockRatio: Perbill = Perbill::one();
	}

	impl frame_system::Trait for TestRuntime {
		type Origin = Origin;
		type Index = u64;
		type Call = ();
		type BlockNumber = u64;
		type Hash = H256;
		type Hashing = BlakeTwo256;
		type AccountId = AccountId;
		type Lookup = IdentityLookup<Self::AccountId>;
		type Header = Header;
		type Event = ();
		type BlockHashCount = BlockHashCount;
		type MaximumBlockWeight = MaximumBlockWeight;
		type DbWeight = ();
		type BlockExecutionWeight = ();
		type ExtrinsicBaseWeight = ();
		type AvailableBlockRatio = AvailableBlockRatio;
		type MaximumBlockLength = MaximumBlockLength;
		type Version = ();
		type ModuleToIndex = ();
		type AccountData = ();
		type OnNewAccount = ();
		type OnKilledAccount = ();
	}

	impl Trait for TestRuntime {
		type OnTransactionSubmitted = DummyTransactionSubmissionHandler;
		type PeerBlockchain = DummyBlockchain;
		type PeerMaybeLockFundsTransaction = DummyTransaction;
		type RecipientsMap = DummyRecipientsMap;
		type Amount = u64;
		type CurrencyConverter = DummyCurrencyConverter;
		type DepositInto = DummyDepositInto;
	}

	type Exhange = Module<TestRuntime>;

	fn new_test_ext() -> sp_io::TestExternalities {
		let t = frame_system::GenesisConfig::default()
			.build_storage::<TestRuntime>()
			.unwrap();
		sp_io::TestExternalities::new(t)
	}

	fn transaction(id: u64) -> RawTransaction {
		RawTransaction {
			id,
			recipient: 1,
			amount: 2,
		}
	}

	#[test]
	fn unfinalized_transaction_rejected() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				Exhange::import_peer_transaction(Origin::signed(SUBMITTER), transaction(0), 0, false,),
				Error::<TestRuntime>::UnfinalizedTransaction,
			);
		});
	}

	#[test]
	fn invalid_transaction_rejected() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				Exhange::import_peer_transaction(
					Origin::signed(SUBMITTER),
					transaction(INVALID_TRANSACTION_ID),
					0,
					true,
				),
				Error::<TestRuntime>::InvalidTransaction,
			);
		});
	}

	#[test]
	fn claimed_transaction_rejected() {
		new_test_ext().execute_with(|| {
			<Exhange as crate::Store>::Transfers::insert(ALREADY_CLAIMED_TRANSACTION_ID, ());
			assert_noop!(
				Exhange::import_peer_transaction(
					Origin::signed(SUBMITTER),
					transaction(ALREADY_CLAIMED_TRANSACTION_ID),
					0,
					true,
				),
				Error::<TestRuntime>::AlreadyClaimed,
			);
		});
	}

	#[test]
	fn transaction_with_unknown_recipient_rejected() {
		new_test_ext().execute_with(|| {
			let mut transaction = transaction(0);
			transaction.recipient = UNKNOWN_RECIPIENT_ID;
			assert_noop!(
				Exhange::import_peer_transaction(Origin::signed(SUBMITTER), transaction, 0, true,),
				Error::<TestRuntime>::FailedToMapRecipients,
			);
		});
	}

	#[test]
	fn transaction_with_invalid_amount_rejected() {
		new_test_ext().execute_with(|| {
			let mut transaction = transaction(0);
			transaction.amount = INVALID_AMOUNT;
			assert_noop!(
				Exhange::import_peer_transaction(Origin::signed(SUBMITTER), transaction, 0, true,),
				Error::<TestRuntime>::FailedToCovertCurrency,
			);
		});
	}

	#[test]
	fn transaction_with_invalid_deposit_rejected() {
		new_test_ext().execute_with(|| {
			let mut transaction = transaction(0);
			transaction.amount = MAX_DEPOSIT_AMOUNT;
			assert_noop!(
				Exhange::import_peer_transaction(Origin::signed(SUBMITTER), transaction, 0, true,),
				Error::<TestRuntime>::DepositFailed,
			);
		});
	}

	#[test]
	fn valid_transaction_accepted() {
		new_test_ext().execute_with(|| {
			assert_ok!(Exhange::import_peer_transaction(
				Origin::signed(SUBMITTER),
				transaction(0),
				0,
				true,
			),);

			// ensure that the transfer has been marked as completed
			assert!(<Exhange as crate::Store>::Transfers::contains_key(0u64));
			// ensure that submitter has been rewarded
			assert!(<Exhange as crate::Store>::Transfers::contains_key(SUBMITTER));
		});
	}
}
