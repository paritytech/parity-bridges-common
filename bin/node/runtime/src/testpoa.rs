// Copyright 2020 Parity Technologies (UK) Ltd.
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

use frame_support::RuntimeDebug;
use hex_literal::hex;
use pallet_bridge_eth_poa::{
	AuraConfiguration, PruningStrategy as TPruningStrategy, ValidatorsConfiguration, ValidatorsSource,
};
use sp_bridge_eth_poa::{Address, Header, U256};
use sp_std::prelude::*;

frame_support::parameter_types! {
	pub const FinalityVotesCachingInterval: Option<u64> = Some(8);
	pub BridgeAuraConfiguration: AuraConfiguration =
		aura_configuration();
	pub BridgeValidatorsConfiguration: ValidatorsConfiguration =
		validators_configuration();
}

/// Max number of finalized headers to keep.
const FINALIZED_HEADERS_TO_KEEP: u64 = 5_000;

/// Aura engine configuration for TestPoa chain.
pub fn aura_configuration() -> AuraConfiguration {
	AuraConfiguration {
		empty_steps_transition: 0,
		strict_empty_steps_transition: 0,
		validate_step_transition: 0,
		validate_score_transition: 0,
		two_thirds_majority_transition: u64::max_value(),
		min_gas_limit: 0x1388.into(),
		max_gas_limit: U256::max_value(),
		maximum_extra_data_size: 0x20,
	}
}

/// Validators configuration for TestPoa chain.
pub fn validators_configuration() -> ValidatorsConfiguration {
	ValidatorsConfiguration::Single(ValidatorsSource::List(genesis_validators()))
}

/// Genesis validators set of TestPoa chain.
pub fn genesis_validators() -> Vec<Address> {
	vec![
		hex!("005e714f896a8b7cede9d38688c1a81de72a58e4").into(),
		hex!("007594304039c2937a12220338aab821d819f5a4").into(),
		hex!("004e7a39907f090e19b0b80a277e77b72b22e269").into(),
	]
}

/// Genesis header of the TestPoa chain.
///
/// To obtain genesis header from a running node, invoke:
/// ```bash
/// TODO
/// ```
pub fn genesis_header() -> Header {
	Header {
		parent_hash: Default::default(),
		timestamp: 0,
		number: 0,
		author: Default::default(),
		transactions_root: hex!("56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421").into(),
		uncles_hash: hex!("1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347").into(),
		extra_data: vec![],
		state_root: hex!("2480155b48a1cea17d67dbfdfaafe821c1d19cdd478c5358e8ec56dec24502b2").into(),
		receipts_root: hex!("56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421").into(),
		log_bloom: Default::default(),
		gas_used: Default::default(),
		gas_limit: 6000000.into(),
		difficulty: 131072.into(),
		seal: vec![
			vec![128].into(),
			vec![
				184, 65, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
				0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
			]
			.into(),
		],
	}
}

/// TestPoa headers pruning strategy.
///
/// We do not prune unfinalized headers because exchange module only accepts
/// claims from finalized headers. And if we're pruning unfinalized headers, then
/// some claims may never be accepted.
#[derive(Default, RuntimeDebug)]
pub struct PruningStrategy;

impl TPruningStrategy for PruningStrategy {
	fn pruning_upper_bound(&mut self, _best_number: u64, best_finalized_number: u64) -> u64 {
		best_finalized_number
			.checked_sub(FINALIZED_HEADERS_TO_KEEP)
			.unwrap_or(0)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn pruning_strategy_keeps_enough_headers() {
		assert_eq!(
			PruningStrategy::default().pruning_upper_bound(100_000, 10_000),
			0,
			"10_000 <= 20_000 => nothing should be pruned yet",
		);

		assert_eq!(
			PruningStrategy::default().pruning_upper_bound(100_000, 20_000),
			0,
			"20_000 <= 20_000 => nothing should be pruned yet",
		);

		assert_eq!(
			PruningStrategy::default().pruning_upper_bound(100_000, 30_000),
			5_000,
			"20_000 <= 30_000 => we're ready to prune first 5_000 headers",
		);
	}
}
