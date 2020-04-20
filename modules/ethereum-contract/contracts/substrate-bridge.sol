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

	/// Returns numbers of headers that require finality proofs.
	function incompleteHeaders() public view returns (uint256[] memory, bytes32[] memory) {
		uint256 incompleteHeadersCount = incompleteHeadersHashes.length;
		uint256[] memory incompleteHeadersNumbers = new uint256[](incompleteHeadersCount);
		for (uint256 i = 0; i < incompleteHeadersCount; ++i) {
			incompleteHeadersNumbers[i] = headerByHash[incompleteHeadersHashes[i]].number;
		}
		return (incompleteHeadersNumbers, incompleteHeadersHashes);
	}

	/// Import header.
	function importHeader(
		bytes memory rawHeader
	) public {
		// parse and save header
		ParsedHeader memory header = parseSubstrateHeader(rawHeader);
		Header storage parentHeader = headerByHash[header.parentHash];
		require(
			parentHeader.isKnown && parentHeader.number == header.number - 1,
			"Missing parent header from the storage"
		);

		// forbid appending to fork until we'll get finality proof for header that
		// requires it
		if (parentHeader.prevSignalTargetNumber != 0 && parentHeader.prevSignalTargetNumber == parentHeader.number) {
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

		// remember if we need finality proof for this header
		if (prevSignalTargetNumber == header.number) {
			// TODO:
			//
			// In current implementation any submitter may submit any (invalid) block B that signals validators
			// set change in N blocks. This would require relay to ask Substrate node for finality proof and
			// submit it here, if it exists. So by spamming contract with signal headers, malicious submitter
			// can significantly slow down sync just because actual block that will need finality proof will
			// be waiting in the relay queue to be processed + incompleteHeadersNumbers may grow without any
			// limits.
			//
			// One solution would be to have reputation system - initially you may submit only one block-with-signal
			// (or you may submit it only once every N blocks). When your block-with-signal is finalized, your
			// reputation is increased and you may submit multiple blocks-with-signals (and they got priority in
			// the queue), but with every submitted block-with-signal, it decreases. This should cause honest
			// competition for block-with-signal 'slots' even if malicious submitters are present.
			uint256 incompleteHeaderHashIndex = incompleteHeadersHashes.length;
			incompleteHeadersHashes.push(header.hash);
			incompleteHeadersIndices[header.hash] = incompleteHeaderHashIndex + 1;
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
			bestFinalizedValidatorsSetId,
			bestFinalizedValidatorsSet,
			rawFinalityProof
		);

		// remember new best finalized header
		Header storage newFinalizedHeader = headerByHash[newBestFinalizedHeaderHash];
		bestFinalizedHeaderHash = newBestFinalizedHeaderHash;
		bestFinalizedHeaderNumber = newFinalizedHeader.number;

		// TODO: we may actually use prevSignalHeaderHash to find previous signal block instead of this while?

		// apply validators set change signal if required
		while (newBestFinalizedHeaderHash != oldBestFinalizedHeaderHash) {
			bytes32 finalizingHeader = newBestFinalizedHeaderHash;
			newFinalizedHeader = headerByHash[finalizingHeader];
			newBestFinalizedHeaderHash = newFinalizedHeader.parentHash;

			// swap_remove from incomplete headers, if required
			uint256 incompleteHeaderIndex = incompleteHeadersIndices[finalizingHeader];
			if (incompleteHeaderIndex != 0) {
				// shift by -1 to get actual array index
				incompleteHeaderIndex = incompleteHeaderIndex - 1;

				// if it isn't the last element, swap with last element
				uint256 incompleteHeadersCount = incompleteHeadersHashes.length;
				if (incompleteHeaderIndex != incompleteHeadersCount - 1) {
					bytes32 lastIncompleHeaderHash = incompleteHeadersHashes[incompleteHeadersCount - 1];
					incompleteHeadersHashes[incompleteHeaderIndex] = lastIncompleHeaderHash;
					incompleteHeadersIndices[lastIncompleHeaderHash] = incompleteHeaderIndex;
				}

				// remove last element from array and index from mapping
				incompleteHeadersHashes.pop();
				delete incompleteHeadersIndices[finalizingHeader];
			}

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

		assembly {
			// inputs
			let rawHeaderSize := mload(rawHeader)
			let rawHeaderPointer := add(rawHeader, 0x20)

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
				rawHeaderPointer,
				rawHeaderSize,
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
		bytes memory headerSignal = new bytes(headerSignalSize);
		if (headerSignalSize != 0) {
			assembly {
				// inputs
				let rawHeaderSize := mload(rawHeader)
				let rawHeaderPointer := add(rawHeader, 0x20)

				// output
				let headerSignalPointer := add(headerSignal, 0x20)

				// get substrate header valdiators set change signal
				if iszero(staticcall(
					not(0),
					SUBSTRATE_GET_HEADER_SIGNAL_BUILTIN_ADDRESS,
					rawHeaderPointer,
					rawHeaderSize,
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
	/// @return Hash of the new best finalized header.
	function verifyFinalityProof(
		uint256 finalityTargetNumber,
		bytes32 finalityTargetHash,
		uint64 bestSetId,
		bytes memory rawBestSet,
		bytes memory rawFinalityProof
	) private view returns (bytes32) {
		bytes memory encodedArgs = abi.encode(
			finalityTargetNumber,
			finalityTargetHash,
			bestSetId,
			rawBestSet,
			rawFinalityProof
		);

		assembly {
			// inputs
			let encodedArgsSize := mload(encodedArgs)
			let encodedArgsPointer := add(encodedArgs, 0x20)

			// verify finality proof
			if iszero(staticcall(
				not(0),
				SUBSTRATE_VERIFY_FINALITY_PROOF_BUILTIN_ADDRESS,
				encodedArgsPointer,
				encodedArgsSize,
				0x00,
				0x00
			)) {
				revert(0, 0)
			}
		}

		return finalityTargetHash;
	}

	/// Address of parse_substrate_header builtin.
	uint256 constant SUBSTRATE_PARSE_HEADER_BUILTIN_ADDRESS = 0x10;
	/// Address of get_substrate_validators_set_signal builtin.
	uint256 constant SUBSTRATE_GET_HEADER_SIGNAL_BUILTIN_ADDRESS = 0x11;
	/// Address of verify_substrate_finality_proof builtin.
	uint256 constant SUBSTRATE_VERIFY_FINALITY_PROOF_BUILTIN_ADDRESS = 0x12;

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

	/// Hashes of headers that require finality proof.
	bytes32[] incompleteHeadersHashes;
	/// Map of the incomplete header hash => index+1 within incompleteHeadersHashes.
	mapping (bytes32 => uint256) incompleteHeadersIndices;

	/// Map of headers by their hashes.
	mapping (bytes32 => Header) headerByHash;
}
