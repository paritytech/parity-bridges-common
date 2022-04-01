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
	BridgedBeefyCommitmentHasher, BridgedBeefyMmrHasher, BridgedBeefyMmrLeaf,
	BridgedBeefySignedCommitment, BridgedBeefyValidatorIdToMerkleLeaf, BridgedBeefyValidatorSet,
};

use bp_beefy::{BeefyMmrHash, ChainWithBeefy, Commitment, MmrDataOrHash, SignedCommitment};
use bp_runtime::Chain;
use codec::Encode;
use frame_support::{construct_runtime, parameter_types, weights::Weight};
use libsecp256k1::{sign, Message, PublicKey, SecretKey};
use sp_core::sr25519::Signature;
use sp_runtime::{
	testing::{Header, H256},
	traits::{BlakeTwo256, Hash, IdentityLookup},
	Perbill,
};
use std::{collections::BTreeSet, marker::PhantomData};

pub use beefy_primitives::crypto::AuthorityId as BeefyId;

pub type AccountId = u64;
pub type BridgedBlockNumber = u64;
pub type BridgedBlockHash = H256;
pub type BridgedHeader = Header;
pub type BridgedCommitment = BridgedBeefySignedCommitment<TestRuntime, ()>;
pub type BridgedValidatorSet = BridgedBeefyValidatorSet<TestRuntime, ()>;
pub type BridgedCommitmentHasher = BridgedBeefyCommitmentHasher<TestRuntime, ()>;
pub type BridgedMmrHasher = BridgedBeefyMmrHasher<TestRuntime, ()>;
pub type BridgedMmrLeaf = BridgedBeefyMmrLeaf<TestRuntime, ()>;
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

impl beefy::Config for TestRuntime {
	type BridgedChain = TestBridgedChain;
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
