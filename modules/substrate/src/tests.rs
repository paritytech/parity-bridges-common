// Copyright 2020 Parity Technologies (UK) Ltd.
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

#![cfg(test)]

use crate::mock::*;
use crate::{BridgeStorage, Module, PalletStorage};
use bp_substrate::ImportedHeader;
use frame_support::assert_ok;

type Bridge = Module<TestRuntime>;

#[test]
fn it_works() {
	run_test(|| {
		assert_ok!(Bridge::test(Origin::signed(1)));
	})
}

#[test]
fn reading_storage_works() {
	run_test(|| {
		let storage = PalletStorage::<TestRuntime>::new();
		assert!(storage.best_finalized_header().is_none());
	})
}

#[test]
fn writing_to_storage_works() {
	run_test(|| {
		let mut storage = PalletStorage::<TestRuntime>::new();
		let header = <TestRuntime as frame_system::Trait>::Header::new_from_number(1);
		let imported_header = ImportedHeader {
			header: header.clone(),
			is_finalized: false,
		};
		storage.write_header(&imported_header);

		let hash = header.hash();
		let header = storage.get_header_by_hash(hash);
		assert_eq!(imported_header, header.unwrap())
	})
}
