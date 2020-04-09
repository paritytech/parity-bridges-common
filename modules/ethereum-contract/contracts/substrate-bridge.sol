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

// TODO: expose interface + switch to external+calldata after https://github.com/ethereum/solidity/issues/7929

// TODO: use ABIEncoderV2 to allow passing headers array as bytes[] and structs to constructor
// when ethabi will support it

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
		/// keccak256(header.hash) of the next header, or bytes32(0) if it is the best header.
		bytes32 nextHeaderKeccak;
		/// Header hash.
		bytes hash;
		/// Header number.
		bytes number;
	}

	/// Initializes bridge contract.
	/// @param rawInitialHeader Vec of single element - raw finalized header that will be ancestor of all importing headers.
	/// @param initialVotersSetId ID of GRANDPA voter set that must finalize direct children of the initial header.
	/// @param initialRawVoters Raw GRANDPA voter set that must finalize direct children of the initial header.
	constructor(
		bytes memory rawInitialHeader,
		uint64 initialVotersSetId,
		bytes memory initialRawVoters
	) public {
		// save initial header
		(
			Header memory initialHeader,
			VoterSetSignal memory voterSetSignal
		) = parseSubstrateHeader(
			0,
			rawInitialHeader
		);
		bytes32 headerKeccak = saveBestHeader(initialHeader);
		oldestHeaderKeccak = headerKeccak;
		// save best voter set
		bestVoterSet.id = initialVotersSetId;
		bestVoterSet.rawVoters = initialRawVoters;
	}

	/// Reject direct payments.
	fallback() external { revert(); }

	/// Returns hash of the best known header.
	function bestKnownHeader() public view returns (bytes memory, bytes memory) {
		Header storage bestHeader = headerByKeccak[bestHeaderKeccak];
		return (bestHeader.number, bestHeader.hash);
	}

	/// Returns true if header is known to the bridge.
	/// @param headerHash Hash of the header we want to check.
	function isKnownHeader(
		bytes memory headerHash
	) public view returns (bool) {
		return headerByKeccak[keccak256(headerHash)].hash.length != 0;
	}

	/// Import range of headers with finalization data.
	/// @param rawHeaders Vec of encoded finalized headers to import.
	/// @param rawFinalityProof Data required to finalize rawHeaders.
	function importHeaders(
		bytes memory rawHeaders,
		bytes memory rawFinalityProof
	) public {
		// verify finalization data
		(uint256 begin, uint256 end) = verifyFinalityProof(
			bestVoterSet.id,
			bestVoterSet.rawVoters,
			headerByKeccak[bestHeaderKeccak].hash,
			rawHeaders,
			rawFinalityProof
		);

		// save finalized headers
		bool enactedNewSet = false;
		for (uint256 i = begin; i < end; ++i) {
			// parse header
			(
				Header memory header,
				VoterSetSignal memory voterSetSignal
			) = parseSubstrateHeader(
				i,
				rawHeaders
			);

			// save header to the storage
			bytes32 headerNumberKeccak = keccak256(header.number);
			saveBestHeader(header);
			// save voters set signal (if signalled by the header)
			if (voterSetSignal.rawVoters.length != 0) {
				saveSignal(voterSetSignal);
			}
			// check if header enacts new set
			bytes memory newRawVoters = voterSetByEnactNumber[headerNumberKeccak];
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
	/// @return keccak256(header.hash)
	function saveBestHeader(
		Header memory header
	) private returns (bytes32) {
		bytes32 headerKeccak = keccak256(header.hash);
		bytes32 previousHeaderKeccak = bestHeaderKeccak;
		if (previousHeaderKeccak != bytes32(0)) {
			headerByKeccak[previousHeaderKeccak].nextHeaderKeccak = headerKeccak;
		}
		headerByKeccak[headerKeccak] = header;
		bestHeaderKeccak = headerKeccak;
		storedHeadersCount = storedHeadersCount + 1;
		return headerKeccak;
	}

	/// Prune oldest header.
	function pruneOldestHeader() private {
		bytes32 headerKeccakToRemove = oldestHeaderKeccak;
		Header storage oldestHeader = headerByKeccak[headerKeccakToRemove];
		oldestHeaderKeccak = oldestHeader.nextHeaderKeccak;
		delete headerByKeccak[headerKeccakToRemove];
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

	/// Parse i-th Substrate header from the raw headers vector.
	/// @return header.hash, header.number and optional voter set signal.
	function parseSubstrateHeader(
		uint256 headerIndex,
		bytes memory rawHeaders
	) private pure returns (Header memory, VoterSetSignal memory) {
		return (
			Header({
				nextHeaderKeccak: bytes32(0),
				hash: abi.encodePacked(keccak256(abi.encode(headerIndex, rawHeaders))),
				number: abi.encodePacked(headerIndex)
			}),
			VoterSetSignal({
				headerNumber: bytes32(0),
				rawVoters: ""
			})
		); // TODO: replace with builtin call
	}

	/// Verify finality proof.
	/// @return Range of headers within rawHeaders that are proved to be final.
	function verifyFinalityProof(
		uint64 /*currentSetId*/,
		bytes memory /*rawCurrentVoters*/,
		bytes memory /*rawBestHeader*/,
		bytes memory /*rawHeaders*/,
		bytes memory /*rawFinalityProof*/
	) private pure returns (uint256, uint256) {
		return (0, 0); // TODO: replace with builtin call
	}

	/// Maximal number of headers that we store.
	uint256 constant maxHeadersToStore = 1024;

	/// Current number of headers that we store.
	uint256 storedHeadersCount;
	/// keccak256(header.hash) of the oldest header.
	bytes32 oldestHeaderKeccak;
	/// keccak256(header.hash) of the last finalized block.
	bytes32 bestHeaderKeccak;
	/// Raw voter set for the last finalized block.
	VoterSet bestVoterSet;
	/// Map of keccak256(header.hash) => header.
	mapping (bytes32 => Header) headerByKeccak;
	/// Map of keccak256(header.number) => raw voter set that is enacted when block
	/// with given number is finalized.
	mapping (bytes32 => bytes) voterSetByEnactNumber;
}
