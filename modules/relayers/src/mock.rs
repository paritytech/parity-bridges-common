// Copyright (C) Parity Technologies (UK) Ltd.
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

#![cfg(test)]

use crate as pallet_bridge_relayers;

use bp_header_chain::ChainWithGrandpa;
use bp_messages::{
	target_chain::{DispatchMessage, MessageDispatch},
	ChainWithMessages, LaneId, MessageNonce,
};
use bp_parachains::SingleParaStoredHeaderDataBuilder;
use bp_relayers::{
	PayRewardFromAccount, PaymentProcedure, RewardsAccountOwner, RewardsAccountParams,
};
use bp_runtime::{messages::MessageDispatchResult, Chain, ChainId, Parachain};
use codec::Encode;
use frame_support::{
	parameter_types,
	traits::fungible::Mutate,
	weights::{ConstantMultiplier, IdentityFee, RuntimeDbWeight, Weight},
};
use pallet_transaction_payment::Multiplier;
use sp_core::{ConstU64, ConstU8, H256};
use sp_runtime::{
	traits::{BlakeTwo256, ConstU32, IdentityLookup},
	transaction_validity::TransactionPriority,
	BuildStorage, FixedPointNumber, Perquintill, StateVersion,
};

// generate identifier of the signed extension
bp_runtime::generate_static_str_provider!(TestId);

/// Account identifier at `ThisChain`.
pub type ThisChainAccountId = u64;
/// Balance at `ThisChain`.
pub type ThisChainBalance = u64;
/// Block number at `ThisChain`.
pub type ThisChainBlockNumber = u32;
/// Hash at `ThisChain`.
pub type ThisChainHash = H256;
/// Hasher at `ThisChain`.
pub type ThisChainHasher = BlakeTwo256;
/// Header of `ThisChain`.
pub type ThisChainHeader = sp_runtime::generic::Header<ThisChainBlockNumber, ThisChainHasher>;
/// Block of `ThisChain`.
pub type ThisChainBlock = frame_system::mocking::MockBlockU32<TestRuntime>;

/// Account identifier at the `BridgedChain`.
pub type BridgedChainAccountId = u128;
/// Balance at the `BridgedChain`.
pub type BridgedChainBalance = u128;
/// Block number at the `BridgedChain`.
pub type BridgedChainBlockNumber = u32;
/// Hash at the `BridgedChain`.
pub type BridgedChainHash = H256;
/// Hasher at the `BridgedChain`.
pub type BridgedChainHasher = BlakeTwo256;
/// Header of the `BridgedChain`.
pub type BridgedChainHeader =
	sp_runtime::generic::Header<BridgedChainBlockNumber, BridgedChainHasher>;

/// Bridged chain id used in tests.
pub const TEST_BRIDGED_CHAIN_ID: ChainId = *b"brdg";
/// Maximal extrinsic size at the `BridgedChain`.
pub const BRIDGED_CHAIN_MAX_EXTRINSIC_SIZE: u32 = 1024;

/// Maximal number of relayers in the next set.
pub const MAX_NEXT_RELAYERS_PER_LANE: u32 = 16;

/// Underlying chain of `ThisChain`.
pub struct ThisUnderlyingChain;

impl Chain for ThisUnderlyingChain {
	const ID: ChainId = *b"tuch";

	type BlockNumber = ThisChainBlockNumber;
	type Hash = ThisChainHash;
	type Hasher = ThisChainHasher;
	type Header = ThisChainHeader;
	type AccountId = ThisChainAccountId;
	type Balance = ThisChainBalance;
	type Nonce = u32;
	type Signature = sp_runtime::MultiSignature;

	const STATE_VERSION: StateVersion = StateVersion::V1;

	fn max_extrinsic_size() -> u32 {
		BRIDGED_CHAIN_MAX_EXTRINSIC_SIZE
	}

	fn max_extrinsic_weight() -> Weight {
		Weight::zero()
	}
}

impl ChainWithMessages for ThisUnderlyingChain {
	const WITH_CHAIN_MESSAGES_PALLET_NAME: &'static str = "";

	const MAX_UNREWARDED_RELAYERS_IN_CONFIRMATION_TX: MessageNonce = 16;
	const MAX_UNCONFIRMED_MESSAGES_IN_CONFIRMATION_TX: MessageNonce = 1000;
}

