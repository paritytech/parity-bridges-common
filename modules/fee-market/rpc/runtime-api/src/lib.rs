// This file is part of Darwinia.
//
// Copyright (C) 2018-2022 Darwinia Network
// SPDX-License-Identifier: GPL-3.0
//
// Darwinia is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Darwinia is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Darwinia. If not, see <https://www.gnu.org/licenses/>.

//! Runtime API definition required by header-mmr RPC extensions.
//!
//! This API should be imported and implemented by the runtime,
//! of a node that wants to use the custom RPC extension
//! adding header-mmr access methods.

#![cfg_attr(not(feature = "std"), no_std)]

// --- crates.io ---
use codec::{Codec, Decode, Encode};
#[cfg(feature = "std")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};
// --- paritytech ---
use bp_messages::{LaneId, MessageNonce};
use sp_api::decl_runtime_apis;
use sp_runtime::traits::{MaybeDisplay, MaybeFromStr};
use sp_std::vec::Vec;

#[derive(Eq, PartialEq, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct Fee<Balance> {
	#[cfg_attr(feature = "std", serde(bound(serialize = "Balance: std::fmt::Display")))]
	#[cfg_attr(feature = "std", serde(serialize_with = "serialize_as_string"))]
	#[cfg_attr(feature = "std", serde(bound(deserialize = "Balance: std::str::FromStr")))]
	#[cfg_attr(feature = "std", serde(deserialize_with = "deserialize_from_string"))]
	pub amount: Balance,
}

#[cfg(feature = "std")]
fn serialize_as_string<S: Serializer, T: std::fmt::Display>(
	t: &T,
	serializer: S,
) -> Result<S::Ok, S::Error> {
	serializer.serialize_str(&t.to_string())
}

#[cfg(feature = "std")]
fn deserialize_from_string<'de, D: Deserializer<'de>, T: std::str::FromStr>(
	deserializer: D,
) -> Result<T, D::Error> {
	let s = String::deserialize(deserializer)?;
	s.parse::<T>().map_err(|_| serde::de::Error::custom("Parse from string failed"))
}

#[derive(Eq, PartialEq, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct InProcessOrders {
	pub orders: Vec<(LaneId, MessageNonce)>,
}

decl_runtime_apis! {
	pub trait FeeMarketApi<Balance>
	where
		Balance: Codec + MaybeDisplay + MaybeFromStr,
	 {
		fn market_fee(
		) -> Option<Fee<Balance>>;

		fn in_process_orders() -> InProcessOrders;
	}
}
