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

use crate as beefy;
use crate::{
	BridgedBeefyCommitmentHasher, BridgedBeefyMmrHasher, BridgedBeefyMmrLeafUnpacked,
	BridgedBeefySignedCommitment, BridgedBeefyValidatorIdToMerkleLeaf,
};

use bp_beefy::{BeefyMmrHash, ChainWithBeefy, Commitment, MmrDataOrHash};
use bp_runtime::Chain;
use codec::Encode;
use frame_support::{construct_runtime, parameter_types, weights::Weight};
use libsecp256k1::{sign, Message, PublicKey, SecretKey};
use sp_core::sr25519::Signature;
use sp_runtime::{
	testing::{Header, H256},
	traits::{BlakeTwo256, Hash, Header as HeaderT, IdentityLookup},
	Perbill,
};
use std::collections::BTreeSet;

pub use beefy_primitives::crypto::AuthorityId as BeefyId;

pub type AccountId = u64;
pub type BridgedBlockNumber = u64;
pub type BridgedBlockHash = H256;
pub type BridgedHeader = Header;
pub type BridgedCommitment = BridgedBeefySignedCommitment<TestRuntime, ()>;
pub type BridgedCommitmentHasher = BridgedBeefyCommitmentHasher<TestRuntime, ()>;
pub type BridgedMmrHasher = BridgedBeefyMmrHasher<TestRuntime, ()>;
pub type BridgedMmrLeaf = BridgedBeefyMmrLeafUnpacked<TestRuntime, ()>;
pub type BridgedRawMmrLeaf =
	beefy_primitives::mmr::MmrLeaf<BridgedBlockNumber, BridgedBlockHash, BeefyMmrHash>;
pub type BridgedMmrNode = MmrDataOrHash<sp_runtime::traits::Keccak256, BridgedRawMmrLeaf>;
pub type BridgedValidatorIdToMerkleLeaf = BridgedBeefyValidatorIdToMerkleLeaf<TestRuntime, ()>;

type Block = frame_system::mocking::MockBlock<TestRuntime>;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;

construct_runtime! {
	pub enum TestRuntime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Beefy: beefy::{Pallet},
	}
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: Weight = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::one();
}

impl frame_system::Config for TestRuntime {
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
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type BaseCallFilter = frame_support::traits::Everything;
	type SystemWeightInfo = ();
	type DbWeight = ();
	type BlockWeights = ();
	type BlockLength = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

pub const EXPECTED_MMR_LEAF_MAJOR_VERSION: u8 = 3;

impl beefy::Config for TestRuntime {
	type MaxRequests = frame_support::traits::ConstU32<16>;
	type BridgedChain = TestBridgedChain;
	type ExpectedMmrLeafMajorVersion =
		frame_support::traits::ConstU8<EXPECTED_MMR_LEAF_MAJOR_VERSION>;
	type CommitmentsToKeep = frame_support::traits::ConstU32<16>;
}

#[derive(Debug)]
pub struct TestBridgedChain;

impl Chain for TestBridgedChain {
	type BlockNumber = BridgedBlockNumber;
	type Hash = H256;
	type Hasher = BlakeTwo256;
	type Header = <TestRuntime as frame_system::Config>::Header;

	type AccountId = AccountId;
	type Balance = u64;
	type Index = u64;
	type Signature = Signature;