/// Underlying chain of `BridgedChain`.
pub struct BridgedUnderlyingChain;

impl Chain for BridgedUnderlyingChain {
	const ID: ChainId = TEST_BRIDGED_CHAIN_ID;

	type BlockNumber = BridgedChainBlockNumber;
	type Hash = BridgedChainHash;
	type Hasher = BridgedChainHasher;
	type Header = BridgedChainHeader;
	type AccountId = BridgedChainAccountId;
	type Balance = BridgedChainBalance;
	type Nonce = u32;
	type Signature = sp_runtime::MultiSignature;

	const STATE_VERSION: StateVersion = StateVersion::V1;

	fn max_extrinsic_size() -> u32 {
		BRIDGED_CHAIN_MAX_EXTRINSIC_SIZE
	}
	fn max_extrinsic_weight() -> Weight {
		Weight::zero()
	}
}

impl ChainWithGrandpa for BridgedUnderlyingChain {
	const WITH_CHAIN_GRANDPA_PALLET_NAME: &'static str = "";
	const MAX_AUTHORITIES_COUNT: u32 = 16;
	const REASONABLE_HEADERS_IN_JUSTIFICATON_ANCESTRY: u32 = 8;
	const MAX_HEADER_SIZE: u32 = 256;
	const AVERAGE_HEADER_SIZE_IN_JUSTIFICATION: u32 = 64;
}

impl ChainWithMessages for BridgedUnderlyingChain {
	const WITH_CHAIN_MESSAGES_PALLET_NAME: &'static str = "";
	const MAX_UNREWARDED_RELAYERS_IN_CONFIRMATION_TX: MessageNonce = 16;
	const MAX_UNCONFIRMED_MESSAGES_IN_CONFIRMATION_TX: MessageNonce = 1000;
}

impl Parachain for BridgedUnderlyingChain {
	const PARACHAIN_ID: u32 = 42;
}

pub type TestStakeAndSlash = pallet_bridge_relayers::StakeAndSlashNamed<
	ThisChainAccountId,
	ThisChainBlockNumber,
	Balances,
	ReserveId,
	Stake,
	LaneStake,
	Lease,
>;

frame_support::construct_runtime! {
	pub enum TestRuntime
	{
		System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>},
		Utility: pallet_utility,
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		TransactionPayment: pallet_transaction_payment::{Pallet, Storage, Event<T>},
		BridgeRelayers: pallet_bridge_relayers::{Pallet, Call, Storage, Event<T>},
		BridgeGrandpa: pallet_bridge_grandpa::{Pallet, Call, Storage, Event<T>},
		BridgeParachains: pallet_bridge_parachains::{Pallet, Call, Storage, Event<T>},
		BridgeMessages: pallet_bridge_messages::{Pallet, Call, Storage, Event<T>, Config<T>},
	}
}

parameter_types! {
	pub const BridgedParasPalletName: &'static str = "Paras";
	pub const DbWeight: RuntimeDbWeight = RuntimeDbWeight { read: 1, write: 2 };
	pub const ExistentialDeposit: ThisChainBalance = 1;
	pub const ReserveId: [u8; 8] = *b"brdgrlrs";
	pub const Stake: ThisChainBalance = 1_000;
	pub const LaneStake: ThisChainBalance = 100;
	pub const Lease: ThisChainBlockNumber = 8;
	pub const TargetBlockFullness: Perquintill = Perquintill::from_percent(25);
	pub const TransactionBaseFee: ThisChainBalance = 0;
	pub const TransactionByteFee: ThisChainBalance = 1;
	pub AdjustmentVariable: Multiplier = Multiplier::saturating_from_rational(3, 100_000);
	pub MinimumMultiplier: Multiplier = Multiplier::saturating_from_rational(1, 1_000_000u128);
	pub MaximumMultiplier: Multiplier = sp_runtime::traits::Bounded::max_value();
	pub InitialElectionLength: u32 = 4;
	pub SlotLength: u32 = 16;
	pub EpochLength: u32 = 1_024;
	pub MaxLanesPerRelayer: u32 = 4;
	pub PriorityBoostForActiveLaneRelayer: TransactionPriority = 4;
}

