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

use frame_support::{traits::Instance, StorageHasher};
use sp_core::storage::StorageKey;
use sp_std::prelude::*;

/// Key of the entry in the storage map, that is a part of pallet without instances.
pub fn storage_map_final_key(map_name: &str, key: &[u8]) -> StorageKey {
	storage_map_final_key_with_prefix("", map_name, key)
}

/// Key of the entry in the storage map, that is a part of pallet without instances.
pub fn storage_map_final_key_with_instance<I: Instance>(map_name: &str, key: &[u8]) -> StorageKey {
	storage_map_final_key_with_prefix(I::PREFIX, map_name, key)
}

fn storage_map_final_key_with_prefix(module_prefix: &str, map_name: &str, key: &[u8]) -> StorageKey {
	let module_prefix_hashed = frame_support::Twox128::hash(module_prefix.as_bytes());
	let storage_prefix_hashed = frame_support::Twox128::hash(map_name.as_bytes());
	let key_hashed = frame_support::Blake2_128Concat::hash(key);

	let mut final_key =
		Vec::with_capacity(module_prefix_hashed.len() + storage_prefix_hashed.len() + key_hashed.len());

	final_key.extend_from_slice(&module_prefix_hashed[..]);
	final_key.extend_from_slice(&storage_prefix_hashed[..]);
	final_key.extend_from_slice(key_hashed.as_ref());

	StorageKey(final_key)
}
