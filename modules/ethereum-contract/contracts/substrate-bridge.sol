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
pragma experimental ABIEncoderV2;

/// @title Substrate-to-PoA Bridge Contract.
contract SubstrateBridge {
	/// Voter set as it is stored in the storage.
	struct VoterSet {
		/// Id of the set.
		uint64 id;
		/// Raw voter set.
		bytes rawVoters;
	}

	/// Voter set change signals.
	struct VoterSetSignal {
		/// keccak256(header.number) of the header that enacts this voter set.
		bytes32 headerNumber;
		/// Raw voter set.
		bytes rawVoters;
	}

	/// Header as it is stored in the storage.
	struct Header {
		/// keccak256(rawHeader) of the next header, or bytes32(0) if it is the best header.
		bytes32 nextHeaderHash;
		/// Raw header data.
		bytes rawHeader;
	}

	/// Initializes bridge contract.
	/// @param rawInitialHeader Raw finalized header that is ancestor of all importing headers.
	/// @param initialVoters GRANDPA voter set that must finalize direct children of the initial header.
	/// @param voterSetSignals GRANDPA voter set signals (including signal from rawInitialHeader).
	constructor(
		bytes memory rawInitialHeader,
		VoterSet memory initialVoters,
		VoterSetSignal[] memory voterSetSignals
	) public {
		// save initial header
		bytes32 headerHash = saveBestHeader(rawInitialHeader);
		oldestHeaderHash = headerHash;
		// save best voter set
		bestVoterSet.id = initialVoters.id;
		bestVoterSet.rawVoters = initialVoters.rawVoters;
		// save all signals
		for (uint i = 0; i < voterSetSignals.length; ++i) {
			saveSignal(voterSetSignals[i]);
		}
	}

	/// Reject direct payments.
	fallback() external { revert(); }

	/// Import range of headers with finalization data.
	/// @param rawHeaders Finalized headers to import.
	/// @param rawFinalityProof Data required to finalize rawHeaders.
	function importHeaders(
		bytes[] calldata rawHeaders,
		bytes calldata rawFinalityProof
	) external {
		// verify finalization data
		(uint256 begin, uint256 end) = verifyFinalityProof(
			bestHeaderHash,
			rawHeaders,
			rawFinalityProof
		);

		// save finalized headers
		bool enactedNewSet = false;
		for (uint256 i = begin; i < end; ++i) {
			// parse header
			bytes memory rawHeader = rawHeaders[i];
			(bytes32 headerNumber, VoterSetSignal memory voterSetSignal) = parseSubstrateHeader(
				rawHeader
			);

			// save header to the storage
			saveBestHeader(rawHeader);
			// save voters set signal (if signalled by the header)
			if (voterSetSignal.rawVoters.length != 0) {
				saveSignal(voterSetSignal);
			}
			// check if header enacts new set
			bytes memory newRawVoters = voterSetByEnactNumber[headerNumber];
			if (newRawVoters.length != 0) {
				require(
					!enactedNewSet,
					"Trying to enact several sets using single finality proof"
				);

				enactedNewSet = true;
				bestVoterSet.id = bestVoterSet.id + 1;
				bestVoterSet.rawVoters = newRawVoters;
			}
		}

		// prune oldest headers
		if (storedHeadersCount > maxHeadersToStore) {
			uint256 headersToPrune = storedHeadersCount - maxHeadersToStore;
			for (uint256 i = 0; i < headersToPrune; ++i) {
				pruneOldestHeader();
			}
		}
	}

	/// Save best header to the storage.
	/// @return keccak256(rawHeader)
	function saveBestHeader(
		bytes memory rawHeader
	) private returns (bytes32) {
		bytes32 headerHash = keccak256(rawHeader);
		bytes32 previousHeaderHash = bestHeaderHash;
		if (previousHeaderHash != bytes32(0)) {
			headerByHash[previousHeaderHash].nextHeaderHash = headerHash;
		}
		headerByHash[headerHash].rawHeader = rawHeader;
		bestHeaderHash = headerHash;
		storedHeadersCount = storedHeadersCount + 1;
		return headerHash;
	}

	/// Prune oldest header.
	function pruneOldestHeader() private {
		Header storage oldestHeader = headerByHash[oldestHeaderHash];
		oldestHeaderHash = oldestHeader.nextHeaderHash;
		delete headerByHash[oldestHeaderHash];
		storedHeadersCount = storedHeadersCount - 1;
	}

	/// Save voter set change signal to the storage.
	function saveSignal(
		VoterSetSignal memory voterSetSignal
	) private {
		require(
			voterSetByEnactNumber[voterSetSignal.headerNumber].length == 0,
			"Duplicate signal for the same block"
		);
		voterSetByEnactNumber[voterSetSignal.headerNumber] = voterSetSignal.rawVoters;
	}

	/// Parse Substrate header.
	/// @return keccak256(header.number) and voter set signal (if signalled by the header).
	function parseSubstrateHeader(
		bytes memory rawHeader
	) private pure returns (bytes32, VoterSetSignal memory) {
		return (
			bytes32(0),
			VoterSetSignal({
				headerNumber: bytes32(0),
				rawVoters: ""
			})
		); // TODO: replace with builtin call
	}

	/// Verify finality proof.
	/// @return Range of headers within rawHeaders that are proved to be final.
	function verifyFinalityProof(
		bytes32 bestHeaderHash,
		bytes[] memory rawHeaders,
		bytes memory rawFinalityProof
	) private pure returns (uint256, uint256) {
		return (0, 0); // TODO: replace with builtin call
	}

	/// Maximal number of headers that we store.
	uint256 constant maxHeadersToStore = 1024;

	/// Current number of headers that we store.
	uint256 storedHeadersCount;
	/// keccak32(rawHeader) of the oldest header.
	bytes32 oldestHeaderHash;
	/// keccak32(rawHeader) of the last finalized block.
	bytes32 bestHeaderHash;
	/// Raw voter set for the last finalized block.
	VoterSet bestVoterSet;
	/// Map of keccak256(raw header) => raw header.
	mapping (bytes32 => Header) headerByHash;
	/// Map of keccak256(header.number) => raw voter set that is enacted when block
	/// with given number is finalized.
	mapping (bytes32 => bytes) voterSetByEnactNumber;
}