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

use bp_runtime::Chain;
use codec::{Decode, Encode};
use frame_support::{impl_outer_event, impl_outer_origin, parameter_types, weights::Weight};
use sp_runtime::{
	testing::{Header as SubstrateHeader, H256},
	traits::{BlakeTwo256, IdentityLookup},
	Perbill,
};

pub type AccountId = u64;
pub type TestPayload = (u64, Weight);
pub type TestMessageFee = u64;
pub type TestRelayer = u64;

#[derive(Clone, Eq, PartialEq, Debug)]
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

impl frame_system::Config for TestRuntime {
	type Origin = Origin;
	type Index = u64;
	type Call = ();
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = SubstrateHeader;
	type Event = ();
	type BlockHashCount = BlockHashCount;
	type Version = ();
	type PalletInfo = ();
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type BaseCallFilter = ();
	type SystemWeightInfo = ();
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
}

parameter_types! {}

impl crate::Config for TestRuntime {
	type BridgedChain = TestBridgedChain;
	type HeaderChain = pallet_substrate_bridge::Module<TestRuntime>;
	type AncestryChecker = Checker<<Self::BridgedChain as Chain>::Header, ()>;
	type AncestryProof = ();
}

#[derive(Debug)]
pub struct TestBridgedChain;

impl Chain for TestBridgedChain {
	type BlockNumber = <TestRuntime as frame_system::Config>::BlockNumber;
	type Hash = <TestRuntime as frame_system::Config>::Hash;
	type Hasher = <TestRuntime as frame_system::Config>::Hashing;
	type Header = <TestRuntime as frame_system::Config>::Header;
}

#[derive(Debug)]
pub struct Checker<H, P>(std::marker::PhantomData<(H, P)>);

impl<H, P> crate::AncestryChecker<H, P> for Checker<H, P> {
	fn are_ancestors(ancestor: H, child: H, proof: P) -> bool {
		true
	}
}

/// Run message lane test.
pub fn run_test<T>(test: impl FnOnce() -> T) -> T {
	let t = frame_system::GenesisConfig::default()
		.build_storage::<TestRuntime>()
		.unwrap();
	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(test)
}
