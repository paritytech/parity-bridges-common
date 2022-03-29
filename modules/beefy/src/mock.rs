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

use crate as beefy;

use bp_beefy::ChainWithBeefy;
use bp_runtime::Chain;
use frame_support::{construct_runtime, parameter_types, weights::Weight};
use sp_core::sr25519::Signature;
use sp_runtime::{
	testing::{Header, H256},
	traits::{BlakeTwo256, IdentityLookup},
	Perbill,
};

pub use beefy_primitives::crypto::AuthorityId as BeefyId;

pub type AccountId = u64;
pub type BridgedBlockNumber = u64;

type Block = frame_system::mocking::MockBlock<TestRuntime>;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;

construct_runtime! {
	pub enum TestRuntime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Beefy: beefy::{Pallet},
	}
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: Weight = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::one();
}

impl frame_system::Config for TestRuntime {
	type Origin = Origin;
	type Index = u64;
	type Call = Call;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = ();
	type BlockHashCount = BlockHashCount;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type BaseCallFilter = frame_support::traits::Everything;
	type SystemWeightInfo = ();
	type DbWeight = ();
	type BlockWeights = ();
	type BlockLength = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl beefy::Config for TestRuntime {
	type BridgedChain = TestBridgedChain;
}

#[derive(Debug)]
pub struct TestBridgedChain;

impl Chain for TestBridgedChain {
	type BlockNumber = BridgedBlockNumber;
	type Hash = H256;
	type Hasher = BlakeTwo256;
	type Header = <TestRuntime as frame_system::Config>::Header;

	type AccountId = AccountId;
	type Balance = u64;
	type Index = u64;
	type Signature = Signature;

	fn max_extrinsic_size() -> u32 {
		unreachable!()
	}
	fn max_extrinsic_weight() -> Weight {
		unreachable!()
	}
}

impl ChainWithBeefy for TestBridgedChain {
	type CommitmentHasher = sp_runtime::traits::Keccak256;
	type MmrHasher = beefy_merkle_tree::Keccak256;
	type ValidatorId = BeefyId;
	type ValidatorIdToMerkleLeaf = ();
}

pub fn run_test<T>(test: impl FnOnce() -> T) -> T {
	sp_io::TestExternalities::new(Default::default()).execute_with(test)
}
