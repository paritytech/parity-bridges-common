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

//! Primitives of the `pallet-xcm-bridge-hub-router` pallet.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{FixedU128, RuntimeDebug};

pub use bp_xcm_bridge_hub::{
	bridge_id_from_locations, bridge_locations, BridgeId, LocalXcmChannelManager,
};

/// All required bridge details, known to the chain that uses XCM bridge hub for
/// sending messages.
#[derive(Clone, Decode, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub struct Bridge<BlockNumber> {
	/// The bridge-specific number to multiply the base delivery fee by.
	///
	/// This is a second component of the total fee factor. The first component is
	/// shared by all bridges and depends on the physical HRMP congestion.
	pub bridge_fee_factor: FixedU128,
	/// A latest block, at which the bridge has been resumed. If bridge is currently
	/// suspended, it is `None`.
	pub bridge_resumed_at: Option<BlockNumber>,
}

impl<BlockNumber> Bridge<BlockNumber> {
	/// Returns true if bridge is currently suspended.
	pub fn is_suspended(&self) -> bool {
		self.bridge_resumed_at.is_none()
	}
}
