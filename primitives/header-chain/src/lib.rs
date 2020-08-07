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

//! Defines traits which represent a common interface for Substrate pallets which want to
//! incorporate bridge functinality.

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::dispatch::DispatchResult;

/// A base trait for pallets which want to keep track of a full set of headers from a bridged chain.
pub trait MinimalHeaderChain<AccountId> {
	/// The header used by the chain.
	type Header;
	/// Any extra data which helps describe a transaction.
	type Extra;
	/// The type of block number used by the chain.
	type BlockNumber;
	/// The type of block hash used by the chain.
	type BlockHash;

	/// Imports a header submitted using an _unsigned_ transaction to the pallet.
	fn import_header_unsigned(header: Self::Header, extra_data: Option<Self::Extra>) -> DispatchResult;

	/// Imports a header submitted using a _signed_ transaction to the pallet.
	fn import_header_signed(
		submitter: AccountId,
		header: Self::Header,
		extra_data: Option<Self::Extra>,
	) -> DispatchResult;

	/// Get the best finalized block the pallet knows of.
	fn best_finalized_header() -> Self::Header;

	/// Get a specific block from the pallet given its hash.
	///
	/// Will return None if this block is not part of the canonical chain tracked by the pallet.
	fn header_by_hash(block_hash: Self::BlockHash) -> Option<Self::Header>;
}
