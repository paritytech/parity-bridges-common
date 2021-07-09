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

//! Parachains support in Rialto runtime.

//  Polkadot commit: 2bad8079a73dc92d83bc3c0e5d81de388d36cf24
// Substrate commit: 3cd75117765c4a63d40c00aa41e1bf12135c237b
// Substrate regex:  substrate\?branch\=master\#[^3]

use crate::{AccountId, Balance, Balances, BlockNumber, Event, Origin, RandomnessCollectiveFlip, Runtime};

use frame_support::parameter_types;
use frame_system::EnsureRoot;
use polkadot_primitives::v1::ValidatorIndex;
use polkadot_runtime_common::{paras_sudo_wrapper, paras_registrar, slots};
use polkadot_runtime_parachains::origin as parachains_origin;
use polkadot_runtime_parachains::configuration as parachains_configuration;
use polkadot_runtime_parachains::shared as parachains_shared;
use polkadot_runtime_parachains::inclusion as parachains_inclusion;
use polkadot_runtime_parachains::paras_inherent as parachains_paras_inherent;
use polkadot_runtime_parachains::initializer as parachains_initializer;
use polkadot_runtime_parachains::session_info as parachains_session_info;
use polkadot_runtime_parachains::paras as parachains_paras;
use polkadot_runtime_parachains::dmp as parachains_dmp;
use polkadot_runtime_parachains::ump as parachains_ump;
use polkadot_runtime_parachains::hrmp as parachains_hrmp;
use polkadot_runtime_parachains::scheduler as parachains_scheduler;
use polkadot_runtime_parachains::reward_points as parachains_reward_points;
use xcm::v0::{MultiLocation, NetworkId};
use xcm_builder::{
	AccountId32Aliases, ChildParachainConvertsVia, SovereignSignedViaLocation,
	CurrencyAdapter as XcmCurrencyAdapter, ChildParachainAsNative,
	SignedAccountId32AsNative, ChildSystemParachainAsSuperuser, LocationInverter,
	IsConcrete, FixedWeightBounds, FixedRateOfConcreteFungible,
};

/// Special `RewardValidators` that does nothing ;)
pub struct RewardValidators;
impl polkadot_runtime_parachains::inclusion::RewardValidators for RewardValidators {
	fn reward_backing(_: impl IntoIterator<Item=ValidatorIndex>) {}
	fn reward_bitfields(_: impl IntoIterator<Item=ValidatorIndex>) {}
}

// required parachain modules from `polkadot-runtime-parachains` crate

impl parachains_configuration::Config for Runtime {}

impl parachains_dmp::Config for Runtime {}

impl parachains_hrmp::Config for Runtime {
	type Event = Event;
	type Origin = Origin;
	type Currency = Balances;
}

impl parachains_inclusion::Config for Runtime {
	type Event = Event;
	type RewardValidators = RewardValidators;
}

impl parachains_initializer::Config for Runtime {
	type Randomness = RandomnessCollectiveFlip;
	type ForceOrigin = EnsureRoot<AccountId>;
}

impl parachains_origin::Config for Runtime {}

impl parachains_paras::Config for Runtime {
	type Origin = Origin;
	type Event = Event;
}

impl parachains_paras_inherent::Config for Runtime {}

impl parachains_scheduler::Config for Runtime {}

impl parachains_session_info::Config for Runtime {}

impl parachains_shared::Config for Runtime {}

parameter_types! {
	pub const FirstMessageFactorPercent: u64 = 100;
}

impl parachains_ump::Config for Runtime {
	type Event = Event;
	type UmpSink = ();
	type FirstMessageFactorPercent = FirstMessageFactorPercent;
}

/*
pallet_authority_discovery: parachains_session_info::Config
*/







/*
impl paras_sudo_wrapper::Config for Runtime {}

parameter_types! {
	pub const ParaDeposit: Balance = 0;
	pub const DataDepositPerByte: Balance = 0;
}

impl paras_registrar::Config for Runtime {
	type Event = Event;
	type Origin = Origin;
	type Currency = Balances;
	type OnSwap = ();
	type ParaDeposit = ParaDeposit;
	type DataDepositPerByte = DataDepositPerByte;
	type WeightInfo = ();
}

parameter_types! {
	pub const LeasePeriod: BlockNumber = 28 * bp_rialto::DAYS;
}

impl slots::Config for Runtime {
	type Event = Event;
	type Currency = Balances;
	type Registrar = Registrar;
	type LeasePeriod = LeasePeriod;
	type WeightInfo = slots::TestWeightInfo;
}
*/