impl frame_system::Config for TestRuntime {
	type RuntimeOrigin = RuntimeOrigin;
	type Nonce = u64;
	type RuntimeCall = RuntimeCall;
	type Block = ThisChainBlock;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = ThisChainAccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = frame_support::traits::ConstU32<250>;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<ThisChainBalance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type BaseCallFilter = frame_support::traits::Everything;
	type SystemWeightInfo = ();
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = DbWeight;
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl pallet_utility::Config for TestRuntime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type PalletsOrigin = OriginCaller;
	type WeightInfo = ();
}

impl pallet_balances::Config for TestRuntime {
	type MaxLocks = ();
	type Balance = ThisChainBalance;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = frame_system::Pallet<TestRuntime>;
	type WeightInfo = ();
	type MaxReserves = ConstU32<1>;
	type ReserveIdentifier = [u8; 8];
	type RuntimeHoldReason = RuntimeHoldReason;
	type FreezeIdentifier = ();
	type MaxHolds = ConstU32<0>;
	type MaxFreezes = ConstU32<0>;
}

impl pallet_transaction_payment::Config for TestRuntime {
	type OnChargeTransaction = pallet_transaction_payment::CurrencyAdapter<Balances, ()>;
	type OperationalFeeMultiplier = ConstU8<5>;
	type WeightToFee = IdentityFee<ThisChainBalance>;
	type LengthToFee = ConstantMultiplier<ThisChainBalance, TransactionByteFee>;
	type FeeMultiplierUpdate = pallet_transaction_payment::TargetedFeeAdjustment<
		TestRuntime,
		TargetBlockFullness,
		AdjustmentVariable,
		MinimumMultiplier,
		MaximumMultiplier,
	>;
	type RuntimeEvent = RuntimeEvent;
}

impl pallet_bridge_grandpa::Config for TestRuntime {
	type RuntimeEvent = RuntimeEvent;
	type BridgedChain = BridgedUnderlyingChain;
	type MaxFreeMandatoryHeadersPerBlock = ConstU32<4>;
	type HeadersToKeep = ConstU32<8>;
	type WeightInfo = pallet_bridge_grandpa::weights::BridgeWeight<TestRuntime>;
}

impl pallet_bridge_parachains::Config for TestRuntime {
	type RuntimeEvent = RuntimeEvent;
	type BridgesGrandpaPalletInstance = ();
	type ParasPalletName = BridgedParasPalletName;
	type ParaStoredHeaderDataBuilder = SingleParaStoredHeaderDataBuilder<BridgedUnderlyingChain>;
	type HeadsToKeep = ConstU32<8>;
	type MaxParaHeadDataSize = ConstU32<1024>;
	type WeightInfo = pallet_bridge_parachains::weights::BridgeWeight<TestRuntime>;
}

impl pallet_bridge_messages::Config for TestRuntime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = pallet_bridge_messages::weights::BridgeWeight<TestRuntime>;

	type OutboundPayload = Vec<u8>;

	type InboundPayload = Vec<u8>;
	type DeliveryPayments = ();

	type DeliveryConfirmationPayments =
		pallet_bridge_relayers::DeliveryConfirmationPaymentsAdapter<TestRuntime, (), ConstU64<10>>;
	type OnMessagesDelivered = ();

	type MessageDispatch = DummyMessageDispatch;
	type ThisChain = ThisUnderlyingChain;
	type BridgedChain = BridgedUnderlyingChain;
	type BridgedHeaderChain = BridgeGrandpa;
}

impl pallet_bridge_relayers::Config for TestRuntime {
	type RuntimeEvent = RuntimeEvent;
	type Reward = ThisChainBalance;
	type PaymentProcedure = TestPaymentProcedure;
	type StakeAndSlash = TestStakeAndSlash;
	type MaxLanesPerRelayer = MaxLanesPerRelayer;
	type MaxActiveRelayersPerLane = ConstU32<4>;
	type MaxNextRelayersPerLane = ConstU32<MAX_NEXT_RELAYERS_PER_LANE>;
	type InitialElectionLength = InitialElectionLength;
	type SlotLength = ConstU32<16>;
	type EpochLength = EpochLength;
	type PriorityBoostPerMessage = ConstU64<1>;
	type PriorityBoostForActiveLaneRelayer = PriorityBoostForActiveLaneRelayer;
	type WeightInfo = ();
}

