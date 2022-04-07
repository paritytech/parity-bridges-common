// This file is part of Darwinia.
//
// Copyright (C) 2018-2022 Darwinia Network
// SPDX-License-Identifier: GPL-3.0
//
// Darwinia is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Darwinia is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Darwinia. If not, see <https://www.gnu.org/licenses/>.

//! Benchmarking
#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::Pallet as FeeMarket;
use frame_benchmarking::{account, benchmarks};
use frame_support::assert_ok;
use frame_system::RawOrigin;
use sp_runtime::traits::Saturating;

const SEED: u32 = 0;

fn fee_market_ready<T: Config>() {
	let caller0: T::AccountId = account("source", 0, SEED);
	let caller1: T::AccountId = account("source", 1, SEED);
	let caller2: T::AccountId = account("source", 2, SEED);
	let caller3: T::AccountId = account("source", 3, SEED);
	let collateral = T::CollateralPerOrder::get();
	T::RingCurrency::make_free_balance_be(&caller0, collateral.saturating_mul(10u32.into()));
	T::RingCurrency::make_free_balance_be(&caller1, collateral.saturating_mul(10u32.into()));
	T::RingCurrency::make_free_balance_be(&caller2, collateral.saturating_mul(10u32.into()));
	T::RingCurrency::make_free_balance_be(&caller3, collateral.saturating_mul(10u32.into()));
	assert_ne!(caller0, caller1);
	assert_ne!(caller1, caller2);

	assert_ok!(<FeeMarket<T>>::enroll_and_lock_collateral(
		RawOrigin::Signed(caller0).into(),
		collateral,
		None
	));
	assert_ok!(<FeeMarket<T>>::enroll_and_lock_collateral(
		RawOrigin::Signed(caller1).into(),
		collateral,
		None
	));
	assert_ok!(<FeeMarket<T>>::enroll_and_lock_collateral(
		RawOrigin::Signed(caller2).into(),
		collateral,
		None
	));
	assert_ok!(<FeeMarket<T>>::enroll_and_lock_collateral(
		RawOrigin::Signed(caller3).into(),
		collateral,
		None
	));
	assert!(<FeeMarket<T>>::market_fee().is_some());
	assert_eq!(<FeeMarket<T>>::relayers().unwrap().len(), 4);
}

benchmarks! {
	enroll_and_lock_collateral {
		fee_market_ready::<T>();
		let relayer: T::AccountId = account("source", 100, SEED);
		T::RingCurrency::make_free_balance_be(&relayer, T::CollateralPerOrder::get().saturating_mul(10u32.into()));
		let lock_collateral = T::CollateralPerOrder::get().saturating_mul(5u32.into());
	}: enroll_and_lock_collateral(RawOrigin::Signed(relayer.clone()), lock_collateral, None)
	verify {
		assert!(<FeeMarket<T>>::is_enrolled(&relayer));
		assert_eq!(<FeeMarket<T>>::relayers().unwrap().len(), 5);
	}

	update_locked_collateral {
		fee_market_ready::<T>();
		let caller3: T::AccountId = account("source", 3, SEED);
		let new_collateral = T::CollateralPerOrder::get().saturating_mul(5u32.into());
	}: update_locked_collateral(RawOrigin::Signed(caller3.clone()), new_collateral)
	verify {
		let relayer = <FeeMarket<T>>::relayer(&caller3).unwrap();
		assert_eq!(relayer.collateral,  T::CollateralPerOrder::get().saturating_mul(5u32.into()));
	}

	update_relay_fee {
		fee_market_ready::<T>();
		let caller3: T::AccountId = account("source", 3, SEED);
		let new_fee = T::CollateralPerOrder::get().saturating_mul(10u32.into());
	}: update_relay_fee(RawOrigin::Signed(caller3.clone()), new_fee)
	verify {
		let relayer = <FeeMarket<T>>::relayer(&caller3).unwrap();
		assert_eq!(relayer.fee,  T::CollateralPerOrder::get().saturating_mul(10u32.into()));
	}

	cancel_enrollment {
		fee_market_ready::<T>();
		let caller1: T::AccountId = account("source", 1, SEED);
	}: cancel_enrollment(RawOrigin::Signed(caller1.clone()))
	verify {
		assert!(!<FeeMarket<T>>::is_enrolled(&caller1));
		assert_eq!(<FeeMarket<T>>::relayers().unwrap().len(), 3);
	}

	set_slash_protect {
	}:set_slash_protect(RawOrigin::Root, T::CollateralPerOrder::get().saturating_mul(1u32.into()))

	set_assigned_relayers_number{
		fee_market_ready::<T>();
	}: set_assigned_relayers_number(RawOrigin::Root, 1)
	verify {
		assert_eq!(<FeeMarket<T>>::assigned_relayers().unwrap().len(), 1);
	}
}
