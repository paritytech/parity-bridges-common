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

/// A trait which defines a common interface for Substrate pallets which want
/// to incorporate bridge functionality.
pub trait HeaderChain {
	/// The header used by the chain.
	type Header;
	/// Any extra data which helps describe a transaction.
	type Extra;
	/// The type of block number used by the chain.
	type BlockNumber;
	/// The type of block hash used by the chain.
	type BlockHash;
	/// The type of transaction proof used by the chain.
	type Proof;

	/// Imports a header submitted using an unsigned transaction to the pallet.
	fn import_header_unsigned(header: Self::Header, extra_data: Option<Self::Extra>);

	/// Imports a header submitted using an signed transaction to the pallet.
	fn import_header_signed(header: Self::Header, extra_dat: Option<Self::Extra>);

	/// Get the best block the pallet knows of.
	fn best_block() -> Self::Header;

	/// Get the best finalized block the pallet knows of.
	fn finalized_block() -> Self::Header;

	/// Get the earliest block the pallet knows of.
	fn earliest_block() -> Self::Header;

	/// Get a specific block from the pallet given its number.
	fn by_number(block_number: Self::BlockNumber) -> Self::Header;

	/// Get a specific block from the pallet given its hash.
	fn by_hash(block_hash: Self::BlockHash) -> Self::Header;

	/// Verfy a transaction proof.
	fn verify_proof(proof: Self::Proof) -> bool;
}
