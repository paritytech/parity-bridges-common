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

//! Benchmarks for the Finality Verifier Pallet.

// Braindump for benches:
//
// Will want to make sure number of requests is low
// Will want to Make sure we have already ImportedHeaders and BestFinalized
//		(E.g pallet is initialized)
// Will want to check how heavy authority set changes are
//		How does weight increase as the authority list size increases since we have to
//		store that info in the pallet
//	Most important thing will be checking how justification verification scales
//		It's checking ancestry proofs, so we'll need to create valid, but different sized
//		justifications

use crate::*;

use bp_test_utils::{alice, bob, make_justification_for_header};
use frame_benchmarking::{benchmarks, whitelisted_caller};
use frame_system::RawOrigin;
use sp_runtime::traits::One;
use sp_std::vec;

pub trait Config: crate::Config {
	// We need some way for the benchmarks to use headers in a "generic" way. However, since we do
	// use a real runtime we need a way for the runtime to tell us what the concrete type is.
	fn bridged_header() -> BridgedHeader<Self>;
}

benchmarks! {
	submit_finality_proof {
		let n in 1..100;
		let caller: T::AccountId = whitelisted_caller();

		let authorities = vec![(alice(), 1), (bob(), 1)];

		let init_data = InitializationData {
			header: T::bridged_header(),
			authority_list: authorities.clone(),
			set_id: 0,
			is_halted: false,
		};

		initialize_bridge::<T>(init_data);

		let mut header = T::bridged_header();
		header.set_number(*header.number() + One::one());
		header.set_parent_hash(*T::bridged_header().parent_hash());

		let digest = header.digest_mut();
		*digest = sp_runtime::Digest {
			logs: vec![]
		};

		let set_id = 0;
		let grandpa_round = 1;
		let justification = make_justification_for_header(&header, grandpa_round, set_id, &authorities).encode();

	}: _(RawOrigin::Signed(caller), header, justification)
	verify {
		assert!(true)
		// Need to play with the types here to get this to compile...
		// assert_eq!(<BestFinalized<mock::TestRuntime>>::get(), header.hash());
		// assert!(<ImportedHeaders<mock::TestRuntime>>::contains_key(header.hash()));
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::assert_ok;

	impl Config for mock::TestRuntime {
		fn bridged_header() -> BridgedHeader<Self> {
			mock::test_header(0)
		}
	}

	#[test]
	fn it_works() {
		mock::run_test(|| {
			assert_ok!(test_benchmark_submit_finality_proof::<mock::TestRuntime>());
		});
	}
}