	fn max_extrinsic_size() -> u32 {
		unreachable!()
	}
	fn max_extrinsic_weight() -> Weight {
		unreachable!()
	}
}

impl ChainWithBeefy for TestBridgedChain {
	type CommitmentHasher = sp_runtime::traits::Keccak256;
	type MmrHasher = beefy_merkle_tree::Keccak256;
	type ValidatorId = BeefyId;
	type ValidatorIdToMerkleLeaf = pallet_beefy_mmr::BeefyEcdsaToEthereum;
}

/// Run test within test runtime.
pub fn run_test<T>(test: impl FnOnce() -> T) -> T {
	sp_io::TestExternalities::new(Default::default()).execute_with(test)
}

/// Initialize pallet and run test.
pub fn run_test_with_initialize<T>(initial_validators_count: usize, test: impl FnOnce() -> T) -> T {
	run_test(|| {
		crate::Pallet::<TestRuntime>::initialize(
			Origin::root(),
			bp_beefy::InitializationData {
				is_halted: false,
				best_beefy_block_number: 0,
				current_validator_set: (0, validator_ids(0, initial_validators_count)),
				next_validator_set: (1, validator_ids(0, initial_validators_count)),
			},
		)
		.expect("initialization data is correct");

		test()
	})
}

/// Import given commitment.
pub fn import_commitment(
	header: crate::mock_chain::HeaderAndCommitment,
) -> sp_runtime::DispatchResult {
	crate::Pallet::<TestRuntime>::submit_commitment(
		Origin::signed(1),
		header
			.commitment
			.expect("thou shall not call import_commitment on header without commitment")
			.encode(),
		header.leaf_proof.encode(),
		header.leaf,
	)
}

/// Return secret of validator with given index.
pub fn validator_key(index: usize) -> SecretKey {
	let mut raw_secret = [1u8; 32];
	raw_secret[0..8].copy_from_slice(&(index as u64).encode());
	SecretKey::parse(&raw_secret).expect("only zero key is invalid; qed")
}

/// Convert validator secret to public.
pub fn validator_key_to_public(key: SecretKey) -> PublicKey {
	PublicKey::from_secret_key(&key)
}

/// Return secrets of validators, starting at given index.
pub fn validator_keys(index: usize, size: usize) -> Vec<SecretKey> {
	(index..index + size).map(validator_key).collect()
}

/// Return identifiers of validators, starting at given index.
pub fn validator_ids(index: usize, size: usize) -> Vec<BeefyId> {
	validator_keys(index, size)
		.into_iter()
		.map(|k| {
			sp_core::ecdsa::Public::from_raw(validator_key_to_public(k).serialize_compressed())
				.into()
		})
		.collect()
}

/// Sign BEEFY commitment.
pub fn sign_commitment(
	commitment: Commitment<BridgedBlockNumber>,
	validator_keys: &[SecretKey],
) -> BridgedCommitment {
	let total_validators = validator_keys.len();
	let signatures_required = crate::commitment::signatures_required(total_validators);
	let random_validators =
		rand::seq::index::sample(&mut rand::thread_rng(), total_validators, signatures_required)
			.into_iter()
			.collect::<BTreeSet<_>>();

	let commitment_hash =
		Message::parse(BridgedCommitmentHasher::hash(&commitment.encode()).as_fixed_bytes());
	let mut signatures = vec![None; total_validators];
	for validator in 0..total_validators {
		if !random_validators.contains(&validator) {
			continue
		}

		let validator_key = &validator_keys[validator];
		let (signature, recovery_id) = sign(&commitment_hash, validator_key);
		let mut raw_signature_with_recovery = [recovery_id.serialize(); 65];
		raw_signature_with_recovery[..64].copy_from_slice(&signature.serialize());
		log::trace!(
			target: "runtime::bridge-beefy",
			"Validator {} ({:?}) has signed commitment hash ({:?}): {:?}",
			validator,
			hex::encode(validator_key_to_public(validator_key.clone()).serialize_compressed()),
			hex::encode(commitment_hash.serialize()),
			hex::encode(signature.serialize()),
		);
		signatures[validator] =
			Some(sp_core::ecdsa::Signature::from_raw(raw_signature_with_recovery).into());
	}

	BridgedCommitment { commitment, signatures }
}

/// Returns dummy parachain heads for given header.
pub fn parachain_heads(header: &BridgedHeader) -> BeefyMmrHash {
	bp_beefy::beefy_merkle_root::<BridgedMmrHasher, _, _>(vec![
		header.number().encode(),
		header.hash().encode(),
	])
}
