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

//! Additional logic for working with Substrate storage proofs.

use sp_std::vec::Vec;

/// Storage proof size requirements.
///
/// This is currently used by benchmarks when generating storage proofs.
#[derive(Clone, Copy, Debug)]
pub enum ProofSize {
	/// The proof is expected to be minimal. If value size may be changed, then it is expected to
	/// have given size.
	Minimal(u32),
	/// The proof is expected to have at least given size and grow by increasing value that is
	/// stored in the trie.
	HasLargeLeaf(u32),
}

/// Add extra data to the trie leaf value so that it'll be of given size.
pub fn grow_trie_leaf_value(mut value: Vec<u8>, size: ProofSize) -> Vec<u8> {
	match size {
		ProofSize::Minimal(_) => (),
		ProofSize::HasLargeLeaf(size) if size as usize > value.len() => {
			value.extend(sp_std::iter::repeat(42u8).take(size as usize - value.len()));
		},
		ProofSize::HasLargeLeaf(_) => (),
	}
	value
}
