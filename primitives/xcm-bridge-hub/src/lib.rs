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

//! Primitives of the xcm-bridge-hub pallet.

#![cfg_attr(not(feature = "std"), no_std)]

use bp_messages::MessageNonce;
use bp_runtime::{AccountIdOf, BalanceOf, BlockNumberOf, Chain};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{CloneNoBound, PartialEqNoBound, RuntimeDebug, RuntimeDebugNoBound};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};

/// Bridge state.
#[derive(Clone, Copy, Decode, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub enum BridgeState<BlockNumber> {
	/// Bridge is opened. Associated lanes are also opened.
	Opened,
	/// Bridge is closing. It will switch to closed state at given block.
	/// Outbound lane is either closed (if bridged is closing because of misbehavior), or it
	/// is closing. Inbound lane is in closing state.
	Closing(BlockNumber),
	/// Bridge is closed. Associated lanes are also closed.
	/// After all outbound messages will be pruned, the bridge will vanish without any traces.
	Closed,
}

/// Bridge metadata.
#[derive(
	CloneNoBound, Decode, Encode, Eq, PartialEqNoBound, TypeInfo, MaxEncodedLen, RuntimeDebugNoBound,
)]
#[scale_info(skip_type_params(ThisChain))]
pub struct Bridge<ThisChain: Chain> {
	/// Current bridge state.
	pub state: BridgeState<BlockNumberOf<ThisChain>>,
	/// Account with the reserved funds.
	pub bridge_owner_account: AccountIdOf<ThisChain>,
	/// Reserved amount on the sovereign account of the sibling bridge origin.
	pub reserve: BalanceOf<ThisChain>,
}

/// Bridge limits. Bridges that exceed those limits may be reported, fined and closed.
#[derive(Clone, Copy, Decode, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub struct BridgeLimits {
	/// Maximal number of outbound messages that may be queued at the outbound lane at a time.
	/// Normally a bridge maintainers must run at least one relayer that will deliver messages
	/// to the bridged chain and confirm delivery. If there's no relayer running, messages will
	/// keep piling up, which will lead to trie growth, which we don't want.
	///
	/// This limit must be selected with care - it should account possible delays because of
	/// runtime upgrades, spamming queues, finality lags and so on.
	pub max_queued_outbound_messages: MessageNonce,
	// TODO: limit to detect relayer activity - i.e. if there are 10 queued messages, but they
	// are not delivered for 14 days => misbehavior

	// TODO: too low funds on the relayers-fund account?
}

/// Bridge misbehavior.
#[derive(
	Clone,
	Copy,
	Decode,
	Encode,
	Eq,
	PartialEq,
	TypeInfo,
	MaxEncodedLen,
	RuntimeDebug,
	Serialize,
	Deserialize,
)]
pub enum BridgeMisbehavior {
	/// The number of messages in the outbound queue is larger than the limit.
	TooManyQueuedOutboundMessages,
}

/// The state of all (reachable) queues as they seen from the bridge hub.
#[derive(
	Clone,
	Copy,
	Decode,
	Default,
	Encode,
	Eq,
	Ord,
	PartialOrd,
	PartialEq,
	RuntimeDebug,
	TypeInfo,
	MaxEncodedLen,
	Serialize,
	Deserialize,
)]
pub struct BridgeQueuesState {
	/// Number of messages queued at bridge hub outbound (`pallet-bridge-messages`) queue.
	pub outbound_here: MessageNonce,
	/// Number of messages queued at the outbound queue of the bridged bridge hub. This
	/// queue connects bridged bridge hub and remote bridge destination. In most cases
	/// it will be the XCMP (HRMP) or DMP queue.
	pub outbound_at_bridged: MessageNonce,
	/// Number of messages queued at the destination inbound queue. This queue connects
	/// bridged bridge hub and remote bridge destination. In most cases it will be the XCMP
	/// (HRMP) or DMP queue.
	///
	/// Bridged (target) bridge hub doesn't have an access to the exact value of
	/// this metric. But it may get an estimation, depending on the channel
	/// state. The channel between target brige hub and desination is suspended
	/// when there are more than `N` unprocessed messages at the destination inbound
	/// queue. So if we see the suspended channel state at the target bridge hub,
	/// we: (1) assume that there's at least `N` queued messages at the inbound
	/// destination queue and (2) all further messages are now piling up at our
	/// outbound queue (`outbound_at_bridged`), so we have exact count.
	pub inbound_at_destination: MessageNonce,
}

impl BridgeQueuesState {
	/// Return total number of messsages that we assume are currently in the bridges queue.
	pub fn total_enqueued_messages(&self) -> MessageNonce {
		self.outbound_here
			.saturating_add(self.outbound_at_bridged)
			.saturating_add(self.inbound_at_destination)
	}
}
