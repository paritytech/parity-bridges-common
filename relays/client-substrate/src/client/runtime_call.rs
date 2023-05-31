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

//! Relay-level runtime calls definition.
//!
//! There are many chains with their own `RuntimeCall` enum definitions. In relay we
//! want to construct this chain-level `RuntimeCall` right before sending transaction
//! to the node. At all other (upper) levels (including the `Client` trait) we want
//! to know contents of the call (for different reasons, including: proper logging,
//! updating test client state, deduplicating calls, ...). So we are using custom
//! generic structures, defined in this file.

// TODO: rename `runtime_call.rs` and `RuntimeCall` associated type to avoid confusions
// with actual `RuntimeCall` enum???!!!

use crate::{AccountIdOf, Chain, HeaderOf, SyncHeader};
use bp_header_chain::justification::GrandpaJustification;
use bp_polkadot_core::parachains::ParaHeadsProof;
use frame_support::{weights::Weight, DebugNoBound};

/// Intermediate message proof returned by the source Substrate node. Includes everything
/// required to submit to the target node: cumulative dispatch weight of bundled messages and
/// the proof itself.
pub type SubstrateMessagesProof<C> = (Weight, FromBridgedChainMessagesProof<HashOf<C>>);

/// Message receiving proof returned by the target Substrate node.
pub type SubstrateMessagesDeliveryProof<C> =
	(UnrewardedRelayersState, FromBridgedChainMessagesDeliveryProof<HashOf<C>>);

/// All runtime calls that are supported by the messages and finality relays.
#[derive(Clone, DebugNoBound)]
pub enum RuntimeCall<BridgedChain: Chain> {
	/// Runtime call that brings GRANDPA-finalized header of the bridged chain.
	GrandpaFinality(GrandpaFinalityCall<BridgedChain>),
	/// Runtime call that brings finalized header of the bridged parachain.
	ParachainFinality(ParachainFinalityCall<BridgedChain>),
	/// Runtime call that brings new messages from the bridged chain.
	MessagesDelivery(MessagesDeliveryCall<BridgedChain>),
	/// Runtime call that brings new delivery confirmations from the bridged chain.
	MessagesDeliveryConfirmation(MessagesDeliveryConfirmationCall<BridgedChain>),
	/// Runtime call that brings multiple runtime calls at once.
	BatchCall(Vec<RuntimeCall<BridgedChain>>),
}

/// Runtime call that brings GRANDPA-finalized header of the bridged chain.
#[derive(Clone, DebugNoBound)]
pub struct GrandpaFinalityCall<BridgedChain> {
	/// Finalized bridged chain header.
	pub header: SyncHeader<HeaderOf<BridgedChain>>,
	/// Finality proof of the bridged chain header.
	pub justification: GrandpaJustification<HeaderOf<BridgedChain>>,
}

/// Runtime call that brings finalized header of the bridged parachain.
#[derive(Clone, DebugNoBound)]
pub struct ParachainFinalityCall<BridgedChain> {
	/// Finalized bridged parachain header.
	pub header: SyncHeader<HeaderOf<BridgedChain>>,
	/// Finality proof of the bridged chain header.
	pub proof: ParaHeadsProof,
}

/// Runtime call that brings new messages from the bridged chain.
#[derive(Clone, DebugNoBound)]
pub struct MessagesDeliveryCall<BridgedChain> {
	/// Relayer account at the bridged chain.
	pub relayer_id_at_source: AccountIdOf<BridgedChain>,
	/// Bundled messages proof.
	pub proof: SubstrateMessagesProof<BridgedChain>,
}

/// Runtime call that brings new delivery confirmations from the bridged chain.
#[derive(Clone, DebugNoBound)]
pub struct MessagesDeliveryConfirmationCall<BridgedChain> {
	/// Bundled messages proof.
	pub proof: SubstrateMessagesDeliveryProof<BridgedChain>,
}
