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

//! Types that allow runtime to act as a source/target endpoint of message lanes.
//!
//! Messages are assumed to be encoded `Call`s of the target chain. Call-dispatch
//! pallet is used to dispatch incoming messages. Message identified by a tuple
//! of to elements - message lane id and message nonce.

use bp_header_chain::HeaderChain;
use bp_messages::target_chain::FromBridgedChainMessagesProof;
pub use bp_runtime::{
	Chain, RangeInclusiveExt, RawStorageProof, Size, TrustedVecDb, UnderlyingChainOf,
	UnderlyingChainProvider, UntrustedVecDb,
};

/// Bidirectional message bridge.
pub trait MessageBridge {
	/// Name of the paired messages pallet instance at the Bridged chain.
	///
	/// Should be the name that is used in the `construct_runtime!()` macro.
	const BRIDGED_MESSAGES_PALLET_NAME: &'static str;

	/// This chain in context of message bridge.
	type ThisChain: ThisChainWithMessages;
	/// Bridged chain in context of message bridge.
	type BridgedChain: BridgedChainWithMessages;
	/// Bridged header chain.
	type BridgedHeaderChain: HeaderChain<UnderlyingChainOf<Self::BridgedChain>>;
}

/// This chain that has `pallet-bridge-messages` module.
pub trait ThisChainWithMessages: UnderlyingChainProvider {
	/// Call origin on the chain.
	type RuntimeOrigin;
}

/// Bridged chain that has `pallet-bridge-messages` module.
pub trait BridgedChainWithMessages: UnderlyingChainProvider {}

/// This chain in context of message bridge.
pub type ThisChain<B> = <B as MessageBridge>::ThisChain;
/// Bridged chain in context of message bridge.
pub type BridgedChain<B> = <B as MessageBridge>::BridgedChain;
/// Hash used on the chain.
pub type HashOf<C> = bp_runtime::HashOf<<C as UnderlyingChainProvider>::Chain>;
/// Hasher used on the chain.
pub type HasherOf<C> = bp_runtime::HasherOf<UnderlyingChainOf<C>>;
/// Account id used on the chain.
pub type AccountIdOf<C> = bp_runtime::AccountIdOf<UnderlyingChainOf<C>>;
/// Type of balances that is used on the chain.
pub type BalanceOf<C> = bp_runtime::BalanceOf<UnderlyingChainOf<C>>;
/// Type of origin that is used on the chain.
pub type OriginOf<C> = <C as ThisChainWithMessages>::RuntimeOrigin;

/// Sub-module that is declaring types required for processing This -> Bridged chain messages.
pub mod source {
	/// Message payload for This -> Bridged chain messages.
	pub type FromThisChainMessagePayload = crate::messages_xcm_extension::XcmAsPlainPayload;
}

/// Sub-module that is declaring types required for processing Bridged -> This chain messages.
pub mod target {
	/// Decoded Bridged -> This message payload.
	pub type FromBridgedChainMessagePayload = crate::messages_xcm_extension::XcmAsPlainPayload;
}

/// The `BridgeMessagesCall` used by a chain.
pub type BridgeMessagesCallOf<C> = bp_messages::BridgeMessagesCall<
	bp_runtime::AccountIdOf<C>,
	FromBridgedChainMessagesProof<bp_runtime::HashOf<C>>,
	bp_messages::source_chain::FromBridgedChainMessagesDeliveryProof<bp_runtime::HashOf<C>>,
>;
