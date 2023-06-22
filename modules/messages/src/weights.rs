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

//! Autogenerated weights for pallet_bridge_messages
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2023-06-22, STEPS: `50`, REPEAT: `20`, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! WORST CASE MAP SIZE: `1000000`
//! HOSTNAME: `serban-ROG-Zephyrus`, CPU: `12th Gen Intel(R) Core(TM) i7-12700H`
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("dev"), DB CACHE: 1024

// Executed Command:
// target/release/millau-bridge-node
// benchmark
// pallet
// --chain=dev
// --steps=50
// --repeat=20
// --pallet=pallet_bridge_messages
// --extrinsic=*
// --execution=wasm
// --wasm-execution=Compiled
// --heap-pages=4096
// --output=./modules/messages/src/weights.rs
// --template=./.maintain/bridge-weight-template.hbs

#![allow(clippy::all)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use sp_std::marker::PhantomData;

/// Weight functions needed for pallet_bridge_messages.
pub trait WeightInfo {
	fn receive_single_message_proof() -> Weight;
	fn receive_n_messages_proof(n: u32) -> Weight;
	fn receive_single_message_proof_with_outbound_lane_state() -> Weight;
	fn receive_single_n_bytes_message_proof(n: u32) -> Weight;
	fn receive_delivery_proof_for_single_message() -> Weight;
	fn receive_delivery_proof_for_two_messages_by_single_relayer() -> Weight;
	fn receive_delivery_proof_for_two_messages_by_two_relayers() -> Weight;
	fn receive_single_n_bytes_message_proof_with_dispatch(n: u32) -> Weight;
}

