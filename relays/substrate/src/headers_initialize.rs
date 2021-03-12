// Copyright 2019-2020 Parity Technologies (UK) Ltd.
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

//! Initialize Substrate -> Substrate headers bridge.
//!
//! Initialization is a transaction that calls `initialize()` function of the
//! `pallet-finality-verifier` pallet. This transaction brings initial header
//! and authorities set from source to target chain. The headers sync starts
//! with this header.

use codec::Decode;
use pallet_finality_verifier::InitializationData;
use relay_substrate_client::{Chain, Client};
use sp_core::Bytes;
use sp_finality_grandpa::{AuthorityList as GrandpaAuthoritiesSet, SetId as GrandpaAuthoritiesSetId};
use sp_runtime::traits::Header as HeaderT;

/// Submit headers-bridge initialization transaction.
pub async fn initialize<SourceChain: Chain, TargetChain: Chain>(
	source_client: Client<SourceChain>,
	target_client: Client<TargetChain>,
	raw_initial_header: Option<Bytes>,
	raw_initial_authorities_set: Option<Bytes>,
	initial_authorities_set_id: Option<GrandpaAuthoritiesSetId>,
	authorities_set_id_key: Vec<u8>,
	prepare_initialize_transaction: impl FnOnce(InitializationData<SourceChain::Header>) -> Result<Bytes, String>,
) {
	let result = do_initialize(
		source_client,
		target_client,
		raw_initial_header,
		raw_initial_authorities_set,
		initial_authorities_set_id,
		authorities_set_id_key,
		prepare_initialize_transaction,
	)
	.await;

	match result {
		Ok(tx_hash) => log::info!(
			target: "bridge",
			"Successfully submitted {}-headers bridge initialization transaction to {}: {:?}",
			SourceChain::NAME,
			TargetChain::NAME,
			tx_hash,
		),
		Err(err) => log::error!(
			target: "bridge",
			"Failed to submit {}-headers bridge initialization transaction to {}: {:?}",
			SourceChain::NAME,
			TargetChain::NAME,
			err,
		),
	}
}

/// Craft and submit initialization transaction, returning any error that may occur.
async fn do_initialize<SourceChain: Chain, TargetChain: Chain>(
	source_client: Client<SourceChain>,
	target_client: Client<TargetChain>,
	raw_initial_header: Option<Bytes>,
	raw_initial_authorities_set: Option<Bytes>,
	initial_authorities_set_id: Option<GrandpaAuthoritiesSetId>,
	authorities_set_id_key: Vec<u8>,
	prepare_initialize_transaction: impl FnOnce(InitializationData<SourceChain::Header>) -> Result<Bytes, String>,
) -> Result<TargetChain::Hash, String> {
	let initialization_data = prepare_initialization_data(
		source_client,
		raw_initial_header,
		raw_initial_authorities_set,
		initial_authorities_set_id,
		authorities_set_id_key,
	)
	.await?;

	log::info!(
		target: "bridge",
		"Trying to initialize {}-headers bridge. Initialization data: {:?}",
		SourceChain::NAME,
		initialization_data,
	);

	let initialization_tx = prepare_initialize_transaction(initialization_data)?;
	let initialization_tx_hash = target_client
		.submit_extrinsic(initialization_tx)
		.await
		.map_err(|err| format!("Failed to submit {} transaction: {:?}", TargetChain::NAME, err))?;
	Ok(initialization_tx_hash)
}

/// Prepare initialization data for the headers-bridge pallet.
async fn prepare_initialization_data<SourceChain: Chain>(
	source_client: Client<SourceChain>,
	raw_initial_header: Option<Bytes>,
	raw_initial_authorities_set: Option<Bytes>,
	initial_authorities_set_id: Option<GrandpaAuthoritiesSetId>,
	authorities_set_id_key: Vec<u8>,
) -> Result<InitializationData<SourceChain::Header>, String> {
	let initial_header = match raw_initial_header {
		Some(raw_initial_header) => SourceChain::Header::decode(&mut &raw_initial_header.0[..])
			.map_err(|err| format!("Failed to decode {} initial header: {:?}", SourceChain::NAME, err))?,
		None => {
			let best_finalized_header_hash = source_client.best_finalized_header_hash().await?;
			source_client
				.header_by_hash(best_finalized_header_hash)
				.await
				.map_err(|err| format!("Failed to retrive {} best finalized header: {:?}", SourceChain::NAME, err))?
		},
	};

	let initial_header_hash = initial_header.hash();
	let raw_initial_authorities_set = match raw_initial_authorities_set {
		Some(raw_initial_authorities_set) => raw_initial_authorities_set.0,
		None => source_client
			.grandpa_authorities_set(initial_header_hash)
			.await
			.map_err(|err| {
				format!(
					"Failed to retrive {} authorities set at genesis header: {:?}",
					SourceChain::NAME,
					err
				)
			})?,
	};
	let initial_authorities_set =
		GrandpaAuthoritiesSet::decode(&mut &raw_initial_authorities_set[..]).map_err(|err| {
			format!(
				"Failed to decode {} initial authorities set: {:?}",
				SourceChain::NAME,
				err
			)
		})?;

	let initial_authorities_set_id = match initial_authorities_set_id {
		Some(initial_authorities_set_id) => initial_authorities_set_id,
		None => source_client
			.storage(initial_header_hash, authorities_set_id_key)
			.await
			.map_err(|err| format!("Failed to read GRANDPA authorities set id from {}: {:?}", SourceChain::NAME, err))
			.and_then(|set_id| set_id.ok_or_else(||
				format!("GRANDPA authorities set id on chain {} is unknown", SourceChain::NAME)
			))
			.and_then(|set_id| Decode::decode(&mut &set_id[..])
				.map_err(|err| format!("Failed to decode GARNDPA authorities set id from {}: {:?}", SourceChain::NAME, err))
			)?,
	};

	Ok(InitializationData {
		header: initial_header,
		authority_list: initial_authorities_set,
		set_id: initial_authorities_set_id,
		is_halted: false,
	})
}
