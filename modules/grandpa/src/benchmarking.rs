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

//! Benchmarks for the GRANDPA Pallet.

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

use bp_test_utils::{
	accounts, authority_list, make_justification_for_header, test_keyring, JustificationGeneratorParams, ALICE,
	TEST_GRANDPA_ROUND, TEST_GRANDPA_SET_ID,
};
use frame_benchmarking::{benchmarks_instance_pallet, whitelisted_caller};
use frame_system::RawOrigin;
use num_traits::cast::AsPrimitive;
use sp_finality_grandpa::AuthorityId;
use sp_runtime::traits::{One, Zero};
use sp_std::vec;

pub trait Config<I: 'static = ()>: crate::Config<I> {
	// We need some way for the benchmarks to use headers in a "generic" way. However, since we do
	// use a real runtime we need a way for the runtime to tell us what the concrete type is.
	fn bridged_header(num: BridgedBlockNumber<Self, I>) -> BridgedHeader<Self, I>;
	fn session_length() -> BridgedBlockNumber<Self, I>;
}

benchmarks_instance_pallet! {
	// What we want to check here is the effect of vote ancestries on justification verification
	// time. We will do this by varying the number of ancestors our finality target has.
	submit_finality_proof_on_single_fork {
		let n in 1..T::session_length().as_() as u32;
		let caller: T::AccountId = whitelisted_caller();

		let init_data = InitializationData {
			header: T::bridged_header(Zero::zero()),
			authority_list: authority_list(),
			set_id: TEST_GRANDPA_SET_ID,
			is_halted: false,
		};

		initialize_bridge::<T, I>(init_data);

		let mut header = T::bridged_header(One::one());
		header.set_parent_hash(*T::bridged_header(Zero::zero()).parent_hash());

		let params = JustificationGeneratorParams {
			header: header.clone(),
			round: TEST_GRANDPA_ROUND,
			set_id: TEST_GRANDPA_SET_ID,
			authorities: test_keyring(),
			depth: n,
			forks: 1,
		};

		let justification = make_justification_for_header(params).encode();

	}: submit_finality_proof(RawOrigin::Signed(caller), header, justification)
	verify {
		let mut header = T::bridged_header(One::one());
		header.set_parent_hash(*T::bridged_header(Zero::zero()).parent_hash());
		let expected_hash = header.hash();

		assert_eq!(<BestFinalized<T, I>>::get(), expected_hash);
		assert!(<ImportedHeaders<T, I>>::contains_key(expected_hash));
	}

	// What we want to check here is the effect of many pre-commits on justification verification.
	// We do this by creating many forks, whose head will be used as a signed pre-commit in the
	// final justification.
	submit_finality_proof_on_many_forks {
		let n in 1..u8::MAX.into();
		let caller: T::AccountId = whitelisted_caller();

		let authority_list = accounts(n as u8)
			.iter()
			.map(|id| (AuthorityId::from(*id), 1))
			.collect::<Vec<_>>();

		let init_data = InitializationData {
			header: T::bridged_header(Zero::zero()),
			authority_list,
			set_id: TEST_GRANDPA_SET_ID,
			is_halted: false,
		};

		initialize_bridge::<T, I>(init_data);

		let mut header = T::bridged_header(One::one());
		header.set_parent_hash(*T::bridged_header(Zero::zero()).parent_hash());

		let params = JustificationGeneratorParams {
			header: header.clone(),
			round: TEST_GRANDPA_ROUND,
			set_id: TEST_GRANDPA_SET_ID,
			authorities: accounts(n as u8).iter().map(|k| (*k, 1)).collect::<Vec<_>>(),
			depth: 2,
			forks: n,
		};

		let justification = make_justification_for_header(params).encode();

	}: submit_finality_proof(RawOrigin::Signed(caller), header, justification)
	verify {
		let mut header = T::bridged_header(One::one());
		header.set_parent_hash(*T::bridged_header(Zero::zero()).parent_hash());
		let expected_hash = header.hash();

		assert_eq!(<BestFinalized<T, I>>::get(), expected_hash);
		assert!(<ImportedHeaders<T, I>>::contains_key(expected_hash));
	}


	// Here we want to find out what the overheader of looking for an enacting an authority set is.
	// I think we can combine the two benchmarks below into this single one...
	// enacts_authority_set  {
	// 	todo!()
	// }: try_enact_authority_change(header)
	// verify {
	// 	assert!(true)
	// }

	// Here we want to find out the overheaded of looking through consensus digests found in a
	// header.
	//
	// E.g, as the number of logs in a header grows, how much more work do we require to look
	// through them?
	//
	// Note that this should be the same for looking through scheduled changes and forces changes,
	// which is why we only have one benchmark for this.
	find_scheduled_change {
		// Not really sure what a good bound for this is.
		let n in 1..1000;

		let mut logs = vec![];
		for i in 0..n {
			// We chose a non-consensus log on purpose since that way we have to look through all
			// the logs in the header
			logs.push(sp_runtime::DigestItem::Other(vec![]));
		}

		let mut header = T::bridged_header(Zero::zero());
		let digest = header.digest_mut();
		*digest = sp_runtime::Digest {
			logs,
		};

	}: {
		crate::find_scheduled_change(&header)
	}

	// What we want to check here is how long it takes to read and write the authority set tracked
	// by the pallet as the number of authorities grows.
	read_write_authority_sets {
		// The current max target number of validators on Polkadot/Kusama
		let n in 1..1000;

		let mut authorities = vec![];
		for i in 0..n {
			authorities.push((ALICE, 1));
		}

		let authority_set = bp_header_chain::AuthoritySet {
			authorities: authorities.iter().map(|(id, w)| (AuthorityId::from(*id), *w)).collect(),
			set_id: 0
		};

		<CurrentAuthoritySet<T, I>>::put(&authority_set);

	}: {
		let authority_set = <CurrentAuthoritySet<T, I>>::get();
		<CurrentAuthoritySet<T, I>>::put(&authority_set);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::assert_ok;

	impl Config for mock::TestRuntime {
		fn bridged_header(num: u64) -> BridgedHeader<Self, ()> {
			mock::test_header(num)
		}

		fn session_length() -> BridgedBlockNumber<Self, ()> {
			5
		}
	}

	#[test]
	fn single_fork_finality_proof_is_valid() {
		mock::run_test(|| {
			assert_ok!(test_benchmark_submit_finality_proof_on_single_fork::<mock::TestRuntime>());
		});
	}

	#[test]
	fn multi_fork_finality_proof_is_valid() {
		mock::run_test(|| {
			assert_ok!(test_benchmark_submit_finality_proof_on_many_forks::<mock::TestRuntime>());
		});
	}
}
