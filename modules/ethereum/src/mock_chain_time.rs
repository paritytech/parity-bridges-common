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

pub use crate::test_utils::{insert_header, validator_utils::*, validators_change_receipt, HeaderBuilder, GAS_LIMIT};
pub use bp_eth_poa::signatures::secret_to_address;

use crate::mock::{self};
use crate::validators::ValidatorsConfiguration;
use crate::{AuraConfiguration, ChainTime, PruningStrategy, Trait};
use bp_eth_poa::H256;
use frame_support::{impl_outer_origin, parameter_types, weights::Weight};
use sp_runtime::{
	testing::Header as SubstrateHeader,
	traits::{BlakeTwo256, IdentityLookup},
	Perbill,
};

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct TestChainTimeRuntime;

impl_outer_origin! {
	pub enum Origin for TestChainTimeRuntime where system = frame_system {}
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: Weight = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::one();
}

impl frame_system::Trait for TestChainTimeRuntime {
	type Origin = Origin;
	type Index = u64;
	type Call = ();
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = mock::AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = SubstrateHeader;
	type Event = ();
	type BlockHashCount = BlockHashCount;
	type MaximumBlockWeight = MaximumBlockWeight;
	type DbWeight = ();
	type BlockExecutionWeight = ();
	type ExtrinsicBaseWeight = ();
	type MaximumExtrinsicWeight = ();
	type AvailableBlockRatio = AvailableBlockRatio;
	type MaximumBlockLength = MaximumBlockLength;
	type Version = ();
	type PalletInfo = ();
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type BaseCallFilter = ();
	type SystemWeightInfo = ();
}

parameter_types! {
	pub const TestFinalityVotesCachingInterval: Option<u64> = Some(16);
	pub TestAuraConfiguration: AuraConfiguration = mock::test_aura_config();
	pub TestValidatorsConfiguration: ValidatorsConfiguration = mock::test_validators_config();
}

impl Trait for TestChainTimeRuntime {
	type AuraConfiguration = TestAuraConfiguration;
	type ValidatorsConfiguration = TestValidatorsConfiguration;
	type FinalityVotesCachingInterval = TestFinalityVotesCachingInterval;
	type PruningStrategy = KeepSomeHeadersBehindBest;
	type ChainTime = ConstChainTime;
	type OnHeadersSubmitted = ();
}

/// Pruning strategy that keeps 10 headers behind best block.
pub struct KeepSomeHeadersBehindBest(pub u64);

impl Default for KeepSomeHeadersBehindBest {
	fn default() -> KeepSomeHeadersBehindBest {
		KeepSomeHeadersBehindBest(10)
	}
}

impl PruningStrategy for KeepSomeHeadersBehindBest {
	fn pruning_upper_bound(&mut self, best_number: u64, _: u64) -> u64 {
		best_number.saturating_sub(self.0)
	}
}

/// Constant chain time
#[derive(Default)]
pub struct ConstChainTime;

impl ChainTime for ConstChainTime {
	fn header_is_ahead(&self, timestamp: u64) -> bool {
		let now = i32::max_value() as u64 / 2;
		if timestamp > now {
			true
		} else {
			false
		}
	}
}
