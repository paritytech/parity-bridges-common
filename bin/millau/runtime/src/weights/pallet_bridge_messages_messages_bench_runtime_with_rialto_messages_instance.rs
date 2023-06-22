
//! Autogenerated weights for `pallet_bridge_messages`
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
// --output=./bin/millau/runtime/src/weights/

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::Weight};
use core::marker::PhantomData;

/// Weight functions for `pallet_bridge_messages`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_bridge_messages::WeightInfo for WeightInfo<T> {
	/// Storage: BridgeRialtoMessages PalletOperatingMode (r:1 w:0)
	/// Proof: BridgeRialtoMessages PalletOperatingMode (max_values: Some(1), max_size: Some(2), added: 497, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoGrandpa ImportedHeaders (r:1 w:0)
	/// Proof: BridgeRialtoGrandpa ImportedHeaders (max_values: Some(14400), max_size: Some(68), added: 2048, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoMessages InboundLanes (r:1 w:1)
	/// Proof: BridgeRialtoMessages InboundLanes (max_values: None, max_size: Some(49180), added: 51655, mode: MaxEncodedLen)
	fn receive_single_message_proof() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `557`
		//  Estimated: `52645`
		// Minimum execution time: 35_009_000 picoseconds.
		Weight::from_parts(36_535_000, 0)
			.saturating_add(Weight::from_parts(0, 52645))
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: BridgeRialtoMessages PalletOperatingMode (r:1 w:0)
	/// Proof: BridgeRialtoMessages PalletOperatingMode (max_values: Some(1), max_size: Some(2), added: 497, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoGrandpa ImportedHeaders (r:1 w:0)
	/// Proof: BridgeRialtoGrandpa ImportedHeaders (max_values: Some(14400), max_size: Some(68), added: 2048, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoMessages InboundLanes (r:1 w:1)
	/// Proof: BridgeRialtoMessages InboundLanes (max_values: None, max_size: Some(49180), added: 51655, mode: MaxEncodedLen)
	/// The range of component `n` is `[1, 1004]`.
	/// The range of component `n` is `[1, 1004]`.
	fn receive_n_messages_proof(n: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `557`
		//  Estimated: `52645`
		// Minimum execution time: 35_190_000 picoseconds.
		Weight::from_parts(34_992_137, 0)
			.saturating_add(Weight::from_parts(0, 52645))
			// Standard Error: 1_537
			.saturating_add(Weight::from_parts(7_409_473, 0).saturating_mul(n.into()))
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: BridgeRialtoMessages PalletOperatingMode (r:1 w:0)
	/// Proof: BridgeRialtoMessages PalletOperatingMode (max_values: Some(1), max_size: Some(2), added: 497, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoGrandpa ImportedHeaders (r:1 w:0)
	/// Proof: BridgeRialtoGrandpa ImportedHeaders (max_values: Some(14400), max_size: Some(68), added: 2048, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoMessages InboundLanes (r:1 w:1)
	/// Proof: BridgeRialtoMessages InboundLanes (max_values: None, max_size: Some(49180), added: 51655, mode: MaxEncodedLen)
	fn receive_single_message_proof_with_outbound_lane_state() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `557`
		//  Estimated: `52645`
		// Minimum execution time: 40_921_000 picoseconds.
		Weight::from_parts(42_805_000, 0)
			.saturating_add(Weight::from_parts(0, 52645))
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: BridgeRialtoMessages PalletOperatingMode (r:1 w:0)
	/// Proof: BridgeRialtoMessages PalletOperatingMode (max_values: Some(1), max_size: Some(2), added: 497, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoGrandpa ImportedHeaders (r:1 w:0)
	/// Proof: BridgeRialtoGrandpa ImportedHeaders (max_values: Some(14400), max_size: Some(68), added: 2048, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoMessages InboundLanes (r:1 w:1)
	/// Proof: BridgeRialtoMessages InboundLanes (max_values: None, max_size: Some(49180), added: 51655, mode: MaxEncodedLen)
	/// The range of component `n` is `[1, 16384]`.
	/// The range of component `n` is `[1, 16384]`.
	fn receive_single_n_bytes_message_proof(n: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `557`
		//  Estimated: `52645`
		// Minimum execution time: 34_806_000 picoseconds.
		Weight::from_parts(37_095_092, 0)
			.saturating_add(Weight::from_parts(0, 52645))
			// Standard Error: 5
			.saturating_add(Weight::from_parts(1_153, 0).saturating_mul(n.into()))
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: BridgeRialtoMessages PalletOperatingMode (r:1 w:0)
	/// Proof: BridgeRialtoMessages PalletOperatingMode (max_values: Some(1), max_size: Some(2), added: 497, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoGrandpa ImportedHeaders (r:1 w:0)
	/// Proof: BridgeRialtoGrandpa ImportedHeaders (max_values: Some(14400), max_size: Some(68), added: 2048, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoMessages OutboundLanes (r:1 w:1)
	/// Proof: BridgeRialtoMessages OutboundLanes (max_values: None, max_size: Some(45), added: 2520, mode: MaxEncodedLen)
	/// Storage: BridgeRelayers RelayerRewards (r:1 w:1)
	/// Proof: BridgeRelayers RelayerRewards (max_values: None, max_size: Some(65), added: 2540, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoMessages OutboundMessages (r:0 w:1)
	/// Proof: BridgeRialtoMessages OutboundMessages (max_values: None, max_size: Some(65568), added: 68043, mode: MaxEncodedLen)
	fn receive_delivery_proof_for_single_message() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `568`
		//  Estimated: `3530`
		// Minimum execution time: 35_707_000 picoseconds.
		Weight::from_parts(37_135_000, 0)
			.saturating_add(Weight::from_parts(0, 3530))
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	/// Storage: BridgeRialtoMessages PalletOperatingMode (r:1 w:0)
	/// Proof: BridgeRialtoMessages PalletOperatingMode (max_values: Some(1), max_size: Some(2), added: 497, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoGrandpa ImportedHeaders (r:1 w:0)
	/// Proof: BridgeRialtoGrandpa ImportedHeaders (max_values: Some(14400), max_size: Some(68), added: 2048, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoMessages OutboundLanes (r:1 w:1)
	/// Proof: BridgeRialtoMessages OutboundLanes (max_values: None, max_size: Some(45), added: 2520, mode: MaxEncodedLen)
	/// Storage: BridgeRelayers RelayerRewards (r:1 w:1)
	/// Proof: BridgeRelayers RelayerRewards (max_values: None, max_size: Some(65), added: 2540, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoMessages OutboundMessages (r:0 w:2)
	/// Proof: BridgeRialtoMessages OutboundMessages (max_values: None, max_size: Some(65568), added: 68043, mode: MaxEncodedLen)
	fn receive_delivery_proof_for_two_messages_by_single_relayer() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `568`
		//  Estimated: `3530`
		// Minimum execution time: 37_472_000 picoseconds.
		Weight::from_parts(38_392_000, 0)
			.saturating_add(Weight::from_parts(0, 3530))
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(4))
	}
	/// Storage: BridgeRialtoMessages PalletOperatingMode (r:1 w:0)
	/// Proof: BridgeRialtoMessages PalletOperatingMode (max_values: Some(1), max_size: Some(2), added: 497, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoGrandpa ImportedHeaders (r:1 w:0)
	/// Proof: BridgeRialtoGrandpa ImportedHeaders (max_values: Some(14400), max_size: Some(68), added: 2048, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoMessages OutboundLanes (r:1 w:1)
	/// Proof: BridgeRialtoMessages OutboundLanes (max_values: None, max_size: Some(45), added: 2520, mode: MaxEncodedLen)
	/// Storage: BridgeRelayers RelayerRewards (r:2 w:2)
	/// Proof: BridgeRelayers RelayerRewards (max_values: None, max_size: Some(65), added: 2540, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoMessages OutboundMessages (r:0 w:2)
	/// Proof: BridgeRialtoMessages OutboundMessages (max_values: None, max_size: Some(65568), added: 68043, mode: MaxEncodedLen)
	fn receive_delivery_proof_for_two_messages_by_two_relayers() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `568`
		//  Estimated: `6070`
		// Minimum execution time: 39_811_000 picoseconds.
		Weight::from_parts(40_845_000, 0)
			.saturating_add(Weight::from_parts(0, 6070))
			.saturating_add(T::DbWeight::get().reads(5))
			.saturating_add(T::DbWeight::get().writes(5))
	}
	/// Storage: BridgeRialtoMessages PalletOperatingMode (r:1 w:0)
	/// Proof: BridgeRialtoMessages PalletOperatingMode (max_values: Some(1), max_size: Some(2), added: 497, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoGrandpa ImportedHeaders (r:1 w:0)
	/// Proof: BridgeRialtoGrandpa ImportedHeaders (max_values: Some(14400), max_size: Some(68), added: 2048, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoMessages InboundLanes (r:1 w:1)
	/// Proof: BridgeRialtoMessages InboundLanes (max_values: None, max_size: Some(49180), added: 51655, mode: MaxEncodedLen)
	/// The range of component `n` is `[1, 16384]`.
	/// The range of component `n` is `[1, 16384]`.
	fn receive_single_n_bytes_message_proof_with_dispatch(n: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `557`
		//  Estimated: `52645`
		// Minimum execution time: 35_149_000 picoseconds.
		Weight::from_parts(35_776_000, 0)
			.saturating_add(Weight::from_parts(0, 52645))
			// Standard Error: 914
			.saturating_add(Weight::from_parts(396_099, 0).saturating_mul(n.into()))
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(1))
	}
}
