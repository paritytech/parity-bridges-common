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

//! Benchmarks for the relayers Pallet.

#![cfg(feature = "runtime-benchmarks")]

use crate::*;

use bp_messages::LaneId;
use bp_relayers::RewardsAccountOwner;
use frame_benchmarking::{benchmarks, whitelisted_caller};
use frame_support::traits::Get;
use frame_system::{pallet_prelude::BlockNumberFor, RawOrigin};
use sp_runtime::traits::One;

/// Reward amount that is (hopefully) is larger than existential deposit across all chains.
const REWARD_AMOUNT: u32 = u32::MAX;

/// Pallet we're benchmarking here.
pub struct Pallet<T: Config>(crate::Pallet<T>);

/// Trait that must be implemented by runtime.
pub trait Config: crate::Config {
	/// Prepare environment for paying given reward for serving given lane.
	fn prepare_rewards_account(account_params: RewardsAccountParams, reward: Self::Reward);
	/// Give enough balance to given account.
	fn deposit_account(account: Self::AccountId, balance: Self::Reward);
}

/// Return lane id that we use in tests.
fn lane_id(i: u32) -> LaneId {
	LaneId::new(i, i)
}

/// Return block number until which our test registration is considered valid.
fn valid_till<T: Config>() -> BlockNumberFor<T> {
	frame_system::Pallet::<T>::block_number()
		.saturating_add(crate::Pallet::<T>::required_registration_lease())
		.saturating_add(One::one())
		.saturating_add(One::one())
}

/// Add basic relayer registration and optionally lane registrations.
fn register_relayer<T: Config>(relayer: &T::AccountId, lanes_reg_count: u32) {
	let stake = crate::Pallet::<T>::base_stake().saturating_add(
		crate::Pallet::<T>::stake_per_lane().saturating_mul((lanes_reg_count + 1).into()),
	);
	T::deposit_account(relayer.clone(), stake);
	crate::Pallet::<T>::increase_stake(RawOrigin::Signed(relayer.clone()).into(), stake).unwrap();
	crate::Pallet::<T>::register(RawOrigin::Signed(relayer.clone()).into(), valid_till::<T>())
		.unwrap();

	for i in 0..lanes_reg_count {
		crate::Pallet::<T>::register_at_lane(
			RawOrigin::Signed(relayer.clone()).into(),
			lane_id(i),
			0,
		)
		.unwrap();
	}
	assert_eq!(
		crate::Pallet::<T>::registered_relayer(&relayer).map(|reg| reg.lanes().len() as u32),
		Some(lanes_reg_count),
	);
}

