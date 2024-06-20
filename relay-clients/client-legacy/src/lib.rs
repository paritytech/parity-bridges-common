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

//! Types used to ensure compatibility with older versions of various types from `polkadot-sdk`.
//!
//! E.g. When a runtime is changed and uses different or newer types for the same struct, we can add
//! the older versions here and use them with `tools/runtime-codegen` for `TypeSubstitute`.

/// Types compatible with versions before the "compact proofs" feature.
pub mod non_compact_proofs {
	pub mod bridge_runtime_common {
		pub mod messages {
			use bp_messages::{LaneId, MessageNonce};
			use codec::{Decode, Encode};
			use scale_info::TypeInfo;
			use sp_runtime::RuntimeDebug;

			/// Raw storage proof type (just raw trie nodes).
			pub type RawStorageProof = Vec<Vec<u8>>;

			pub mod source {
				use super::*;

				/// Messages delivery proof from bridged chain:
				///
				/// - hash of finalized header;
				/// - storage proof of inbound lane state;
				/// - lane id.
				#[derive(Clone, Decode, Encode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
				pub struct FromBridgedChainMessagesDeliveryProof<BridgedHeaderHash> {
					/// Hash of the bridge header the proof is for.
					pub bridged_header_hash: BridgedHeaderHash,
					/// Storage trie proof generated for [`Self::bridged_header_hash`].
					pub storage_proof: RawStorageProof,
					/// Lane id of which messages were delivered and the proof is for.
					pub lane: LaneId,
				}
			}
			pub mod target {
				use super::*;

				/// Messages proof from bridged chain:
				///
				/// - hash of finalized header;
				/// - storage proof of messages and (optionally) outbound lane state;
				/// - lane id;
				/// - nonces (inclusive range) of messages which are included in this proof.
				#[derive(Clone, Decode, Encode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
				pub struct FromBridgedChainMessagesProof<BridgedHeaderHash> {
					/// Hash of the finalized bridged header the proof is for.
					pub bridged_header_hash: BridgedHeaderHash,
					/// A storage trie proof of messages being delivered.
					pub storage_proof: RawStorageProof,
					/// Messages in this proof are sent over this lane.
					pub lane: LaneId,
					/// Nonce of the first message being delivered.
					pub nonces_start: MessageNonce,
					/// Nonce of the last message being delivered.
					pub nonces_end: MessageNonce,
				}
			}
		}
	}
}
