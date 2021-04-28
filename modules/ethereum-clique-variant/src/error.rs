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

use sp_runtime::RuntimeDebug;

/// Header import error.
#[derive(Clone, Copy, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(PartialEq))]
pub enum Error {
	/// The header is beyond last finalized and can not be imported.
	AncientHeader = 0,
	/// The header is already imported.
	KnownHeader = 1,
	/// Seal has an incorrect format.
	InvalidSealArity = 2,
	/// Block number isn't sensible.
	RidiculousNumber = 3,
	/// Block has too much gas used.
	TooMuchGasUsed = 4,
	/// Gas limit header field is invalid.
	InvalidGasLimit = 5,
	/// Extra data is of an invalid length.
	ExtraDataOutOfBounds = 6,
	/// Timestamp header overflowed.
	TimestampOverflow = 7,
	/// The parent header is missing from the blockchain.
	MissingParentBlock = 8,
	/// Validation proof insufficient.
	InsufficientProof = 13,
	/// Difficulty header field is invalid.
	InvalidDifficulty = 14,
	/// The received block is from an incorrect proposer.
	NotValidator = 15,
	/// Missing transaction receipts for the operation.
	MissingTransactionsReceipts = 16,
	/// Redundant transaction receipts are provided.
	RedundantTransactionsReceipts = 17,
	/// Provided transactions receipts are not matching the header.
	TransactionsReceiptsMismatch = 18,
	/// Can't accept unsigned header from the far future.
	UnsignedTooFarInTheFuture = 19,
	/// Trying to finalize sibling of finalized block.
	TryingToFinalizeSibling = 20,
	/// Header timestamp is ahead of on-chain timestamp
	HeaderTimestampIsAhead = 21,
	/// extra-data 32 byte vanity prefix missing
	/// MissingVanity is returned if a block's extra-data section is shorter than
	/// 32 bytes, which is required to store the validator(signer) vanity.
	MissingVanity = 22,
	/// extra-data 65 byte signature suffix missing
	/// MissingSignature is returned if a block's extra-data section doesn't seem
	/// to contain a 65 byte secp256k1 signature
	MissingSignature = 23,
	/// non-checkpoint block contains extra validator list
	/// ExtraValidators is returned if non-checkpoint block contain validator data in
	/// their extra-data fields
	ExtraValidators = 24,
	/// Invalid validator list on checkpoint block
	/// errInvalidCheckpointValidators is returned if a checkpoint block contains an
	/// invalid list of validators (i.e. non divisible by 20 bytes).
	InvalidCheckpointValidators = 25,
	/// Non-zero mix digest
	/// InvalidMixDigest is returned if a block's mix digest is non-zero.
	InvalidMixDigest = 26,
	/// Non empty uncle hash
	/// InvalidUncleHash is returned if a block contains an non-empty uncle list.
	InvalidUncleHash = 27,
	/// Non empty nonce
	/// InvalidNonce is returned if a block header nonce is non-empty
	InvalidNonce = 28,
	/// UnknownAncestor is returned when validating a block requires an ancestor that is unknown.
	UnknownAncestor = 29,
	/// Header timestamp too close
	/// HeaderTimestampTooClose is returned when header timestamp is too close with parent's
	HeaderTimestampTooClose = 30,
}

impl Error {
	pub fn msg(&self) -> &'static str {
		match *self {
			Error::AncientHeader => "Header is beyound last finalized and can not be imported",
			Error::KnownHeader => "Header is already imported",
			Error::InvalidSealArity => "Header has an incorrect seal",
			Error::RidiculousNumber => "Header has too large number",
			Error::TooMuchGasUsed => "Header has too much gas used",
			Error::InvalidGasLimit => "Header has invalid gas limit",
			Error::ExtraDataOutOfBounds => "Header has too large extra data",
			Error::TimestampOverflow => "Header has too large timestamp",
			Error::MissingParentBlock => "Header has unknown parent hash",
			Error::MissingStep => "Header is missing step seal",
			Error::MissingEmptySteps => "Header is missing empty steps seal",
			Error::DoubleVote => "Header has invalid step in seal",
			Error::InsufficientProof => "Header has insufficient proof",
			Error::InvalidDifficulty => "Header has invalid difficulty",
			Error::NotValidator => "Header is sealed by unexpected validator",
			Error::MissingTransactionsReceipts => "The import operation requires transactions receipts",
			Error::RedundantTransactionsReceipts => "Redundant transactions receipts are provided",
			Error::TransactionsReceiptsMismatch => "Invalid transactions receipts provided",
			Error::UnsignedTooFarInTheFuture => "The unsigned header is too far in future",
			Error::TryingToFinalizeSibling => "Trying to finalize sibling of finalized block",
			Error::HeaderTimestampIsAhead => "Header timestamp is ahead of on-chain timestamp",
			Error::MissingSignature => "Extra-data 65 byte signature suffix missing",
			_ => "TODO :)",
		}
	}

	/// Return unique error code.
	pub fn code(&self) -> u8 {
		*self as u8
	}
}
