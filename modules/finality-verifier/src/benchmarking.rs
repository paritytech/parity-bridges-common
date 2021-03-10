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

use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;
use sp_std::vec;

pub trait Config: crate::Config {
	// We need some way for the benchmarks to use headers in a "generic" way. However, since we do use a
	// real runtime we need a way for the runtime to tell us what the concrete type is.
	fn bridged_header() -> BridgedHeader<Self>;
}

benchmarks! {
	submit_finality_proof {
		let n in 1..100;

	}: _(RawOrigin::Root, T::bridged_header(), vec![])
	verify {
		assert!(true)
	}
}

#[test]
fn test_benchmarks() {
	run_test(|| {
		// assert_ok!(test_benchmark_it_works::<Test>());
	});
}
