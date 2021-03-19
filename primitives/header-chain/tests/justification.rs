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

//! Tests for Grandpa Justification code.

use bp_header_chain::justification::{verify_justification, Error};
use bp_test_utils::Keyring::*;
use bp_test_utils::*;
use codec::Encode;

type TestHeader = sp_runtime::testing::Header;

#[test]
fn valid_justification_accepted() {
	let depth = 5;
	let forks = 5;

	assert_eq!(
		verify_justification::<TestHeader>(
			header_id::<TestHeader>(1),
			TEST_GRANDPA_SET_ID,
			&voter_set(),
			&make_justification_for_header::<TestHeader>(
				&test_header(1),
				TEST_GRANDPA_ROUND,
				TEST_GRANDPA_SET_ID,
				&[(Alice, 1), (Bob, 1), (Charlie, 1), (Dave, 1), (Eve, 1)],
				depth,
				forks,
			)
			.encode()
		),
		Ok(()),
	);
}

#[test]
fn valid_justification_accepted_with_single_fork() {
	let depth = 5;
	let forks = 1;

	assert_eq!(
		verify_justification::<TestHeader>(
			header_id::<TestHeader>(1),
			TEST_GRANDPA_SET_ID,
			&voter_set(),
			&make_justification_for_header::<TestHeader>(
				&test_header(1),
				TEST_GRANDPA_ROUND,
				TEST_GRANDPA_SET_ID,
				&[(Alice, 1), (Bob, 1), (Charlie, 1), (Dave, 1), (Eve, 1)],
				depth,
				forks,
			)
			.encode()
		),
		Ok(()),
	);
}

#[test]
fn justification_with_invalid_encoding_rejected() {
	assert_eq!(
		verify_justification::<TestHeader>(header_id::<TestHeader>(1), TEST_GRANDPA_SET_ID, &voter_set(), &[],),
		Err(Error::JustificationDecode),
	);
}

#[test]
fn justification_with_invalid_target_rejected() {
	assert_eq!(
		verify_justification::<TestHeader>(
			header_id::<TestHeader>(2),
			TEST_GRANDPA_SET_ID,
			&voter_set(),
			&make_default_justification::<TestHeader>(&test_header(1)).encode(),
		),
		Err(Error::InvalidJustificationTarget),
	);
}

#[test]
fn justification_with_invalid_commit_rejected() {
	let mut justification = make_default_justification::<TestHeader>(&test_header(1));
	justification.commit.precommits.clear();

	assert_eq!(
		verify_justification::<TestHeader>(
			header_id::<TestHeader>(1),
			TEST_GRANDPA_SET_ID,
			&voter_set(),
			&justification.encode(),
		),
		Err(Error::InvalidJustificationCommit),
	);
}

#[test]
fn justification_with_invalid_authority_signature_rejected() {
	let mut justification = make_default_justification::<TestHeader>(&test_header(1));
	justification.commit.precommits[0].signature = Default::default();

	assert_eq!(
		verify_justification::<TestHeader>(
			header_id::<TestHeader>(1),
			TEST_GRANDPA_SET_ID,
			&voter_set(),
			&justification.encode(),
		),
		Err(Error::InvalidAuthoritySignature),
	);
}

#[test]
fn justification_with_invalid_precommit_ancestry() {
	let mut justification = make_default_justification::<TestHeader>(&test_header(1));
	justification.votes_ancestries.push(test_header(10));

	assert_eq!(
		verify_justification::<TestHeader>(
			header_id::<TestHeader>(1),
			TEST_GRANDPA_SET_ID,
			&voter_set(),
			&justification.encode(),
		),
		Err(Error::InvalidPrecommitAncestries),
	);
}

#[test]
fn justification_is_invalid_if_we_dont_meet_threshold() {
	let depth = 2;
	let forks = 2;

	// Need at least three authorities to sign off or else the voter set threshold can't be reached
	let authorities = [(Alice, 1), (Bob, 1)];

	assert_eq!(
		verify_justification::<TestHeader>(
			header_id::<TestHeader>(1),
			TEST_GRANDPA_SET_ID,
			&voter_set(),
			&make_justification_for_header::<TestHeader>(
				&test_header(1),
				TEST_GRANDPA_ROUND,
				TEST_GRANDPA_SET_ID,
				&authorities,
				depth,
				forks,
			)
			.encode()
		),
		Err(Error::InvalidJustificationCommit),
	);
}