benchmarks! {
	// Benchmark `claim_rewards` call.
	claim_rewards {
		let lane = LaneId::new(1, 2);
		let account_params =
			RewardsAccountParams::new(lane, *b"test", RewardsAccountOwner::ThisChain);
		let relayer: T::AccountId = whitelisted_caller();
		let reward = T::Reward::from(REWARD_AMOUNT);

		T::prepare_rewards_account(account_params, reward);
		RelayerRewards::<T>::insert(&relayer, account_params, reward);
	}: _(RawOrigin::Signed(relayer), account_params)
	verify {
		// we can't check anything here, because `PaymentProcedure` is responsible for
		// payment logic, so we assume that if call has succeeded, the procedure has
		// also completed successfully
	}

	// Benchmark `increase_stake` call.
	increase_stake {
		let relayer: T::AccountId = whitelisted_caller();
		let stake = crate::Pallet::<T>::base_stake();
		T::deposit_account(relayer.clone(), stake);
	}: _(RawOrigin::Signed(relayer.clone()), stake)
	verify {
		assert_eq!(
			crate::Pallet::<T>::registered_relayer(&relayer).map(|reg| reg.current_stake()),
			Some(stake),
		);
	}

	// Benchmark `decrease_stake` call.
	decrease_stake {
		let relayer: T::AccountId = whitelisted_caller();
		let base_stake = crate::Pallet::<T>::base_stake();
		let stake = base_stake.saturating_add(100u32.into());
		T::deposit_account(relayer.clone(), stake);
		crate::Pallet::<T>::increase_stake(RawOrigin::Signed(relayer.clone()).into(), stake).unwrap();
	}: _(RawOrigin::Signed(relayer.clone()), 100u32.into())
	verify {
		assert_eq!(
			crate::Pallet::<T>::registered_relayer(&relayer).map(|reg| reg.current_stake()),
			Some(base_stake),
		);
	}

	// Benchmark `register` call.
	register {
		let relayer: T::AccountId = whitelisted_caller();
		let base_stake = crate::Pallet::<T>::base_stake();
		let valid_till = frame_system::Pallet::<T>::block_number()
			.saturating_add(crate::Pallet::<T>::required_registration_lease())
			.saturating_add(One::one())
			.saturating_add(One::one());

		T::deposit_account(relayer.clone(), base_stake);
		crate::Pallet::<T>::increase_stake(RawOrigin::Signed(relayer.clone()).into(), base_stake).unwrap();
	}: _(RawOrigin::Signed(relayer.clone()), valid_till)
	verify {
		assert!(crate::Pallet::<T>::is_registration_active(&relayer));
	}

	// Benchmark `deregister` call.
	deregister {
		let relayer: T::AccountId = whitelisted_caller();
		register_relayer::<T>(&relayer, 0);
		frame_system::Pallet::<T>::set_block_number(valid_till::<T>().saturating_add(One::one()));
	}: _(RawOrigin::Signed(relayer.clone()))
	verify {
		assert!(!crate::Pallet::<T>::is_registration_active(&relayer));
	}

	// Benchmark `register_at_lane` call. The worst case for this call is when:
	//
	// - relayer has `T::MaxLanesPerRelayer::get() - 1` lane registrations;
	//
	// - there are no other relayers registered at that lane yet;
	register_at_lane {
		let relayer: T::AccountId = whitelisted_caller();
		let max_lanes_per_relayer = T::MaxLanesPerRelayer::get();
		register_relayer::<T>(&relayer, max_lanes_per_relayer - 1);
	}: _(RawOrigin::Signed(relayer.clone()), lane_id(max_lanes_per_relayer), 0)
	verify {
		assert_eq!(
			crate::Pallet::<T>::registered_relayer(&relayer).map(|reg| reg.lanes().len() as u32),
			Some(max_lanes_per_relayer),
		);
	}

	// Benchmark `deregister_at_lane` call. The worst case for this call is when relayer is not in
	// the active relayers set.
	deregister_at_lane {
		let relayer: T::AccountId = whitelisted_caller();
		let max_lanes_per_relayer = T::MaxLanesPerRelayer::get();
		register_relayer::<T>(&relayer, max_lanes_per_relayer);
	}: _(RawOrigin::Signed(relayer.clone()), lane_id(0))
	verify {
		assert_eq!(
			crate::Pallet::<T>::registered_relayer(&relayer).map(|reg| reg.lanes().len() as u32),
			Some(max_lanes_per_relayer - 1),
		);
	}

	// Benchmark `slash_and_deregister` method of the pallet. We are adding this weight to
	// the weight of message delivery call if `RefundBridgedParachainMessages` signed extension
	// is deployed at runtime level.
	slash_and_deregister {
		// prepare and register relayer account
		let relayer: T::AccountId = whitelisted_caller();
		let base_stake = crate::Pallet::<T>::base_stake();
		let valid_till = frame_system::Pallet::<T>::block_number()
			.saturating_add(crate::Pallet::<T>::required_registration_lease())
			.saturating_add(One::one())
			.saturating_add(One::one());
		T::deposit_account(relayer.clone(), base_stake);
		crate::Pallet::<T>::increase_stake(RawOrigin::Signed(relayer.clone()).into(), base_stake).unwrap();
		crate::Pallet::<T>::register(RawOrigin::Signed(relayer.clone()).into(), valid_till).unwrap();

		// TODO: add max number of lane registrations

		// create slash destination account
		let lane = LaneId::new(1, 2);
		let slash_destination = RewardsAccountParams::new(lane, *b"test", RewardsAccountOwner::ThisChain);
		T::prepare_rewards_account(slash_destination.clone(), Zero::zero());
	}: {
		crate::Pallet::<T>::slash_and_deregister(&relayer, slash_destination)
	}
	verify {
		assert!(!crate::Pallet::<T>::is_registration_active(&relayer));
	}

	// Benchmark `register_relayer_reward` method of the pallet. We are adding this weight to
	// the weight of message delivery call if `RefundBridgedParachainMessages` signed extension
	// is deployed at runtime level.
	register_relayer_reward {
		let lane = LaneId::new(1, 2);
		let relayer: T::AccountId = whitelisted_caller();
		let account_params =
			RewardsAccountParams::new(lane, *b"test", RewardsAccountOwner::ThisChain);
	}: {
		crate::Pallet::<T>::register_relayer_reward(account_params.clone(), &relayer, One::one());
	}
	verify {
		assert_eq!(RelayerRewards::<T>::get(relayer, &account_params), Some(One::one()));
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::TestRuntime)
}
