
//! Autogenerated weights for `pallet_bridge_messages`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2023-05-26, STEPS: `50`, REPEAT: `20`, LOW RANGE: `[]`, HIGH RANGE: `[]`
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
	/// Storage: BridgeRialtoParachainMessages PalletOperatingMode (r:1 w:0)
	/// Proof: BridgeRialtoParachainMessages PalletOperatingMode (max_values: Some(1), max_size: Some(2), added: 497, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoParachains ImportedParaHeads (r:1 w:0)
	/// Proof: BridgeRialtoParachains ImportedParaHeads (max_values: Some(1024), max_size: Some(196), added: 1681, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoParachainMessages InboundLanes (r:1 w:1)
	/// Proof: BridgeRialtoParachainMessages InboundLanes (max_values: None, max_size: Some(49180), added: 51655, mode: MaxEncodedLen)
	fn receive_single_message_proof() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `428`
		//  Estimated: `52645`
		// Minimum execution time: 35_150_000 picoseconds.
		Weight::from_parts(36_665_000, 0)
			.saturating_add(Weight::from_parts(0, 52645))
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: BridgeRialtoParachainMessages PalletOperatingMode (r:1 w:0)
	/// Proof: BridgeRialtoParachainMessages PalletOperatingMode (max_values: Some(1), max_size: Some(2), added: 497, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoParachains ImportedParaHeads (r:1 w:0)
	/// Proof: BridgeRialtoParachains ImportedParaHeads (max_values: Some(1024), max_size: Some(196), added: 1681, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoParachainMessages InboundLanes (r:1 w:1)
	/// Proof: BridgeRialtoParachainMessages InboundLanes (max_values: None, max_size: Some(49180), added: 51655, mode: MaxEncodedLen)
	fn receive_two_messages_proof() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `428`
		//  Estimated: `52645`
		// Minimum execution time: 44_082_000 picoseconds.
		Weight::from_parts(47_509_000, 0)
			.saturating_add(Weight::from_parts(0, 52645))
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: BridgeRialtoParachainMessages PalletOperatingMode (r:1 w:0)
	/// Proof: BridgeRialtoParachainMessages PalletOperatingMode (max_values: Some(1), max_size: Some(2), added: 497, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoParachains ImportedParaHeads (r:1 w:0)
	/// Proof: BridgeRialtoParachains ImportedParaHeads (max_values: Some(1024), max_size: Some(196), added: 1681, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoParachainMessages InboundLanes (r:1 w:1)
	/// Proof: BridgeRialtoParachainMessages InboundLanes (max_values: None, max_size: Some(49180), added: 51655, mode: MaxEncodedLen)
	fn receive_single_message_proof_with_outbound_lane_state() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `428`
		//  Estimated: `52645`
		// Minimum execution time: 40_102_000 picoseconds.
		Weight::from_parts(43_872_000, 0)
			.saturating_add(Weight::from_parts(0, 52645))
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: BridgeRialtoParachainMessages PalletOperatingMode (r:1 w:0)
	/// Proof: BridgeRialtoParachainMessages PalletOperatingMode (max_values: Some(1), max_size: Some(2), added: 497, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoParachains ImportedParaHeads (r:1 w:0)
	/// Proof: BridgeRialtoParachains ImportedParaHeads (max_values: Some(1024), max_size: Some(196), added: 1681, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoParachainMessages InboundLanes (r:1 w:1)
	/// Proof: BridgeRialtoParachainMessages InboundLanes (max_values: None, max_size: Some(49180), added: 51655, mode: MaxEncodedLen)
	fn receive_single_message_proof_1_kb() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `428`
		//  Estimated: `52645`
		// Minimum execution time: 35_144_000 picoseconds.
		Weight::from_parts(36_876_000, 0)
			.saturating_add(Weight::from_parts(0, 52645))
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: BridgeRialtoParachainMessages PalletOperatingMode (r:1 w:0)
	/// Proof: BridgeRialtoParachainMessages PalletOperatingMode (max_values: Some(1), max_size: Some(2), added: 497, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoParachains ImportedParaHeads (r:1 w:0)
	/// Proof: BridgeRialtoParachains ImportedParaHeads (max_values: Some(1024), max_size: Some(196), added: 1681, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoParachainMessages InboundLanes (r:1 w:1)
	/// Proof: BridgeRialtoParachainMessages InboundLanes (max_values: None, max_size: Some(49180), added: 51655, mode: MaxEncodedLen)
	fn receive_single_message_proof_16_kb() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `428`
		//  Estimated: `52645`
		// Minimum execution time: 54_479_000 picoseconds.
		Weight::from_parts(56_344_000, 0)
			.saturating_add(Weight::from_parts(0, 52645))
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: BridgeRialtoParachainMessages PalletOperatingMode (r:1 w:0)
	/// Proof: BridgeRialtoParachainMessages PalletOperatingMode (max_values: Some(1), max_size: Some(2), added: 497, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoParachains ImportedParaHeads (r:1 w:0)
	/// Proof: BridgeRialtoParachains ImportedParaHeads (max_values: Some(1024), max_size: Some(196), added: 1681, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoParachainMessages OutboundLanes (r:1 w:1)
	/// Proof: BridgeRialtoParachainMessages OutboundLanes (max_values: Some(1), max_size: Some(44), added: 539, mode: MaxEncodedLen)
	/// Storage: BridgeRelayers RelayerRewards (r:1 w:1)
	/// Proof: BridgeRelayers RelayerRewards (max_values: None, max_size: Some(65), added: 2540, mode: MaxEncodedLen)
	fn receive_delivery_proof_for_single_message() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `453`
		//  Estimated: `3530`
		// Minimum execution time: 33_503_000 picoseconds.
		Weight::from_parts(35_653_000, 0)
			.saturating_add(Weight::from_parts(0, 3530))
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	/// Storage: BridgeRialtoParachainMessages PalletOperatingMode (r:1 w:0)
	/// Proof: BridgeRialtoParachainMessages PalletOperatingMode (max_values: Some(1), max_size: Some(2), added: 497, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoParachains ImportedParaHeads (r:1 w:0)
	/// Proof: BridgeRialtoParachains ImportedParaHeads (max_values: Some(1024), max_size: Some(196), added: 1681, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoParachainMessages OutboundLanes (r:1 w:1)
	/// Proof: BridgeRialtoParachainMessages OutboundLanes (max_values: Some(1), max_size: Some(44), added: 539, mode: MaxEncodedLen)
	/// Storage: BridgeRelayers RelayerRewards (r:1 w:1)
	/// Proof: BridgeRelayers RelayerRewards (max_values: None, max_size: Some(65), added: 2540, mode: MaxEncodedLen)
	fn receive_delivery_proof_for_two_messages_by_single_relayer() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `470`
		//  Estimated: `3530`
		// Minimum execution time: 32_651_000 picoseconds.
		Weight::from_parts(34_135_000, 0)
			.saturating_add(Weight::from_parts(0, 3530))
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	/// Storage: BridgeRialtoParachainMessages PalletOperatingMode (r:1 w:0)
	/// Proof: BridgeRialtoParachainMessages PalletOperatingMode (max_values: Some(1), max_size: Some(2), added: 497, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoParachains ImportedParaHeads (r:1 w:0)
	/// Proof: BridgeRialtoParachains ImportedParaHeads (max_values: Some(1024), max_size: Some(196), added: 1681, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoParachainMessages OutboundLanes (r:1 w:1)
	/// Proof: BridgeRialtoParachainMessages OutboundLanes (max_values: Some(1), max_size: Some(44), added: 539, mode: MaxEncodedLen)
	/// Storage: BridgeRelayers RelayerRewards (r:2 w:2)
	/// Proof: BridgeRelayers RelayerRewards (max_values: None, max_size: Some(65), added: 2540, mode: MaxEncodedLen)
	fn receive_delivery_proof_for_two_messages_by_two_relayers() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `470`
		//  Estimated: `6070`
		// Minimum execution time: 34_746_000 picoseconds.
		Weight::from_parts(36_357_000, 0)
			.saturating_add(Weight::from_parts(0, 6070))
			.saturating_add(T::DbWeight::get().reads(5))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	/// Storage: BridgeRialtoParachainMessages PalletOperatingMode (r:1 w:0)
	/// Proof: BridgeRialtoParachainMessages PalletOperatingMode (max_values: Some(1), max_size: Some(2), added: 497, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoParachains ImportedParaHeads (r:1 w:0)
	/// Proof: BridgeRialtoParachains ImportedParaHeads (max_values: Some(1024), max_size: Some(196), added: 1681, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoParachainMessages InboundLanes (r:1 w:1)
	/// Proof: BridgeRialtoParachainMessages InboundLanes (max_values: None, max_size: Some(49180), added: 51655, mode: MaxEncodedLen)
	/// The range of component `i` is `[128, 2048]`.
	/// The range of component `i` is `[128, 2048]`.
	fn receive_single_message_proof_with_dispatch(i: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `428`
		//  Estimated: `52645`
		// Minimum execution time: 77_643_000 picoseconds.
		Weight::from_parts(69_015_529, 0)
			.saturating_add(Weight::from_parts(0, 52645))
			// Standard Error: 1_450
			.saturating_add(Weight::from_parts(304_163, 0).saturating_mul(i.into()))
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(1))
	}
}
