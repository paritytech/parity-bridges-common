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

//! The verifier's role is to check the validity of headers being imported, and also determine if
//! they can be finalized.
//!
//! When importing headers it performs checks to ensure that no invariants are broken (like
//! importing the same header twice). When it imports finality proofs it will ensure that the proof
//! has been signed off by the correct GRANDPA authorities, and also enact any authority set changes
//! if required.

use crate::storage::{ImportedHeader, ScheduledChange};
use crate::BridgeStorage;

use bp_header_chain::{justification::verify_justification, AuthoritySet};
use finality_grandpa::voter_set::VoterSet;
use sp_finality_grandpa::{ConsensusLog, GRANDPA_ENGINE_ID};
use sp_runtime::generic::OpaqueDigestItemId;
use sp_runtime::traits::{CheckedAdd, Header as HeaderT, One};
use sp_runtime::RuntimeDebug;
use sp_std::{prelude::Vec, vec};

/// The finality proof used by the pallet.
///
/// For a Substrate based chain using GRANDPA this will
/// be an encoded GRANDPA Justification.
#[derive(RuntimeDebug)]
pub struct FinalityProof(Vec<u8>);

impl From<&[u8]> for FinalityProof {
	fn from(proof: &[u8]) -> Self {
		Self(proof.to_vec())
	}
}

impl From<Vec<u8>> for FinalityProof {
	fn from(proof: Vec<u8>) -> Self {
		Self(proof)
	}
}

pub(crate) fn find_scheduled_change<H: HeaderT>(header: &H) -> Option<sp_finality_grandpa::ScheduledChange<H::Number>> {
	let id = OpaqueDigestItemId::Consensus(&GRANDPA_ENGINE_ID);

	let filter_log = |log: ConsensusLog<H::Number>| match log {
		ConsensusLog::ScheduledChange(change) => Some(change),
		_ => None,
	};

	// find the first consensus digest with the right ID which converts to
	// the right kind of consensus log.
	header.digest().convert_first(|l| l.try_to(id).and_then(filter_log))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::*;
	use crate::{BestFinalized, BestHeight, HeaderId, ImportedHeaders, PalletStorage};
	use bp_test_utils::{alice, authority_list, bob, make_justification_for_header};
	use codec::Encode;
	use frame_support::{assert_err, assert_ok};
	use frame_support::{StorageMap, StorageValue};
	use sp_finality_grandpa::{AuthorityId, SetId};
	use sp_runtime::{Digest, DigestItem};

	fn schedule_next_change(
		authorities: Vec<AuthorityId>,
		set_id: SetId,
		height: TestNumber,
	) -> ScheduledChange<TestNumber> {
		let authorities = authorities.into_iter().map(|id| (id, 1u64)).collect();
		let authority_set = AuthoritySet::new(authorities, set_id);
		ScheduledChange { authority_set, height }
	}

	// Useful for quickly writing a chain of headers to storage
	// Input is expected in the form: vec![(num, requires_justification, is_finalized)]
	fn write_headers<S: BridgeStorage<Header = TestHeader>>(
		storage: &mut S,
		headers: Vec<(u64, bool, bool)>,
	) -> Vec<ImportedHeader<TestHeader>> {
		let mut imported_headers = vec![];
		let genesis = ImportedHeader {
			header: test_header(0),
			requires_justification: false,
			is_finalized: true,
			signal_hash: None,
		};

		<BestFinalized<TestRuntime>>::put(genesis.hash());
		storage.write_header(&genesis);
		imported_headers.push(genesis);

		for (num, requires_justification, is_finalized) in headers {
			let header = ImportedHeader {
				header: test_header(num),
				requires_justification,
				is_finalized,
				signal_hash: None,
			};

			storage.write_header(&header);
			imported_headers.push(header);
		}

		imported_headers
	}

	// Given a block number will generate a chain of headers which don't require justification and
	// are not considered to be finalized.
	fn write_default_headers<S: BridgeStorage<Header = TestHeader>>(
		storage: &mut S,
		headers: Vec<u64>,
	) -> Vec<ImportedHeader<TestHeader>> {
		let headers = headers.iter().map(|num| (*num, false, false)).collect();
		write_headers(storage, headers)
	}
}
