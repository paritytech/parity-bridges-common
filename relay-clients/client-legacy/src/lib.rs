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

				impl<BridgedHeaderHash>
					From<
						bp_messages::source_chain::FromBridgedChainMessagesDeliveryProof<
							BridgedHeaderHash,
						>,
					> for FromBridgedChainMessagesDeliveryProof<BridgedHeaderHash>
				{
					fn from(
						value: bp_messages::source_chain::FromBridgedChainMessagesDeliveryProof<
							BridgedHeaderHash,
						>,
					) -> Self {
						FromBridgedChainMessagesDeliveryProof {
							bridged_header_hash: value.bridged_header_hash,
							// this is legacy change, we need to get `RawStorageProof` from
							// `UnverifiedStorageProof.proof`
							storage_proof: value.storage_proof.proof().clone(),
							lane: value.lane,
						}
					}
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

				impl<BridgedHeaderHash>
					From<
						bp_messages::target_chain::FromBridgedChainMessagesProof<BridgedHeaderHash>,
					> for FromBridgedChainMessagesProof<BridgedHeaderHash>
				{
					fn from(
						value: bp_messages::target_chain::FromBridgedChainMessagesProof<
							BridgedHeaderHash,
						>,
					) -> Self {
						FromBridgedChainMessagesProof {
							bridged_header_hash: value.bridged_header_hash,
							// this is legacy change, we need to get `RawStorageProof` from
							// `UnverifiedStorageProof.proof`
							storage_proof: value.storage.proof().clone(),
							lane: value.lane,
							nonces_start: value.nonces_start,
							nonces_end: value.nonces_end,
						}
					}
				}
			}
		}
	}

	/// Macro that generates `ReceiveMessagesProofCallBuilder` implementation for the case when
	/// you only have an access to the mocked version of target chain runtime. In this case you
	/// should provide "name" of the call variant for the bridge messages calls and the "name" of
	/// the variant for the `receive_messages_proof` call within that first option.
	#[rustfmt::skip]
	#[macro_export]
	macro_rules! generate_receive_message_proof_call_builder {
		($pipeline:ident, $mocked_builder:ident, $bridge_messages:path, $receive_messages_proof:path) => {
			pub struct $mocked_builder;

			impl substrate_relay_helper::messages::ReceiveMessagesProofCallBuilder<$pipeline>
				for $mocked_builder
			{
				fn build_receive_messages_proof_call(
					relayer_id_at_source: relay_substrate_client::AccountIdOf<
						<$pipeline as substrate_relay_helper::messages::SubstrateMessageLane>::SourceChain
					>,
					proof: substrate_relay_helper::messages::source::SubstrateMessagesProof<
						<$pipeline as substrate_relay_helper::messages::SubstrateMessageLane>::SourceChain
					>,
					messages_count: u32,
					dispatch_weight: bp_messages::Weight,
					_trace_call: bool,
				) -> relay_substrate_client::CallOf<
					<$pipeline as substrate_relay_helper::messages::SubstrateMessageLane>::TargetChain
				> {
					bp_runtime::paste::item! {
						$bridge_messages($receive_messages_proof {
							relayer_id_at_bridged_chain: relayer_id_at_source,
							// a legacy change - convert between `bp_messages::target_chain::FromBridgedChainMessagesDeliveryProof` and `FromBridgedChainMessagesDeliveryProof` - see `From` impl above
							proof: proof.1.into(),
							messages_count: messages_count,
							dispatch_weight: dispatch_weight,
						})
					}
				}
			}
		};
	}

	/// Macro that generates `ReceiveMessagesDeliveryProofCallBuilder` implementation for the case when
	/// you only have an access to the mocked version of source chain runtime. In this case you
	/// should provide "name" of the call variant for the bridge messages calls and the "name" of
	/// the variant for the `receive_messages_delivery_proof` call within that first option.
	#[rustfmt::skip]
	#[macro_export]
	macro_rules! generate_receive_message_delivery_proof_call_builder {
	($pipeline:ident, $mocked_builder:ident, $bridge_messages:path, $receive_messages_delivery_proof:path) => {
		pub struct $mocked_builder;

		impl substrate_relay_helper::messages::ReceiveMessagesDeliveryProofCallBuilder<$pipeline>
			for $mocked_builder
		{
			fn build_receive_messages_delivery_proof_call(
				proof: substrate_relay_helper::messages::target::SubstrateMessagesDeliveryProof<
					<$pipeline as substrate_relay_helper::messages::SubstrateMessageLane>::TargetChain
				>,
				_trace_call: bool,
			) -> relay_substrate_client::CallOf<
				<$pipeline as substrate_relay_helper::messages::SubstrateMessageLane>::SourceChain
			> {
				bp_runtime::paste::item! {
					$bridge_messages($receive_messages_delivery_proof {
						// a legacy change - convert between `bp_messages::source_chain::FromBridgedChainMessagesProof` and `FromBridgedChainMessagesProof` - see `From` impl above
						proof: proof.1.into(),
						relayers_state: proof.0
					})
				}
			}
		}
	};
}
}
