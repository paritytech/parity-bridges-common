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

use frame_support::dispatch::DispatchResult;

/// A base trait for tracking finalized headers on foreign chains in a Substrate pallet. Should be
/// used as a primitive for more complicated header tracking logic.
pub trait FinalityHeaderChain<AccountId> {
	/// The header used by the chain.
	type Header;
	/// Any extra data which helps describe a transaction.
	type Extra;
	/// The type of transaction proof used by the chain.
	type Proof;

	/// Imports a finalized header submitted using an unsigned transaction to the pallet.
	fn import_finalized_header_unsigned(header: Self::Header, extra_data: Option<Self::Extra>);

	/// Imports a finalized header submitted using an signed transaction to the pallet.
	fn import_finalized_header_signed(submitter: AccountId, header: Self::Header, extra_data: Option<Self::Extra>);

	/// Get the best finalized block the pallet knows of.
	fn best_finalized_block() -> Self::Header;

	/// Get the earliest block the pallet knows of.
	fn earliest_finalized_block() -> Self::Header;

	/// Verify a proof of finality.
	fn verify_finality_proof(proof: Self::Proof) -> bool;
}

/// A trait for pallets which want to keep track of a full set of headers from a bridged chain.
pub trait FullHeaderChain<AccountId>: FinalityHeaderChain<AccountId> {
	/// The type of block number used by the chain.
	type BlockNumber;
	/// The type of block hash used by the chain.
	type BlockHash;

	/// Imports a header submitted using an unsigned transaction to the pallet.
	fn import_header_unsigned(header: Self::Header, extra_data: Option<Self::Extra>) -> DispatchResult;

	/// Imports a header submitted using an signed transaction to the pallet.
	fn import_header_signed(
		submitter: AccountId,
		header: Self::Header,
		extra_data: Option<Self::Extra>,
	) -> DispatchResult;

	/// Get the best block the pallet knows of.
	fn best_block() -> Self::Header;

	/// Get the earliest block the pallet knows of.
	fn earliest_block() -> Self::Header;

	/// Get a specific block from the pallet given its number.
	fn header_by_number(block_number: Self::BlockNumber) -> Self::Header;

	/// Get a specific block from the pallet given its hash.
	fn header_by_hash(block_hash: Self::BlockHash) -> Self::Header;
}
