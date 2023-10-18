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
use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_support::traits::Get;
use frame_system::{pallet_prelude::BlockNumberFor, RawOrigin};
use sp_runtime::traits::One;
use sp_std::vec::Vec;

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
fn register_relayer<T: Config>(
	relayer: &T::AccountId,
	lanes_reg_count: u32,
	expected_reward: RelayerRewardAtSource,
) {
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
			expected_reward,
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
		register_relayer::<T>(&relayer, 0, 0);
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
		register_relayer::<T>(&relayer, max_lanes_per_relayer - 1, 0);
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
		register_relayer::<T>(&relayer, max_lanes_per_relayer, 0);
	}: _(RawOrigin::Signed(relayer.clone()), lane_id(0))
	verify {
		assert_eq!(
			crate::Pallet::<T>::registered_relayer(&relayer).map(|reg| reg.lanes().len() as u32),
			Some(max_lanes_per_relayer - 1),
		);
	}

	// Benchmark `advance_lane_epoch` call. The worst case for this call is when active set is completely
	// replaced with a next set.
	advance_lane_epoch {
		let current_block_number = frame_system::Pallet::<T>::block_number();

		// prepare active relayers set with max possible relayers count
		let max_active_relayers_per_lane = T::MaxActiveRelayersPerLane::get();
		let active_relayers = (0..max_active_relayers_per_lane)
			.map(|i| account("relayer", i, 0))
			.collect::<Vec<_>>();
		let mut active_relayers_set = ActiveLaneRelayersSet::<_, BlockNumberFor<T>, T::MaxActiveRelayersPerLane>::default();
		let mut next_relayers_set = NextLaneRelayersSet::<_, BlockNumberFor<T>, T::MaxNextRelayersPerLane>::empty(
			current_block_number,
		);
		for active_relayer in &active_relayers {
			register_relayer::<T>(active_relayer, 1, 1);
			assert!(next_relayers_set.try_push(active_relayer.clone(), 0));
		}
		active_relayers_set.activate_next_set(current_block_number, next_relayers_set, |_| true);
		ActiveLaneRelayers::<T>::insert(lane_id(0), active_relayers_set);

		// prepare next relayers set with max possible relayers count
		let max_next_relayers_per_lane = T::MaxNextRelayersPerLane::get();
		let next_relayers = (0..max_next_relayers_per_lane)
			.map(|i| account::<T::AccountId>("relayer", max_active_relayers_per_lane + i, 0))
			.collect::<Vec<_>>();
		for next_relayer in &next_relayers {
			register_relayer::<T>(next_relayer, 1, 0);
		}

		// set next block to block where next set can be activated
		frame_system::Pallet::<T>::set_block_number(
			NextLaneRelayers::<T>::get(lane_id(0)).unwrap().may_enact_at(),
		);
	}: _(RawOrigin::Signed(whitelisted_caller()), lane_id(0))
	verify {
		// active relayers are replaced with next relayers
		assert_eq!(
			crate::Pallet::<T>::active_lane_relayers(&lane_id(0))
				.relayers()
				.iter()
				.map(|r| r.relayer())
				.collect::<Vec<_>>(),
			next_relayers.iter().take(max_active_relayers_per_lane as _).collect::<Vec<_>>(),
		);

		// all (previous) active relayers have no lane registration
		active_relayers.into_iter().all(|r| crate::Pallet::<T>::registered_relayer(&r).unwrap().lanes().len() == 0);

		// all (previous) next relayers have no lane registration
		next_relayers.into_iter().all(|r| crate::Pallet::<T>::registered_relayer(&r).unwrap().lanes().len() == 1);
	}

	// Benchmark `slash_and_deregister` method of the pallet. We are adding this weight to
	// the weight of message delivery call if `RefundBridgedParachainMessages` signed extension
	// is deployed at runtime level.
	slash_and_deregister {
		// prepare and register relayer account
		let relayer: T::AccountId = whitelisted_caller();
		let max_lanes_per_relayer = T::MaxLanesPerRelayer::get();
		register_relayer::<T>(&relayer, max_lanes_per_relayer, 1);

		// also register relayer in next lane relayers set (with better bid)
		for i in 0..max_lanes_per_relayer {
			crate::Pallet::<T>::register_at_lane(
				RawOrigin::Signed(relayer.clone()).into(),
				lane_id(i),
				0,
			)
			.unwrap();
		}

		// create slash destination account
		let lane = LaneId::new(1, 2);
		let slash_destination = RewardsAccountParams::new(lane, *b"test", RewardsAccountOwner::ThisChain);
		T::prepare_rewards_account(slash_destination.clone(), Zero::zero());
	}: {
		crate::Pallet::<T>::slash_and_deregister(&relayer, slash_destination)
	}
	verify {
		assert!(!crate::Pallet::<T>::is_registration_active(&relayer));
		for i in 0..max_lanes_per_relayer {
			assert!(
				crate::Pallet::<T>::active_lane_relayers(lane_id(i))
					.relayer(&relayer)
					.is_none(),
			);
			assert!(
				crate::Pallet::<T>::next_lane_relayers(lane_id(i))
					.unwrap_or_else(|| NextLaneRelayersSet::empty(Zero::zero()))
					.relayer(&relayer)
					.is_none(),
			);
		}
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
