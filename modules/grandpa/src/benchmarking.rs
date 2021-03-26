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

use crate::*;

use bp_test_utils::{
	accounts, authority_list, make_justification_for_header, test_keyring, JustificationGeneratorParams, ALICE,
	TEST_GRANDPA_ROUND, TEST_GRANDPA_SET_ID,
};
use frame_benchmarking::{benchmarks_instance_pallet, whitelisted_caller};
use frame_support::traits::Get;
use frame_system::RawOrigin;
use num_traits::cast::AsPrimitive;
use sp_finality_grandpa::AuthorityId;
use sp_runtime::traits::{One, Zero};
use sp_std::vec;

benchmarks_instance_pallet! {
	submit_finality_proof {
		let s in 1..T::BridgedSessionLength::get().as_() as u32;
		let p in 1..T::BridgedValidatorCount::get() as u32;

		let caller: T::AccountId = whitelisted_caller();

		let authority_list = accounts(p as u8)
			.iter()
			.map(|id| (AuthorityId::from(*id), 1))
			.collect::<Vec<_>>();

		let init_data = InitializationData {
			header: bp_test_utils::test_header(Zero::zero()),
			authority_list,
			set_id: TEST_GRANDPA_SET_ID,
			is_halted: false,
		};

		initialize_bridge::<T, I>(init_data);
		let header: BridgedHeader<T, I> = bp_test_utils::test_header(One::one());

		let params = JustificationGeneratorParams {
			header: header.clone(),
			round: TEST_GRANDPA_ROUND,
			set_id: TEST_GRANDPA_SET_ID,
			authorities: accounts(p as u8).iter().map(|k| (*k, 1)).collect::<Vec<_>>(),
			depth: s,
			forks: p,
		};

		let justification = make_justification_for_header(params).encode();

	}: _(RawOrigin::Signed(caller), header, justification)
	verify {
		let header: BridgedHeader<T, I> = bp_test_utils::test_header(One::one());
		let expected_hash = header.hash();

		assert_eq!(<BestFinalized<T, I>>::get(), expected_hash);
		assert!(<ImportedHeaders<T, I>>::contains_key(expected_hash));
	}

	// What we want to check here is the effect of vote ancestries on justification verification
	// time. We will do this by varying the number of ancestors our finality target has.
	submit_finality_proof_on_single_fork {
		let s in 1..T::BridgedSessionLength::get().as_() as u32;

		let caller: T::AccountId = whitelisted_caller();

		let init_data = InitializationData {
			header: bp_test_utils::test_header(Zero::zero()),
			authority_list: authority_list(),
			set_id: TEST_GRANDPA_SET_ID,
			is_halted: false,
		};

		initialize_bridge::<T, I>(init_data);
		let header: BridgedHeader<T, I> = bp_test_utils::test_header(One::one());

		let params = JustificationGeneratorParams {
			header: header.clone(),
			round: TEST_GRANDPA_ROUND,
			set_id: TEST_GRANDPA_SET_ID,
			authorities: test_keyring(),
			depth: s,
			forks: 1,
		};

		let justification = make_justification_for_header(params).encode();

	}: submit_finality_proof(RawOrigin::Signed(caller), header, justification)
	verify {
		let header: BridgedHeader<T, I> = bp_test_utils::test_header(One::one());
		let expected_hash = header.hash();

		assert_eq!(<BestFinalized<T, I>>::get(), expected_hash);
		assert!(<ImportedHeaders<T, I>>::contains_key(expected_hash));
	}

	// What we want to check here is the effect of many pre-commits on justification verification.
	// We do this by creating many forks, whose head will be used as a signed pre-commit in the
	// final justification.
	submit_finality_proof_on_many_forks {
		let p in 1..T::BridgedValidatorCount::get() as u32;

		let caller: T::AccountId = whitelisted_caller();

		let authority_list = accounts(p as u8)
			.iter()
			.map(|id| (AuthorityId::from(*id), 1))
			.collect::<Vec<_>>();

		let init_data = InitializationData {
			header: bp_test_utils::test_header(Zero::zero()),
			authority_list,
			set_id: TEST_GRANDPA_SET_ID,
			is_halted: false,
		};

		initialize_bridge::<T, I>(init_data);
		let header: BridgedHeader<T, I> = bp_test_utils::test_header(One::one());

		let params = JustificationGeneratorParams {
			header: header.clone(),
			round: TEST_GRANDPA_ROUND,
			set_id: TEST_GRANDPA_SET_ID,
			authorities: accounts(p as u8).iter().map(|k| (*k, 1)).collect::<Vec<_>>(),
			depth: 2,
			forks: p,
		};

		let justification = make_justification_for_header(params).encode();

	}: submit_finality_proof(RawOrigin::Signed(caller), header, justification)
	verify {
		let header: BridgedHeader<T, I> = bp_test_utils::test_header(One::one());
		let expected_hash = header.hash();

		assert_eq!(<BestFinalized<T, I>>::get(), expected_hash);
		assert!(<ImportedHeaders<T, I>>::contains_key(expected_hash));
	}

	// Here we want to find out the overheaded of looking through consensus digests found in a
	// header. As the number of logs in a header grows, how much more work do we require to look
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

		let mut header: BridgedHeader<T, I> = bp_test_utils::test_header(Zero::zero());
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

	#[test]
	fn finality_proof_is_valid() {
		mock::run_test(|| {
			assert_ok!(test_benchmark_submit_finality_proof::<mock::TestRuntime>());
		});
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
