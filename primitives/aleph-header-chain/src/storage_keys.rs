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

//! Storage keys of bridge Aleph pallet.

/// Name of the `IsHalted` storage value.
pub const PALLET_OPERATING_MODE_VALUE_NAME: &str = "PalletOperatingMode";
/// Name of the `BestFinalized` storage value.
pub const BEST_FINALIZED_VALUE_NAME: &str = "BestFinalized";
/// Name of the `CurrentAuthoritySet` storage value.
pub const CURRENT_AUTHORITY_SET_VALUE_NAME: &str = "CurrentAuthoritySet";

use sp_core::storage::StorageKey;

/// Storage key of the `PalletOperatingMode` variable in the runtime storage.
pub fn pallet_operating_mode_key(pallet_prefix: &str) -> StorageKey {
	StorageKey(
		bp_runtime::storage_value_final_key(
			pallet_prefix.as_bytes(),
			PALLET_OPERATING_MODE_VALUE_NAME.as_bytes(),
		)
		.to_vec(),
	)
}

/// Storage key of the `CurrentAuthoritySet` variable in the runtime storage.
pub fn current_authority_set_key(pallet_prefix: &str) -> StorageKey {
	StorageKey(
		bp_runtime::storage_value_final_key(
			pallet_prefix.as_bytes(),
			CURRENT_AUTHORITY_SET_VALUE_NAME.as_bytes(),
		)
		.to_vec(),
	)
}

/// Storage key of the best finalized header number and hash value in the runtime storage.
pub fn best_finalized_key(pallet_prefix: &str) -> StorageKey {
	StorageKey(
		bp_runtime::storage_value_final_key(
			pallet_prefix.as_bytes(),
			BEST_FINALIZED_VALUE_NAME.as_bytes(),
		)
		.to_vec(),
	)
}
