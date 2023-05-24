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

use crate::{Chain, HeaderOf, SyncHeader};
use bp_header_chain::justification::GrandpaJustification;
use frame_support::DebugNoBound;

/// Runtime call that brings finalized header of the bridged chain.
#[derive(Clone, DebugNoBound)]
pub struct GrandpaFinalityCall<BridgedChain: Chain> {
	/// Finalized bridged chain header.
	pub header: SyncHeader<HeaderOf<BridgedChain>>,
	/// Finality proof of the bridged chain header.
	pub justification: GrandpaJustification<HeaderOf<BridgedChain>>,
}