/// Weights for `pallet_bridge_messages` that are generated using one of the Bridge testnets.
///
/// Those weights are test only and must never be used in production.
pub struct BridgeWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for BridgeWeight<T> {
	/// Storage: BridgeRialtoParachainMessages PalletOperatingMode (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachainMessages PalletOperatingMode (max_values: Some(1), max_size:
	/// Some(2), added: 497, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachains ImportedParaHeads (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachains ImportedParaHeads (max_values: Some(1024), max_size:
	/// Some(196), added: 1681, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachainMessages InboundLanes (r:1 w:1)
	///
	/// Proof: BridgeRialtoParachainMessages InboundLanes (max_values: None, max_size: Some(49180),
	/// added: 51655, mode: MaxEncodedLen)
	fn receive_single_message_proof() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `495`
		//  Estimated: `52645`
		// Minimum execution time: 33_026 nanoseconds.
		Weight::from_parts(34_520_000, 52645)
			.saturating_add(T::DbWeight::get().reads(3_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	/// Storage: BridgeRialtoParachainMessages PalletOperatingMode (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachainMessages PalletOperatingMode (max_values: Some(1), max_size:
	/// Some(2), added: 497, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachains ImportedParaHeads (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachains ImportedParaHeads (max_values: Some(1024), max_size:
	/// Some(196), added: 1681, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachainMessages InboundLanes (r:1 w:1)
	///
	/// Proof: BridgeRialtoParachainMessages InboundLanes (max_values: None, max_size: Some(49180),
	/// added: 51655, mode: MaxEncodedLen)
	///
	/// The range of component `n` is `[1, 1004]`.
	///
	/// The range of component `n` is `[1, 1004]`.
	fn receive_n_messages_proof(n: u32) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `495`
		//  Estimated: `52645`
		// Minimum execution time: 33_441 nanoseconds.
		Weight::from_parts(23_668_338, 52645)
			// Standard Error: 2_400
			.saturating_add(Weight::from_parts(7_198_921, 0).saturating_mul(n.into()))
			.saturating_add(T::DbWeight::get().reads(3_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	/// Storage: BridgeRialtoParachainMessages PalletOperatingMode (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachainMessages PalletOperatingMode (max_values: Some(1), max_size:
	/// Some(2), added: 497, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachains ImportedParaHeads (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachains ImportedParaHeads (max_values: Some(1024), max_size:
	/// Some(196), added: 1681, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachainMessages InboundLanes (r:1 w:1)
	///
	/// Proof: BridgeRialtoParachainMessages InboundLanes (max_values: None, max_size: Some(49180),
	/// added: 51655, mode: MaxEncodedLen)
	fn receive_single_message_proof_with_outbound_lane_state() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `495`
		//  Estimated: `52645`
		// Minimum execution time: 39_693 nanoseconds.
		Weight::from_parts(41_483_000, 52645)
			.saturating_add(T::DbWeight::get().reads(3_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	/// Storage: BridgeRialtoParachainMessages PalletOperatingMode (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachainMessages PalletOperatingMode (max_values: Some(1), max_size:
	/// Some(2), added: 497, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachains ImportedParaHeads (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachains ImportedParaHeads (max_values: Some(1024), max_size:
	/// Some(196), added: 1681, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachainMessages InboundLanes (r:1 w:1)
	///
	/// Proof: BridgeRialtoParachainMessages InboundLanes (max_values: None, max_size: Some(49180),
	/// added: 51655, mode: MaxEncodedLen)
	///
	/// The range of component `n` is `[1, 16384]`.
	///
	/// The range of component `n` is `[1, 16384]`.
	fn receive_single_n_bytes_message_proof(n: u32) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `495`
		//  Estimated: `52645`
		// Minimum execution time: 33_614 nanoseconds.
		Weight::from_parts(35_220_911, 52645)
			// Standard Error: 4
			.saturating_add(Weight::from_parts(1_224, 0).saturating_mul(n.into()))
			.saturating_add(T::DbWeight::get().reads(3_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	/// Storage: BridgeRialtoParachainMessages PalletOperatingMode (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachainMessages PalletOperatingMode (max_values: Some(1), max_size:
	/// Some(2), added: 497, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachains ImportedParaHeads (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachains ImportedParaHeads (max_values: Some(1024), max_size:
	/// Some(196), added: 1681, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachainMessages OutboundLanes (r:1 w:1)
	///
	/// Proof: BridgeRialtoParachainMessages OutboundLanes (max_values: None, max_size: Some(45),
	/// added: 2520, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRelayers RelayerRewards (r:1 w:1)
	///
	/// Proof: BridgeRelayers RelayerRewards (max_values: None, max_size: Some(65), added: 2540,
	/// mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachainMessages OutboundMessages (r:0 w:1)
	///
	/// Proof: BridgeRialtoParachainMessages OutboundMessages (max_values: None, max_size:
	/// Some(65568), added: 68043, mode: MaxEncodedLen)
	fn receive_delivery_proof_for_single_message() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `506`
		//  Estimated: `3530`
		// Minimum execution time: 34_552 nanoseconds.
		Weight::from_parts(35_414_000, 3530)
			.saturating_add(T::DbWeight::get().reads(4_u64))
			.saturating_add(T::DbWeight::get().writes(3_u64))
	}
	/// Storage: BridgeRialtoParachainMessages PalletOperatingMode (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachainMessages PalletOperatingMode (max_values: Some(1), max_size:
	/// Some(2), added: 497, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachains ImportedParaHeads (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachains ImportedParaHeads (max_values: Some(1024), max_size:
	/// Some(196), added: 1681, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachainMessages OutboundLanes (r:1 w:1)
	///
	/// Proof: BridgeRialtoParachainMessages OutboundLanes (max_values: None, max_size: Some(45),
	/// added: 2520, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRelayers RelayerRewards (r:1 w:1)
	///
	/// Proof: BridgeRelayers RelayerRewards (max_values: None, max_size: Some(65), added: 2540,
	/// mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachainMessages OutboundMessages (r:0 w:2)
	///
	/// Proof: BridgeRialtoParachainMessages OutboundMessages (max_values: None, max_size:
	/// Some(65568), added: 68043, mode: MaxEncodedLen)
	fn receive_delivery_proof_for_two_messages_by_single_relayer() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `506`
		//  Estimated: `3530`
		// Minimum execution time: 35_743 nanoseconds.
		Weight::from_parts(36_792_000, 3530)
			.saturating_add(T::DbWeight::get().reads(4_u64))
			.saturating_add(T::DbWeight::get().writes(4_u64))
	}
	/// Storage: BridgeRialtoParachainMessages PalletOperatingMode (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachainMessages PalletOperatingMode (max_values: Some(1), max_size:
	/// Some(2), added: 497, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachains ImportedParaHeads (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachains ImportedParaHeads (max_values: Some(1024), max_size:
	/// Some(196), added: 1681, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachainMessages OutboundLanes (r:1 w:1)
	///
	/// Proof: BridgeRialtoParachainMessages OutboundLanes (max_values: None, max_size: Some(45),
	/// added: 2520, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRelayers RelayerRewards (r:2 w:2)
	///
	/// Proof: BridgeRelayers RelayerRewards (max_values: None, max_size: Some(65), added: 2540,
	/// mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachainMessages OutboundMessages (r:0 w:2)
	///
	/// Proof: BridgeRialtoParachainMessages OutboundMessages (max_values: None, max_size:
	/// Some(65568), added: 68043, mode: MaxEncodedLen)
	fn receive_delivery_proof_for_two_messages_by_two_relayers() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `506`
		//  Estimated: `6070`
		// Minimum execution time: 38_031 nanoseconds.
		Weight::from_parts(39_537_000, 6070)
			.saturating_add(T::DbWeight::get().reads(5_u64))
			.saturating_add(T::DbWeight::get().writes(5_u64))
	}
	/// Storage: BridgeRialtoParachainMessages PalletOperatingMode (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachainMessages PalletOperatingMode (max_values: Some(1), max_size:
	/// Some(2), added: 497, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachains ImportedParaHeads (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachains ImportedParaHeads (max_values: Some(1024), max_size:
	/// Some(196), added: 1681, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachainMessages InboundLanes (r:1 w:1)
	///
	/// Proof: BridgeRialtoParachainMessages InboundLanes (max_values: None, max_size: Some(49180),
	/// added: 51655, mode: MaxEncodedLen)
	///
	/// The range of component `n` is `[1, 16384]`.
	///
	/// The range of component `n` is `[1, 16384]`.
	fn receive_single_n_bytes_message_proof_with_dispatch(n: u32) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `495`
		//  Estimated: `52645`
		// Minimum execution time: 33_547 nanoseconds.
		Weight::from_parts(33_924_000, 52645)
			// Standard Error: 894
			.saturating_add(Weight::from_parts(394_236, 0).saturating_mul(n.into()))
			.saturating_add(T::DbWeight::get().reads(3_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	/// Storage: BridgeRialtoParachainMessages PalletOperatingMode (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachainMessages PalletOperatingMode (max_values: Some(1), max_size:
	/// Some(2), added: 497, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachains ImportedParaHeads (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachains ImportedParaHeads (max_values: Some(1024), max_size:
	/// Some(196), added: 1681, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachainMessages InboundLanes (r:1 w:1)
	///
	/// Proof: BridgeRialtoParachainMessages InboundLanes (max_values: None, max_size: Some(49180),
	/// added: 51655, mode: MaxEncodedLen)
	fn receive_single_message_proof() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `495`
		//  Estimated: `52645`
		// Minimum execution time: 33_026 nanoseconds.
		Weight::from_parts(34_520_000, 52645)
			.saturating_add(RocksDbWeight::get().reads(3_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: BridgeRialtoParachainMessages PalletOperatingMode (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachainMessages PalletOperatingMode (max_values: Some(1), max_size:
	/// Some(2), added: 497, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachains ImportedParaHeads (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachains ImportedParaHeads (max_values: Some(1024), max_size:
	/// Some(196), added: 1681, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachainMessages InboundLanes (r:1 w:1)
	///
	/// Proof: BridgeRialtoParachainMessages InboundLanes (max_values: None, max_size: Some(49180),
	/// added: 51655, mode: MaxEncodedLen)
	///
	/// The range of component `n` is `[1, 1004]`.
	///
	/// The range of component `n` is `[1, 1004]`.
	fn receive_n_messages_proof(n: u32) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `495`
		//  Estimated: `52645`
		// Minimum execution time: 33_441 nanoseconds.
		Weight::from_parts(23_668_338, 52645)
			// Standard Error: 2_400
			.saturating_add(Weight::from_parts(7_198_921, 0).saturating_mul(n.into()))
			.saturating_add(RocksDbWeight::get().reads(3_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: BridgeRialtoParachainMessages PalletOperatingMode (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachainMessages PalletOperatingMode (max_values: Some(1), max_size:
	/// Some(2), added: 497, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachains ImportedParaHeads (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachains ImportedParaHeads (max_values: Some(1024), max_size:
	/// Some(196), added: 1681, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachainMessages InboundLanes (r:1 w:1)
	///
	/// Proof: BridgeRialtoParachainMessages InboundLanes (max_values: None, max_size: Some(49180),
	/// added: 51655, mode: MaxEncodedLen)
	fn receive_single_message_proof_with_outbound_lane_state() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `495`
		//  Estimated: `52645`
		// Minimum execution time: 39_693 nanoseconds.
		Weight::from_parts(41_483_000, 52645)
			.saturating_add(RocksDbWeight::get().reads(3_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: BridgeRialtoParachainMessages PalletOperatingMode (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachainMessages PalletOperatingMode (max_values: Some(1), max_size:
	/// Some(2), added: 497, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachains ImportedParaHeads (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachains ImportedParaHeads (max_values: Some(1024), max_size:
	/// Some(196), added: 1681, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachainMessages InboundLanes (r:1 w:1)
	///
	/// Proof: BridgeRialtoParachainMessages InboundLanes (max_values: None, max_size: Some(49180),
	/// added: 51655, mode: MaxEncodedLen)
	///
	/// The range of component `n` is `[1, 16384]`.
	///
	/// The range of component `n` is `[1, 16384]`.
	fn receive_single_n_bytes_message_proof(n: u32) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `495`
		//  Estimated: `52645`
		// Minimum execution time: 33_614 nanoseconds.
		Weight::from_parts(35_220_911, 52645)
			// Standard Error: 4
			.saturating_add(Weight::from_parts(1_224, 0).saturating_mul(n.into()))
			.saturating_add(RocksDbWeight::get().reads(3_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: BridgeRialtoParachainMessages PalletOperatingMode (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachainMessages PalletOperatingMode (max_values: Some(1), max_size:
	/// Some(2), added: 497, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachains ImportedParaHeads (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachains ImportedParaHeads (max_values: Some(1024), max_size:
	/// Some(196), added: 1681, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachainMessages OutboundLanes (r:1 w:1)
	///
	/// Proof: BridgeRialtoParachainMessages OutboundLanes (max_values: None, max_size: Some(45),
	/// added: 2520, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRelayers RelayerRewards (r:1 w:1)
	///
	/// Proof: BridgeRelayers RelayerRewards (max_values: None, max_size: Some(65), added: 2540,
	/// mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachainMessages OutboundMessages (r:0 w:1)
	///
	/// Proof: BridgeRialtoParachainMessages OutboundMessages (max_values: None, max_size:
	/// Some(65568), added: 68043, mode: MaxEncodedLen)
	fn receive_delivery_proof_for_single_message() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `506`
		//  Estimated: `3530`
		// Minimum execution time: 34_552 nanoseconds.
		Weight::from_parts(35_414_000, 3530)
			.saturating_add(RocksDbWeight::get().reads(4_u64))
			.saturating_add(RocksDbWeight::get().writes(3_u64))
	}
	/// Storage: BridgeRialtoParachainMessages PalletOperatingMode (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachainMessages PalletOperatingMode (max_values: Some(1), max_size:
	/// Some(2), added: 497, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachains ImportedParaHeads (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachains ImportedParaHeads (max_values: Some(1024), max_size:
	/// Some(196), added: 1681, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachainMessages OutboundLanes (r:1 w:1)
	///
	/// Proof: BridgeRialtoParachainMessages OutboundLanes (max_values: None, max_size: Some(45),
	/// added: 2520, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRelayers RelayerRewards (r:1 w:1)
	///
	/// Proof: BridgeRelayers RelayerRewards (max_values: None, max_size: Some(65), added: 2540,
	/// mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachainMessages OutboundMessages (r:0 w:2)
	///
	/// Proof: BridgeRialtoParachainMessages OutboundMessages (max_values: None, max_size:
	/// Some(65568), added: 68043, mode: MaxEncodedLen)
	fn receive_delivery_proof_for_two_messages_by_single_relayer() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `506`
		//  Estimated: `3530`
		// Minimum execution time: 35_743 nanoseconds.
		Weight::from_parts(36_792_000, 3530)
			.saturating_add(RocksDbWeight::get().reads(4_u64))
			.saturating_add(RocksDbWeight::get().writes(4_u64))
	}
	/// Storage: BridgeRialtoParachainMessages PalletOperatingMode (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachainMessages PalletOperatingMode (max_values: Some(1), max_size:
	/// Some(2), added: 497, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachains ImportedParaHeads (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachains ImportedParaHeads (max_values: Some(1024), max_size:
	/// Some(196), added: 1681, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachainMessages OutboundLanes (r:1 w:1)
	///
	/// Proof: BridgeRialtoParachainMessages OutboundLanes (max_values: None, max_size: Some(45),
	/// added: 2520, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRelayers RelayerRewards (r:2 w:2)
	///
	/// Proof: BridgeRelayers RelayerRewards (max_values: None, max_size: Some(65), added: 2540,
	/// mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachainMessages OutboundMessages (r:0 w:2)
	///
	/// Proof: BridgeRialtoParachainMessages OutboundMessages (max_values: None, max_size:
	/// Some(65568), added: 68043, mode: MaxEncodedLen)
	fn receive_delivery_proof_for_two_messages_by_two_relayers() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `506`
		//  Estimated: `6070`
		// Minimum execution time: 38_031 nanoseconds.
		Weight::from_parts(39_537_000, 6070)
			.saturating_add(RocksDbWeight::get().reads(5_u64))
			.saturating_add(RocksDbWeight::get().writes(5_u64))
	}
	/// Storage: BridgeRialtoParachainMessages PalletOperatingMode (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachainMessages PalletOperatingMode (max_values: Some(1), max_size:
	/// Some(2), added: 497, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachains ImportedParaHeads (r:1 w:0)
	///
	/// Proof: BridgeRialtoParachains ImportedParaHeads (max_values: Some(1024), max_size:
	/// Some(196), added: 1681, mode: MaxEncodedLen)
	///
	/// Storage: BridgeRialtoParachainMessages InboundLanes (r:1 w:1)
	///
	/// Proof: BridgeRialtoParachainMessages InboundLanes (max_values: None, max_size: Some(49180),
	/// added: 51655, mode: MaxEncodedLen)
	///
	/// The range of component `n` is `[1, 16384]`.
	///
	/// The range of component `n` is `[1, 16384]`.
	fn receive_single_n_bytes_message_proof_with_dispatch(n: u32) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `495`
		//  Estimated: `52645`
		// Minimum execution time: 33_547 nanoseconds.
		Weight::from_parts(33_924_000, 52645)
			// Standard Error: 894
			.saturating_add(Weight::from_parts(394_236, 0).saturating_mul(n.into()))
			.saturating_add(RocksDbWeight::get().reads(3_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
}
