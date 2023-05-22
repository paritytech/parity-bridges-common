// Copyright 2019-2021 Parity Technologies (UK) Ltd.
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

//! Pallet provides a set of guard functions that are running in background threads
//! and are aborting process if some condition fails.

//! Test chain implementation to use in tests.

#![cfg(any(feature = "test-helpers", test))]

use crate::{
	Chain, ChainWithBalances, ChainWithTransactions, HeaderOf, SignParam, UnsignedTransaction,
};
use bp_runtime::ChainId;
use codec::{Decode, Encode};
use frame_support::weights::Weight;
use sp_runtime::AccountId32;
use std::time::Duration;

/// One of chains that may be used in tests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestChainA;

impl bp_runtime::Chain for TestChainA {
	type BlockNumber = u32;
	type Hash = sp_core::H256;
	type Hasher = sp_runtime::traits::BlakeTwo256;
	type Header = sp_runtime::generic::Header<u32, sp_runtime::traits::BlakeTwo256>;

	type AccountId = AccountId32;
	type Balance = u32;
	type Index = u32;
	type Signature = sp_runtime::testing::TestSignature;

	fn max_extrinsic_size() -> u32 {
		unreachable!()
	}

	fn max_extrinsic_weight() -> Weight {
		unreachable!()
	}
}

impl Chain for TestChainA {
	const ID: ChainId = *b"TCHA";
	const NAME: &'static str = "ChainA";
	const BEST_FINALIZED_HEADER_ID_METHOD: &'static str = "BestFinalizedOfTestChainA";
	const AVERAGE_BLOCK_INTERVAL: Duration = Duration::from_millis(1);

	type SignedBlock = sp_runtime::generic::SignedBlock<
		sp_runtime::generic::Block<Self::Header, sp_runtime::OpaqueExtrinsic>,
	>;
	type Call = ();
}

impl ChainWithBalances for TestChainA {
	fn account_info_storage_key(_account_id: &AccountId32) -> sp_core::storage::StorageKey {
		unreachable!()
	}
}

impl ChainWithTransactions for TestChainA {
	type AccountKeyPair = sp_core::sr25519::Pair;
	type SignedTransaction = Vec<u8>;

	fn sign_transaction(
		_param: SignParam<Self>,
		_unsigned: UnsignedTransaction<Self>,
	) -> Result<Self::SignedTransaction, crate::Error>
	where
		Self: Sized,
	{
		unimplemented!()
	}

	fn is_signed(_tx: &Self::SignedTransaction) -> bool {
		true
	}

	fn is_signed_by(_signer: &Self::AccountKeyPair, _tx: &Self::SignedTransaction) -> bool {
		unimplemented!()
	}

	fn parse_transaction(_tx: Self::SignedTransaction) -> Option<UnsignedTransaction<Self>> {
		unimplemented!()
	}
}

/// One of chains that may be used in tests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestChainB;

impl bp_runtime::Chain for TestChainB {
	type BlockNumber = u32;
	type Hash = sp_core::H256;
	type Hasher = sp_runtime::traits::BlakeTwo256;
	type Header = sp_runtime::generic::Header<u32, sp_runtime::traits::BlakeTwo256>;

	type AccountId = AccountId32;
	type Balance = u32;
	type Index = u32;
	type Signature = sp_runtime::testing::TestSignature;

	fn max_extrinsic_size() -> u32 {
		unreachable!()
	}

	fn max_extrinsic_weight() -> Weight {
		unreachable!()
	}
}

impl Chain for TestChainB {
	const ID: ChainId = *b"TCHB";
	const NAME: &'static str = "ChainB";
	const BEST_FINALIZED_HEADER_ID_METHOD: &'static str = "BestFinalizedOfTestChainB";
	const AVERAGE_BLOCK_INTERVAL: Duration = Duration::from_millis(1);

	type SignedBlock = sp_runtime::generic::SignedBlock<
		sp_runtime::generic::Block<Self::Header, sp_runtime::OpaqueExtrinsic>,
	>;
	type Call = TestChainBCall;
}

impl ChainWithBalances for TestChainB {
	fn account_info_storage_key(_account_id: &AccountId32) -> sp_core::storage::StorageKey {
		unreachable!()
	}
}

impl ChainWithTransactions for TestChainB {
	type AccountKeyPair = sp_core::sr25519::Pair;
	type SignedTransaction = Vec<u8>;

	fn sign_transaction(
		_param: SignParam<Self>,
		_unsigned: UnsignedTransaction<Self>,
	) -> Result<Self::SignedTransaction, crate::Error>
	where
		Self: Sized,
	{
		unimplemented!()
	}

	fn is_signed(_tx: &Self::SignedTransaction) -> bool {
		true
	}

	fn is_signed_by(_signer: &Self::AccountKeyPair, _tx: &Self::SignedTransaction) -> bool {
		unimplemented!()
	}

	fn parse_transaction(_tx: Self::SignedTransaction) -> Option<UnsignedTransaction<Self>> {
		unimplemented!()
	}
}

#[derive(Clone, Debug, Decode, Encode)]
pub enum TestChainBCall {
	ChainAHeader(HeaderOf<TestChainA>),
}

/// Primitives-level parachain that may be used in tests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestParachainBase;

impl bp_runtime::Chain for TestParachainBase {
	type BlockNumber = u32;
	type Hash = sp_core::H256;
	type Hasher = sp_runtime::traits::BlakeTwo256;
	type Header = sp_runtime::generic::Header<u32, sp_runtime::traits::BlakeTwo256>;

	type AccountId = u32;
	type Balance = u32;
	type Index = u32;
	type Signature = sp_runtime::testing::TestSignature;

	fn max_extrinsic_size() -> u32 {
		unreachable!()
	}

	fn max_extrinsic_weight() -> Weight {
		unreachable!()
	}
}

impl bp_runtime::Parachain for TestParachainBase {
	const PARACHAIN_ID: u32 = 1000;
}

/// Parachain that may be used in tests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestParachain;

impl bp_runtime::UnderlyingChainProvider for TestParachain {
	type Chain = TestParachainBase;
}

impl Chain for TestParachain {
	const ID: ChainId = *b"test";
	const NAME: &'static str = "TestParachain";
	const BEST_FINALIZED_HEADER_ID_METHOD: &'static str = "TestParachainMethod";
	const AVERAGE_BLOCK_INTERVAL: Duration = Duration::from_millis(0);

	type SignedBlock = sp_runtime::generic::SignedBlock<
		sp_runtime::generic::Block<Self::Header, sp_runtime::OpaqueExtrinsic>,
	>;
	type Call = ();
}
