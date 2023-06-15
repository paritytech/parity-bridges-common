
//! Autogenerated weights for `pallet_bridge_messages`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2023-06-15, STEPS: `50`, REPEAT: `20`, LOW RANGE: `[]`, HIGH RANGE: `[]`
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
		// Minimum execution time: 46_589_000 picoseconds.
		Weight::from_parts(49_086_000, 0)
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
	/// The range of component `n` is `[1, 1004]`.
	/// The range of component `n` is `[1, 1004]`.
	fn receive_n_messages_proof(n: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `428`
		//  Estimated: `52645`
		// Minimum execution time: 47_603_000 picoseconds.
		Weight::from_parts(49_179_000, 0)
			.saturating_add(Weight::from_parts(0, 52645))
			// Standard Error: 66_399
			.saturating_add(Weight::from_parts(10_712_351, 0).saturating_mul(n.into()))
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
		// Minimum execution time: 55_669_000 picoseconds.
		Weight::from_parts(58_369_000, 0)
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
	/// The range of component `n` is `[1, 16]`.
	/// The range of component `n` is `[1, 16]`.
	fn receive_single_message_n_kb_proof(n: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `428`
		//  Estimated: `52645`
		// Minimum execution time: 48_479_000 picoseconds.
		Weight::from_parts(48_917_533, 0)
			.saturating_add(Weight::from_parts(0, 52645))
			// Standard Error: 4_157
			.saturating_add(Weight::from_parts(1_468_948, 0).saturating_mul(n.into()))
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
	/// Storage: BridgeRialtoParachainMessages OutboundMessages (r:0 w:1)
	/// Proof: BridgeRialtoParachainMessages OutboundMessages (max_values: None, max_size: Some(2621472), added: 2623947, mode: MaxEncodedLen)
	fn receive_delivery_proof_for_single_message() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `453`
		//  Estimated: `3530`
		// Minimum execution time: 43_189_000 picoseconds.
		Weight::from_parts(44_120_000, 0)
			.saturating_add(Weight::from_parts(0, 3530))
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	/// Storage: BridgeRialtoParachainMessages PalletOperatingMode (r:1 w:0)
	/// Proof: BridgeRialtoParachainMessages PalletOperatingMode (max_values: Some(1), max_size: Some(2), added: 497, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoParachains ImportedParaHeads (r:1 w:0)
	/// Proof: BridgeRialtoParachains ImportedParaHeads (max_values: Some(1024), max_size: Some(196), added: 1681, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoParachainMessages OutboundLanes (r:1 w:1)
	/// Proof: BridgeRialtoParachainMessages OutboundLanes (max_values: Some(1), max_size: Some(44), added: 539, mode: MaxEncodedLen)
	/// Storage: BridgeRelayers RelayerRewards (r:1 w:1)
	/// Proof: BridgeRelayers RelayerRewards (max_values: None, max_size: Some(65), added: 2540, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoParachainMessages OutboundMessages (r:0 w:2)
	/// Proof: BridgeRialtoParachainMessages OutboundMessages (max_values: None, max_size: Some(2621472), added: 2623947, mode: MaxEncodedLen)
	fn receive_delivery_proof_for_two_messages_by_single_relayer() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `470`
		//  Estimated: `3530`
		// Minimum execution time: 43_845_000 picoseconds.
		Weight::from_parts(44_937_000, 0)
			.saturating_add(Weight::from_parts(0, 3530))
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(4))
	}
	/// Storage: BridgeRialtoParachainMessages PalletOperatingMode (r:1 w:0)
	/// Proof: BridgeRialtoParachainMessages PalletOperatingMode (max_values: Some(1), max_size: Some(2), added: 497, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoParachains ImportedParaHeads (r:1 w:0)
	/// Proof: BridgeRialtoParachains ImportedParaHeads (max_values: Some(1024), max_size: Some(196), added: 1681, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoParachainMessages OutboundLanes (r:1 w:1)
	/// Proof: BridgeRialtoParachainMessages OutboundLanes (max_values: Some(1), max_size: Some(44), added: 539, mode: MaxEncodedLen)
	/// Storage: BridgeRelayers RelayerRewards (r:2 w:2)
	/// Proof: BridgeRelayers RelayerRewards (max_values: None, max_size: Some(65), added: 2540, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoParachainMessages OutboundMessages (r:0 w:2)
	/// Proof: BridgeRialtoParachainMessages OutboundMessages (max_values: None, max_size: Some(2621472), added: 2623947, mode: MaxEncodedLen)
	fn receive_delivery_proof_for_two_messages_by_two_relayers() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `470`
		//  Estimated: `6070`
		// Minimum execution time: 47_106_000 picoseconds.
		Weight::from_parts(48_426_000, 0)
			.saturating_add(Weight::from_parts(0, 6070))
			.saturating_add(T::DbWeight::get().reads(5))
			.saturating_add(T::DbWeight::get().writes(5))
	}
	/// Storage: BridgeRialtoParachainMessages PalletOperatingMode (r:1 w:0)
	/// Proof: BridgeRialtoParachainMessages PalletOperatingMode (max_values: Some(1), max_size: Some(2), added: 497, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoParachains ImportedParaHeads (r:1 w:0)
	/// Proof: BridgeRialtoParachains ImportedParaHeads (max_values: Some(1024), max_size: Some(196), added: 1681, mode: MaxEncodedLen)
	/// Storage: BridgeRialtoParachainMessages InboundLanes (r:1 w:1)
	/// Proof: BridgeRialtoParachainMessages InboundLanes (max_values: None, max_size: Some(49180), added: 51655, mode: MaxEncodedLen)
	/// The range of component `n` is `[128, 2048]`.
	/// The range of component `n` is `[128, 2048]`.
	fn receive_single_message_n_bytes_proof_with_dispatch(n: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `428`
		//  Estimated: `52645`
		// Minimum execution time: 102_818_000 picoseconds.
		Weight::from_parts(91_768_134, 0)
			.saturating_add(Weight::from_parts(0, 52645))
			// Standard Error: 2_202
			.saturating_add(Weight::from_parts(424_113, 0).saturating_mul(n.into()))
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(1))
	}
}
