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

//! Primitives of the `xcm-bridge-hub-router` pallet.

#![cfg_attr(not(feature = "std"), no_std)]

use bp_messages::MessageNonce;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::RuntimeDebug;
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use sp_runtime::{traits::Zero, FixedU128};

/// Bridge limits.
#[derive(Clone, RuntimeDebug)]
pub struct BridgeLimits<BlockNumber> {
	/// Maximal delay in blocks before we expect to receive bridge state report. If we have
	/// issued request `bridge_state_report_delay + 1` blocks ago and still have not received
	/// report, we consider that something is wrong with the pipeline and start increasing
	/// message fee.
	pub maximal_bridge_state_report_delay: BlockNumber,
	/// Minimal delay in blocks between our state report requests.
	pub minimal_bridge_state_request_delay: BlockNumber,
	/// Maximal allowed number of queued messages across all bridge queues. If number of messages
	/// is larger than this threshold, every following message will lead to fee increase.
	pub increase_fee_factor_threshold: MessageNonce,
	/// Maximal number of queued messages before we will start
	pub send_report_bridge_state_threshold: MessageNonce,
}

// TODO: this should be moved to `bp-xcm-bridge-hub`.
/// All bridge queues state.
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
pub struct AtBridgeHubBridgeQueuesState {
	/// Number of messages queued at sibling (source) bridge hub outbound queue.
	pub outbound_at_sibling: MessageNonce,
	/// Number of messages queued at bridged (target) bridge hub outbound queue.
	pub outbound_at_bridged: MessageNonce,
	/// Number of messages queued at the destination inbound queue.
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

impl AtBridgeHubBridgeQueuesState {
	/// Return total number of messsages that we assume are currently in the bridges queue.
	pub fn total_enqueued_messages(&self) -> MessageNonce {
		self.outbound_at_sibling
			.saturating_add(self.outbound_at_bridged)
			.saturating_add(self.inbound_at_destination)
	}
}

/// State of the last bridge state report request;
pub struct ReportRequestState<BlockNumber> {
	/// The number of block, at which we have sent our last bridge state report request.
	pub sent_at: Option<BlockNumber>,
	/// The number of messages that were sent after we have issued our report request.
	pub messages_sent_after_request: MessageNonce,
}

/// Current state of bridge with the remote deestination.
pub struct Bridge<BlockNumber> {
	/// If true, the bridge is currently in "relieving" state, where we are decreasing
	/// the fee factor at every block.
	pub is_relieving: bool,
	/// The number to multiply the base message delivery fee by. We will increase this
	/// value exponentially when we the bridge throughput decreases and decrease after
	/// it is back to normal.
	pub fee_factor: FixedU128,
	/// Count of undelivered bridge messages as we see it. The actual number may be lower
	/// if some messages are already delivered, but we have not yet received a report.
	/// The actual number may be higher e.g. if previous report estimation was incorrect
	/// or if some messages have not yet been accounted by the report.
	///
	/// This field is incremented by one every time we send a message. This field is changed
	/// to the reported number every time we receive a state report.
	pub total_enqueued_messages: MessageNonce,
	/// The number of block, at which we have received last bridge state report. If we have
	/// never received a report, it is set to zero.
	pub last_report_block: BlockNumber,
	/// Status of the last bridge state report request. If `None`, then there are no active
	/// report request.
	pub last_report_request: Option<ReportRequestState<BlockNumber>>,
}

impl<BlockNumber: Zero> Default for Bridge<BlockNumber> {
	fn default() -> Self {
		Bridge {
			is_relieving: false,
			fee_factor: FixedU128::from_u32(1),
			total_enqueued_messages: 0,
			last_report_block: Zero::zero(),
			last_report_request: None,
		}
	}
}
