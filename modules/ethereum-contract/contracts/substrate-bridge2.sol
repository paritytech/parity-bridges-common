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

pragma solidity ^0.6.4;

// for simplicity, this contract works with 32-bit headers hashes and headers
// numbers that can be represented as uint256 (supporting uint256 arithmetics)

/// @title Substrate-to-PoA Bridge Contract.
contract SubstrateBridge {
	/// Parsed header.
	struct ParsedHeader {
		/// Header hash.
		bytes32 hash;
		/// Parent header hash.
		bytes32 parentHash;
		/// Header number.
		uint256 number;
		/// Validators set change signal delay.
		uint256 signalDelay;
		/// Validators set change signal.
		bytes signal;
	}

	/// Header as it is stored in the storage.
	struct Header {
		/// Flag to ensure that the header exists :/
		bool isKnown;

		/// Parent header hash.
		bytes32 parentHash;
		/// Header number.
		uint256 number;

		/// Validators set change signal.
		bytes signal;

		/// ID of validators set that must finalize this header. This equals to same
		/// field of the parent + 1 if parent header should enact new set.
		uint64 validatorsSetId;
		/// Hash of the latest header of this fork that has emitted last validators set
		/// change signal.
		bytes32 prevSignalHeaderHash;
		/// Number of the header where latest signal of this fork must be enacted.
		uint256 prevSignalTargetNumber;
	}

	/// Initializes bridge contract.
	/// @param rawInitialHeader Raw finalized header that will be ancestor of all importing headers.
	/// @param initialValidatorsSetId ID of validators set that must finalize direct children of the initial header.
	/// @param initialValidatorsSet Raw validators set that must finalize direct children of the initial header.
	constructor(
		bytes memory rawInitialHeader,
		uint64 initialValidatorsSetId,
		bytes memory initialValidatorsSet
	) public {
		// parse and save header
		ParsedHeader memory header = parseSubstrateHeader(rawInitialHeader);
		lastImportedHeaderHash = header.hash;
		bestFinalizedHeaderHash = header.hash;
		bestFinalizedHeaderNumber = header.number;
		headerByHash[header.hash] = Header({
			isKnown: true,
			parentHash: header.parentHash,
			number: header.number,
			signal: header.signal,
			validatorsSetId: initialValidatorsSetId,
			prevSignalHeaderHash: bytes32(0),
			prevSignalTargetNumber: 0
		});

		// save best validators set
		bestFinalizedValidatorsSetId = initialValidatorsSetId;
		bestFinalizedValidatorsSet = initialValidatorsSet;
	}

	/// Reject direct payments.
	fallback() external { revert(); }

	/// Returns number and hash of the best known header. Best known header is
	/// the last header we have received, no matter hash or number. We can't
	/// verify unfinalized header => we are only signalling relay that we are
	/// receiving new headers here, so honest relay can continue to submit valid
	/// headers and, eventually, finality proofs.
	function bestKnownHeader() public view returns (uint256, bytes32) {
		Header storage lastImportedHeader = headerByHash[lastImportedHeaderHash];
		return (lastImportedHeader.number, lastImportedHeaderHash);
	}

	/// Returns true if header is known to the bridge.
	/// @param headerHash Hash of the header we want to check.
	function isKnownHeader(
		bytes32 headerHash
	) public view returns (bool) {
		return headerByHash[headerHash].isKnown;
	}

	/// Returns true if finality proof is required for this header.
	function isFinalityProofRequired(
		bytes32 headerHash
	) public view returns (bool) {
		Header storage header = headerByHash[headerHash];
		return header.isKnown
			&& header.number > bestFinalizedHeaderNumber
			&& header.number == header.prevSignalTargetNumber
			&& header.validatorsSetId == bestFinalizedValidatorsSetId;
	}

	/// Import header.
	function importHeader(
		bytes memory rawHeader
	) public {
		// parse and save header
		ParsedHeader memory header = parseSubstrateHeader(rawHeader);
		Header storage parentHeader = headerByHash[header.parentHash];
		require(
			parentHeader.number == header.number - 1,
			"Missing parent header from the storage"
		);

		// forbid appending to fork until we'll get finality proof for header that
		// requires it
		if (parentHeader.prevSignalTargetNumber == parentHeader.number) {
			require(
				bestFinalizedHeaderHash == header.parentHash,
				"Missing required finality proof for parent header"
			);
		}

		// forbid overlapping signals
		uint256 prevSignalTargetNumber = parentHeader.prevSignalTargetNumber;
		if (header.signal.length != 0) {
			require(
				prevSignalTargetNumber < header.number,
				"Overlapping signals found"
			);
			prevSignalTargetNumber = header.number + header.signalDelay;
		}

		// check if parent header has emitted validators set change signal
		uint64 validatorsSetId = parentHeader.validatorsSetId;
		bytes32 prevSignalHeaderHash = parentHeader.prevSignalHeaderHash;
		if (parentHeader.signal.length != 0) {
			prevSignalHeaderHash = header.parentHash;
			validatorsSetId = validatorsSetId + 1;
		}

		// store header in the storage
		headerByHash[header.hash] = Header({
			isKnown: true,
			parentHash: header.parentHash,
			number: header.number,
			signal: header.signal,
			validatorsSetId: validatorsSetId,
			prevSignalHeaderHash: prevSignalHeaderHash,
			prevSignalTargetNumber: prevSignalTargetNumber
		});
		lastImportedHeaderHash = header.hash;
	}

	/// Import finality proof.
	function importFinalityProof(
		uint256 finalityTargetNumber,
		bytes32 finalityTargetHash,
		bytes memory rawFinalityProof
	) public {
		// check that header that we're going to finalize is already imported
		require(
			headerByHash[finalityTargetHash].number == finalityTargetNumber,
			"Missing finality target header from the storage"
		);

		// verify finality proof
		bytes32 oldBestFinalizedHeaderHash = bestFinalizedHeaderHash;
		bytes32 newBestFinalizedHeaderHash = verifyFinalityProof(
			finalityTargetNumber,
			finalityTargetHash,
			rawFinalityProof
		);

		// remember new best finalized header
		Header storage newFinalizedHeader = headerByHash[newBestFinalizedHeaderHash];
		bestFinalizedHeaderHash = newBestFinalizedHeaderHash;
		bestFinalizedHeaderNumber = newFinalizedHeader.number;

		// apply validators set change signal if required
		while (newBestFinalizedHeaderHash != oldBestFinalizedHeaderHash) {
			newFinalizedHeader = headerByHash[newBestFinalizedHeaderHash];
			newBestFinalizedHeaderHash = newFinalizedHeader.parentHash;
			// if we are finalizing header that should enact validators set change, do this
			// (this only affects latest scheduled change)
			if (newFinalizedHeader.number == newFinalizedHeader.prevSignalTargetNumber) {
				Header storage signalHeader = headerByHash[newFinalizedHeader.prevSignalHeaderHash];
				bestFinalizedValidatorsSetId += 1;
				bestFinalizedValidatorsSet = signalHeader.signal;
				break;
			}
		}
	}

	/// Parse Substrate header.
	function parseSubstrateHeader(
		bytes memory rawHeader
	) private view returns (ParsedHeader memory) {
		bytes32 headerHash;
		bytes32 headerParentHash;
		uint256 headerNumber;
		uint256 headerSignalDelay;
		uint256 headerSignalSize;
		bytes memory headerSignal;

		assembly {
			// inputs
			let rawHeadersSize := mload(rawHeader)
			let rawHeadersPointer := add(rawHeader, 0x20)

			// output
			let headerHashPointer := mload(0x40)
			let headerParentHashPointer := add(headerHashPointer, 0x20)
			let headerNumberPointer := add(headerParentHashPointer, 0x20)
			let headerSignalDelayPointer := add(headerNumberPointer, 0x20)
			let headerSignalSizePointer := add(headerSignalDelayPointer, 0x20)

			// parse substrate header
			if iszero(staticcall(
				not(0),
				SUBSTRATE_PARSE_HEADER_BUILTIN_ADDRESS,
				rawHeadersPointer,
				rawHeadersSize,
				headerHashPointer,
				0xA0
			)) {
				revert(0, 0)
			}

			// fill basic header fields
			headerHash := mload(headerHashPointer)
			headerParentHash := mload(headerParentHashPointer)
			headerNumber := mload(headerNumberPointer)
			headerSignalDelay := mload(headerSignalDelayPointer)
			headerSignalSize := mload(headerSignalSizePointer)
		}

		// if validators set change is signalled, read it
		if (headerSignalSize != 0) {
			headerSignal = new bytes(headerSignalSize);

			assembly {
				// inputs
				let rawHeadersSize := mload(rawHeader)
				let rawHeadersPointer := add(rawHeader, 0x20)

				// output
				let headerSignalPointer := add(headerSignal, 0x20)

				// get substrate header valdiators set change signal
				if iszero(staticcall(
					not(0),
					SUBSTRATE_GET_HEADER_SIGNAL_BUILTIN_ADDRESS,
					rawHeadersPointer,
					rawHeadersSize,
					headerSignalPointer,
					headerSignalSize
				)) {
					revert(0, 0)
				}
			}
		}

		return ParsedHeader({
			hash: headerHash,
			parentHash: headerParentHash,
			number: headerNumber,
			signalDelay: headerSignalDelay,
			signal: headerSignal
		});
	}


	/// Verify finality proof.
	function verifyFinalityProof(
		uint256 /*finalityTargetNumber*/,
		bytes32 /*finalityTargetHash*/,
		bytes memory /*rawFinalityProof*/
	) private view returns (bytes32) {
		return bestFinalizedHeaderHash; // TODO: call builtin instead
	}

	/// Address of parse_substrate_header builtin.
	uint256 constant SUBSTRATE_PARSE_HEADER_BUILTIN_ADDRESS = 0x10;
	/// Address of get_substrate_validators_set_signal builtin.
	uint256 constant SUBSTRATE_GET_HEADER_SIGNAL_BUILTIN_ADDRESS = 0x11;

	/// Last imported header hash.
	bytes32 lastImportedHeaderHash;

	/// Best finalized header number.
	uint256 bestFinalizedHeaderNumber;
	/// Best finalized header hash.
	bytes32 bestFinalizedHeaderHash;
	/// Best finalized validators set id.
	uint64 bestFinalizedValidatorsSetId;
	/// Best finalized validators set.
	bytes bestFinalizedValidatorsSet;

	/// Map of headers by their hashes.
	mapping (bytes32 => Header) headerByHash;
}
