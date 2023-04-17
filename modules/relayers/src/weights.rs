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

//! Autogenerated weights for pallet_bridge_relayers
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2023-04-13, STEPS: `50`, REPEAT: `20`, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! WORST CASE MAP SIZE: `1000000`
//! HOSTNAME: `covid`, CPU: `11th Gen Intel(R) Core(TM) i7-11800H @ 2.30GHz`
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("dev"), DB CACHE: 1024

// Executed Command:
// target/release/millau-bridge-node
// benchmark
// pallet
// --chain=dev
// --steps=50
// --repeat=20
// --pallet=pallet_bridge_relayers
// --extrinsic=*
// --execution=wasm
// --wasm-execution=Compiled
// --heap-pages=4096
// --output=./modules/relayers/src/weights.rs
// --template=./.maintain/millau-weight-template.hbs

#![allow(clippy::all)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use sp_std::marker::PhantomData;

/// Weight functions needed for pallet_bridge_relayers.
pub trait WeightInfo {
	fn claim_rewards() -> Weight;
	fn register() -> Weight;
	fn deregister() -> Weight;
}

/// Weights for `pallet_bridge_relayers` that are generated using one of the Bridge testnets.
///
/// Those weights are test only and must never be used in production.
pub struct BridgeWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for BridgeWeight<T> {
	/// Storage: BridgeRelayers RelayerRewards (r:1 w:1)
	///
	/// Proof: BridgeRelayers RelayerRewards (max_values: None, max_size: Some(65), added: 2540,
	/// mode: MaxEncodedLen)
	///
	/// Storage: Balances TotalIssuance (r:1 w:0)
	///
	/// Proof: Balances TotalIssuance (max_values: Some(1), max_size: Some(8), added: 503, mode:
	/// MaxEncodedLen)
	///
	/// Storage: System Account (r:1 w:1)
	///
	/// Proof: System Account (max_values: None, max_size: Some(104), added: 2579, mode:
	/// MaxEncodedLen)
	fn claim_rewards() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `294`
		//  Estimated: `8592`
		// Minimum execution time: 76_500 nanoseconds.
		Weight::from_parts(77_615_000, 8592)
			.saturating_add(T::DbWeight::get().reads(3_u64))
			.saturating_add(T::DbWeight::get().writes(2_u64))
	}
	/// Storage: BridgeRelayers RegisteredRelayers (r:1 w:1)
	///
	/// Proof: BridgeRelayers RegisteredRelayers (max_values: None, max_size: Some(64), added: 2539,
	/// mode: MaxEncodedLen)
	///
	/// Storage: Balances Reserves (r:1 w:1)
	///
	/// Proof: Balances Reserves (max_values: None, max_size: Some(849), added: 3324, mode:
	/// MaxEncodedLen)
	fn register() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `87`
		//  Estimated: `7843`
		// Minimum execution time: 39_384 nanoseconds.
		Weight::from_parts(39_989_000, 7843)
			.saturating_add(T::DbWeight::get().reads(2_u64))
			.saturating_add(T::DbWeight::get().writes(2_u64))
	}
	/// Storage: BridgeRelayers RegisteredRelayers (r:1 w:1)
	///
	/// Proof: BridgeRelayers RegisteredRelayers (max_values: None, max_size: Some(64), added: 2539,
	/// mode: MaxEncodedLen)
	///
	/// Storage: Balances Reserves (r:1 w:1)
	///
	/// Proof: Balances Reserves (max_values: None, max_size: Some(849), added: 3324, mode:
	/// MaxEncodedLen)
	fn deregister() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `264`
		//  Estimated: `7843`
		// Minimum execution time: 43_907 nanoseconds.
		Weight::from_parts(44_830_000, 7843)
			.saturating_add(T::DbWeight::get().reads(2_u64))
			.saturating_add(T::DbWeight::get().writes(2_u64))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	/// Storage: BridgeRelayers RelayerRewards (r:1 w:1)
	///
	/// Proof: BridgeRelayers RelayerRewards (max_values: None, max_size: Some(65), added: 2540,
	/// mode: MaxEncodedLen)
	///
	/// Storage: Balances TotalIssuance (r:1 w:0)
	///
	/// Proof: Balances TotalIssuance (max_values: Some(1), max_size: Some(8), added: 503, mode:
	/// MaxEncodedLen)
	///
	/// Storage: System Account (r:1 w:1)
	///
	/// Proof: System Account (max_values: None, max_size: Some(104), added: 2579, mode:
	/// MaxEncodedLen)
	fn claim_rewards() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `294`
		//  Estimated: `8592`
		// Minimum execution time: 76_500 nanoseconds.
		Weight::from_parts(77_615_000, 8592)
			.saturating_add(RocksDbWeight::get().reads(3_u64))
			.saturating_add(RocksDbWeight::get().writes(2_u64))
	}
	/// Storage: BridgeRelayers RegisteredRelayers (r:1 w:1)
	///
	/// Proof: BridgeRelayers RegisteredRelayers (max_values: None, max_size: Some(64), added: 2539,
	/// mode: MaxEncodedLen)
	///
	/// Storage: Balances Reserves (r:1 w:1)
	///
	/// Proof: Balances Reserves (max_values: None, max_size: Some(849), added: 3324, mode:
	/// MaxEncodedLen)
	fn register() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `87`
		//  Estimated: `7843`
		// Minimum execution time: 39_384 nanoseconds.
		Weight::from_parts(39_989_000, 7843)
			.saturating_add(RocksDbWeight::get().reads(2_u64))
			.saturating_add(RocksDbWeight::get().writes(2_u64))
	}
	/// Storage: BridgeRelayers RegisteredRelayers (r:1 w:1)
	///
	/// Proof: BridgeRelayers RegisteredRelayers (max_values: None, max_size: Some(64), added: 2539,
	/// mode: MaxEncodedLen)
	///
	/// Storage: Balances Reserves (r:1 w:1)
	///
	/// Proof: Balances Reserves (max_values: None, max_size: Some(849), added: 3324, mode:
	/// MaxEncodedLen)
	fn deregister() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `264`
		//  Estimated: `7843`
		// Minimum execution time: 43_907 nanoseconds.
		Weight::from_parts(44_830_000, 7843)
			.saturating_add(RocksDbWeight::get().reads(2_u64))
			.saturating_add(RocksDbWeight::get().writes(2_u64))
	}
}
