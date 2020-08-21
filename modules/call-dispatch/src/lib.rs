// Copyright 2019-2020 Parity Technologies (UK) Ltd.
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

//! Runtime module which takes care of dispatching messages received over the bridge.
//!
//! The messages are interpreted directly as runtime `Call`s, we attempt to decode
//! them and then dispatch as usualy.
//! To prevent compatibility issues, the calls have to include `spec_version` as well
//! which is being checked before dispatch.
//!
//! In case of succesful dispatch event is emitted.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

use bp_message_dispatch::{MessageDispatch, BridgeOrigin};
use codec::{Encode, Decode};
use frame_support::{
	decl_module, debug,
	dispatch::{Parameter, Dispatchable},
	traits::Get,
};
use sp_io::hashing::blake2_256;

/// Spec version type.
pub type SpecVersion = u32;

/// The module configuration trait
pub trait Trait: frame_system::Trait {
	/// The overarching dispatch call type.
	type Call: Parameter + Dispatchable<Origin=<Self as frame_system::Trait>::Origin>;
}

decl_module! {
	/// Call Dispatch FRAME Pallet.
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {}
}

impl<T: Trait> MessageDispatch for Module<T> where
	<<T as Trait>::Call as Dispatchable>::PostInfo: sp_std::fmt::Debug
{
	type Message = (SpecVersion, <T as Trait>::Call);

	// TODO [ToDr] Weight calculations?
	fn dispatch(origin: BridgeOrigin, msg: Self::Message) -> bool {
		let (spec_version, msg) = msg;
		// verify spec version
		let expected_version = <T as frame_system::Trait>::Version::get().spec_version;
		if spec_version != expected_version {
			debug::native::error!(
				"Mismatching spec_version. Expected {:?}, got {:?}",
				expected_version,
				spec_version
			);
			return false
		}

		let bridge_origin = || Self::bridge_account_id(origin);
		if let Err(e) = msg.dispatch(frame_system::RawOrigin::Signed(bridge_origin()).into()) {
			debug::native::error!("Error while dispatching bridge call: {:?}: {:?}", bridge_origin(), e);
			false
		} else {
			true
		}
	}
}

impl<T: Trait> Module<T> {
	fn bridge_account_id(origin: BridgeOrigin) -> T::AccountId {
		let entropy = (b"pallet-bridge/call-dispatch", origin).using_encoded(blake2_256);
		T::AccountId::decode(&mut &entropy[..]).unwrap_or_default()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::{
		assert_noop, assert_ok, impl_outer_origin, impl_outer_dispatch, parameter_types,
		weights::Weight
	};
	use sp_core::H256;
	use sp_runtime::{
		testing::Header,
		traits::{BlakeTwo256, IdentityLookup},
		Perbill,
	};

	type AccountId = u64;
	type CallDispatch = Module<TestRuntime>;
	type System = frame_system::Module<TestRuntime>;

	#[derive(Clone, Eq, PartialEq)]
	pub struct TestRuntime;

	impl_outer_origin! {
		pub enum Origin for TestRuntime where system = frame_system {}
	}

	impl_outer_dispatch! {
		pub enum Call for TestRuntime where origin: Origin {
			frame_system::System,
			call_dispatch::CallDispatch,
		}
	}

	parameter_types! {
		pub const BlockHashCount: u64 = 250;
		pub const MaximumBlockWeight: Weight = 1024;
		pub const MaximumBlockLength: u32 = 2 * 1024;
		pub const AvailableBlockRatio: Perbill = Perbill::one();
	}

	impl frame_system::Trait for TestRuntime {
		type Origin = Origin;
		type Index = u64;
		type Call = Call;
		type BlockNumber = u64;
		type Hash = H256;
		type Hashing = BlakeTwo256;
		type AccountId = AccountId;
		type Lookup = IdentityLookup<Self::AccountId>;
		type Header = Header;
		type Event = ();
		type BlockHashCount = BlockHashCount;
		type MaximumBlockWeight = MaximumBlockWeight;
		type DbWeight = ();
		type BlockExecutionWeight = ();
		type ExtrinsicBaseWeight = ();
		type MaximumExtrinsicWeight = ();
		type AvailableBlockRatio = AvailableBlockRatio;
		type MaximumBlockLength = MaximumBlockLength;
		type Version = ();
		type ModuleToIndex = ();
		type AccountData = ();
		type OnNewAccount = ();
		type OnKilledAccount = ();
		type BaseCallFilter = ();
		type SystemWeightInfo = ();
	}

	impl Trait for TestRuntime {
		type Call = Call;
	}

	fn new_test_ext() -> sp_io::TestExternalities {
		let t = frame_system::GenesisConfig::default()
			.build_storage::<TestRuntime>()
			.unwrap();
		sp_io::TestExternalities::new(t)
	}


	#[test]
	fn should_succesfuly_dispatch_remark() {
		new_test_ext().execute_with(|| {
			let origin = b"eth".to_owned();
			let message = (0, Call::System(
				<frame_system::Call<TestRuntime>>::remark(vec![1, 2, 3])
			));
			assert!(CallDispatch::dispatch(origin, message));
		});
	}

	#[test]
	fn should_fail_on_spec_version_mismatch() {
		new_test_ext().execute_with(|| {
			let origin = b"eth".to_owned();
			let message = (69, Call::System(
				<frame_system::Call<TestRuntime>>::remark(vec![1, 2, 3])
			));
			assert!(!CallDispatch::dispatch(origin, message));
		});
	}

	#[test]
	fn should_dispatch_from_non_root_origin() {
		new_test_ext().execute_with(|| {
			let origin = b"eth".to_owned();
			let message = (69, Call::System(
				<frame_system::Call<TestRuntime>>::fill_block(Perbill::from_percent(10))
			));
			assert!(!CallDispatch::dispatch(origin, message));
		});
	}
}
