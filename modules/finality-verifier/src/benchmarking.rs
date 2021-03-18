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

use bp_test_utils::{
	make_justification_for_header,
	Keyring::{Alice, Bob},
};
use frame_benchmarking::{benchmarks_instance, whitelisted_caller};
use frame_support::traits::Instance;
use frame_system::RawOrigin;
use sp_finality_grandpa::AuthorityId;
use sp_runtime::traits::One;
use sp_std::vec;

pub trait Config<I: 'static>: crate::Config<I> {
	// We need some way for the benchmarks to use headers in a "generic" way. However, since we do
	// use a real runtime we need a way for the runtime to tell us what the concrete type is.
	fn bridged_header() -> BridgedHeader<Self, I>;
}

benchmarks_instance! {
	submit_finality_proof {
		let n in 1..100;
		let caller: T::AccountId = whitelisted_caller();

		let authorities = vec![(Alice.into(), 1), (Bob.into(), 1)];

		let init_data = InitializationData {
			header: T::bridged_header(),
			authority_list: authorities.clone(),
			set_id: 0,
			is_halted: false,
		};

		initialize_bridge::<T, I>(init_data);

		let mut header = T::bridged_header();
		header.set_number(*header.number() + One::one());
		header.set_parent_hash(*T::bridged_header().parent_hash());

		let digest = header.digest_mut();
		*digest = sp_runtime::Digest {
			logs: vec![]
		};

		let set_id = 0;
		let grandpa_round = 1;
		let justification = make_justification_for_header(&header, grandpa_round, set_id, &vec![(Alice, 1), (Bob, 1)]).encode();

	}: _(RawOrigin::Signed(caller), header, justification)
	verify {
		assert!(true)
		// Need to play with the types here to get this to compile...
		// assert_eq!(<BestFinalized<mock::TestRuntime>>::get(), header.hash());
		// assert!(<ImportedHeaders<mock::TestRuntime>>::contains_key(header.hash()));
	}

	// What we want to check here is how the number of commits/precommits/vote ancestries affects
	// the verification time of justifications. With the helper function we have this number grows
	// based off the number of authorities, so we'll use that as a proxy for the number of
	// commits/precommits/vote ancestries.
	submit_finality_proof_justification_verification {
		// The current max target number of validators on Polkadot/Kusama
		// Looks like 1000 is too high for tests...
		let n in 1..1000;
		let caller: T::AccountId = whitelisted_caller();

		let mut authorities = vec![];
		for i in 0..n {
			// Do we need to have different identities for the authorities?
			authorities.push((Alice, 1));
		}

		let init_data = InitializationData {
			header: T::bridged_header(),
			authority_list: authorities.iter().map(|(id, w)| (AuthorityId::from(*id), *w)).collect(),
			set_id: 0,
			is_halted: false,
		};

		initialize_bridge::<T, I>(init_data);

		let mut header = T::bridged_header();
		header.set_number(*header.number() + One::one());
		header.set_parent_hash(*T::bridged_header().parent_hash());

		let set_id = 0;
		let grandpa_round = 1;
		let justification = make_justification_for_header(&header, grandpa_round, set_id, &authorities).encode();

	}: submit_finality_proof(RawOrigin::Signed(caller), header, justification)
	verify {
		assert!(true)
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

		let mut header = T::bridged_header();
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
			authorities.push((Alice, 1));
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

	#[test]
	fn it_also_works() {
		mock::run_test(|| {
			assert_ok!(test_benchmark_submit_finality_proof_justification_verification::<
				mock::TestRuntime,
			>());
		});
	}
}
