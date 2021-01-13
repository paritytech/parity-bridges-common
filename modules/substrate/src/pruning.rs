// Copyright 2021 Parity Technologies (UK) Ltd.
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

//! Old headers pruning utilities.

use crate::BridgeStorage;
use frame_support::ensure;
use sp_runtime::traits::{self, Saturating};
use sp_std::vec::Vec;

pub enum PruneError {
	/// The requested header is not finalized yet.
	NotFinalized,
	/// The requested header is too recent, we can't remove it.
	TooRecent,
	/// Requested header is not found, we can't start pruning.
	Missing,
	/// We've reached the declared ancestry limit, but didn't reach
	/// a non-existent header.
	/// This means that the limit is not enough to perform consistent pruning.
	LimitReached,
}

/// Prune all ancestry of given header hash.
///
/// The function first verifies if it's fine to prune the ancestry, by checking that:
/// 1. the hash is already finalized
/// 2. the hash is old enough (i.e. there is a minimal number of headers that will still be kept;
///    `min_headers_to_keep`)
///
/// To make sure the function is bounded, we cannot look further than `ancestry_limit` blocks behind.
/// It is also incorrect to leave gaps - i.e. by traversing the ancestry, we must reach
/// a non-existent header before we reach the `ancestry_limit`.
///
/// Returns number of headers actually pruned.
pub fn prune_ancestry<S, H>(
	min_headers_to_keep: u32,
	mut storage: S,
	latest_hash: H::Hash,
	ancestry_limit: u32,
) -> Result<usize, PruneError> where
	H: traits::Header,
	S: BridgeStorage<Header = H>,
{
	// Make sure pruning is allowed: the header is old enough & finalized.
	let header = storage.header_by_hash(latest_hash).ok_or(PruneError::Missing)?;
	ensure!(header.is_finalized, PruneError::NotFinalized);

	let best_finalized_header = storage.best_finalized_header();
	let best_finalized_number = best_finalized_header.header.number();
	let is_old_enough = best_finalized_number.saturating_sub(*header.number())
		> min_headers_to_keep.into();
	ensure!(is_old_enough, PruneError::TooRecent);

	// traverse the ancestry and collect header hashes to remove.
	let mut to_prune = Vec::new();
	let mut current_hash = latest_hash;
	for _ in 0..ancestry_limit {
		if let Some(header) = storage.header_by_hash(current_hash) {
			to_prune.push(header.header.hash());
			current_hash = *header.header.parent_hash();
		} else {
			// seems that we've reached a non-existent header, which is fine.
			// we simply break the loop at this point and proceed with removal.
			current_hash = Default::default();
			break;
		}
	}

	// if the parent hash exists, means we've reached the limit and would leave a gap.
	ensure!(storage.header_exists(current_hash), PruneError::LimitReached);

	// perform the clean-up
	storage.remove_old_headers(&to_prune);

	Ok(to_prune.len())
}