#[cfg(feature = "runtime-benchmarks")]
impl pallet_bridge_relayers::benchmarking::Config for TestRuntime {
	fn prepare_rewards_account(account_params: RewardsAccountParams, reward: ThisChainBalance) {
		let rewards_account =
			bp_relayers::PayRewardFromAccount::<Balances, ThisChainAccountId>::rewards_account(
				account_params,
			);
		Self::deposit_account(rewards_account, reward);
	}

	fn deposit_account(account: Self::AccountId, balance: Self::Reward) {
		Balances::mint_into(&account, balance.saturating_add(ExistentialDeposit::get())).unwrap();
	}
}

/// Regular relayer that may receive rewards.
pub const REGULAR_RELAYER: ThisChainAccountId = 1;

/// Relayer that can't receive rewards.
pub const FAILING_RELAYER: ThisChainAccountId = 2;

/// Relayer that is able to register.
pub const REGISTER_RELAYER: ThisChainAccountId = 42;
/// Another relayer that is able to register.
pub const REGISTER_RELAYER_2: ThisChainAccountId = 43;

/// Payment procedure that rejects payments to the `FAILING_RELAYER`.
pub struct TestPaymentProcedure;

impl TestPaymentProcedure {
	pub fn rewards_account(params: RewardsAccountParams) -> ThisChainAccountId {
		PayRewardFromAccount::<(), ThisChainAccountId>::rewards_account(params)
	}
}

impl PaymentProcedure<ThisChainAccountId, ThisChainBalance> for TestPaymentProcedure {
	type Error = ();

	fn pay_reward(
		relayer: &ThisChainAccountId,
		_lane_id: RewardsAccountParams,
		_reward: ThisChainBalance,
	) -> Result<(), Self::Error> {
		match *relayer {
			FAILING_RELAYER => Err(()),
			_ => Ok(()),
		}
	}
}

/// Dummy message dispatcher.
pub struct DummyMessageDispatch;

impl DummyMessageDispatch {
	pub fn deactivate(lane: LaneId) {
		frame_support::storage::unhashed::put(&(b"inactive", lane).encode()[..], &false);
	}
}

impl MessageDispatch for DummyMessageDispatch {
	type DispatchPayload = Vec<u8>;
	type DispatchLevelResult = ();

	fn is_active(lane: LaneId) -> bool {
		frame_support::storage::unhashed::take::<bool>(&(b"inactive", lane).encode()[..]) !=
			Some(false)
	}

	fn dispatch_weight(_message: &mut DispatchMessage<Self::DispatchPayload>) -> Weight {
		Weight::zero()
	}

	fn dispatch(
		_: DispatchMessage<Self::DispatchPayload>,
	) -> MessageDispatchResult<Self::DispatchLevelResult> {
		MessageDispatchResult { unspent_weight: Weight::zero(), dispatch_level_result: () }
	}
}

/// Lane identifier used in tests.
pub fn test_lane_id() -> LaneId {
	LaneId::new(1, 2)
}

/// Reward account params that we are using in tests.
pub fn test_reward_account_param() -> RewardsAccountParams {
	RewardsAccountParams::new(
		test_lane_id(),
		TEST_BRIDGED_CHAIN_ID,
		RewardsAccountOwner::BridgedChain,
	)
}

/// Return test externalities to use in tests.
pub fn new_test_ext() -> sp_io::TestExternalities {
	let t = frame_system::GenesisConfig::<TestRuntime>::default().build_storage().unwrap();
	sp_io::TestExternalities::new(t)
}

/// Run pallet test.
pub fn run_test<T>(test: impl FnOnce() -> T) -> T {
	new_test_ext().execute_with(|| {
		Balances::mint_into(&REGISTER_RELAYER, ExistentialDeposit::get() + 10 * Stake::get())
			.unwrap();
		Balances::mint_into(&REGISTER_RELAYER_2, ExistentialDeposit::get() + 10 * Stake::get())
			.unwrap();

		test()
	})
}
