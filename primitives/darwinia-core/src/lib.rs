// This file is part of Darwinia.
//
// Copyright (C) 2018-2022 Darwinia Network
// SPDX-License-Identifier: GPL-3.0
//
// Darwinia is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Darwinia is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Darwinia. If not, see <https://www.gnu.org/licenses/>.

#![cfg_attr(not(feature = "std"), no_std)]

mod copy_paste_from_darwinia {
	// --- paritytech ---
	use frame_support::weights::{
		constants::{BlockExecutionWeight, ExtrinsicBaseWeight, WEIGHT_PER_SECOND},
		DispatchClass, Weight,
	};
	use frame_system::limits::{BlockLength, BlockWeights};
	use sp_core::H256;
	use sp_runtime::{
		generic,
		traits::{BlakeTwo256, IdentifyAccount, Verify},
		MultiAddress, MultiSignature, OpaqueExtrinsic, Perbill,
	};

	pub type BlockNumber = u32;
	pub type Hashing = BlakeTwo256;
	pub type Hash = H256;
	pub type Signature = MultiSignature;
	pub type AccountPublic = <Signature as Verify>::Signer;
	pub type AccountId = <AccountPublic as IdentifyAccount>::AccountId;
	pub type Address = MultiAddress<AccountId, ()>;
	pub type Nonce = u32;
	pub type Balance = u128;
	pub type Header = generic::Header<BlockNumber, Hashing>;
	pub type OpaqueBlock = generic::Block<Header, OpaqueExtrinsic>;

	pub const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_perthousand(25);
	pub const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);
	pub const MAXIMUM_BLOCK_WEIGHT: Weight = 2 * WEIGHT_PER_SECOND;

	frame_support::parameter_types! {
		pub RuntimeBlockLength: BlockLength =
			BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
		pub RuntimeBlockWeights: BlockWeights = BlockWeights::builder()
			.base_block(BlockExecutionWeight::get())
			.for_class(DispatchClass::all(), |weights| {
				weights.base_extrinsic = ExtrinsicBaseWeight::get();
			})
			.for_class(DispatchClass::Normal, |weights| {
				weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
			})
			.for_class(DispatchClass::Operational, |weights| {
				weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
				// Operational transactions have some extra reserved space, so that they
				// are included even if block reached `MAXIMUM_BLOCK_WEIGHT`.
				weights.reserved = Some(
					MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT
				);
			})
			.avg_block_initialization(AVERAGE_ON_INITIALIZE_RATIO)
			.build_or_panic();
	}

	pub const MILLISECS_PER_BLOCK: u64 = 6000;
	pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;

	pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
	pub const HOURS: BlockNumber = 60 * MINUTES;
	pub const DAYS: BlockNumber = 24 * HOURS;

	pub const NANO: Balance = 1;
	pub const MICRO: Balance = 1_000 * NANO;
	pub const MILLI: Balance = 1_000 * MICRO;
	pub const COIN: Balance = 1_000 * MILLI;

	pub const GWEI: Balance = 1_000_000_000;
}
pub use copy_paste_from_darwinia::*;

// --- core ---
use core::{fmt::Debug, marker::PhantomData};
// --- crates.io ---
use parity_scale_codec::{Codec, Compact, Decode, Encode, Error as CodecError, Input};
use scale_info::{StaticTypeInfo, TypeInfo};
// --- paritytech ---
use bp_messages::MessageNonce;
use bp_runtime::{Chain, EncodedOrDecodedCall, TransactionEraOf};
use frame_support::{
	unsigned::{TransactionValidityError, UnknownTransaction},
	weights::{DispatchClass, Weight},
	Blake2_128Concat, StorageHasher, Twox128,
};
use sp_core::H256;
use sp_runtime::{
	generic,
	generic::Era,
	traits::{Convert, DispatchInfoOf, Dispatchable, SignedExtension as SignedExtensionT},
	RuntimeDebug,
};

/// Unchecked Extrinsic type.
pub type UncheckedExtrinsic<Call> = generic::UncheckedExtrinsic<
	Address,
	EncodedOrDecodedCall<Call>,
	Signature,
	SignedExtensions<Call>,
>;

/// Parameters which are part of the payload used to produce transaction signature,
/// but don't end up in the transaction itself (i.e. inherent part of the runtime).
pub type AdditionalSigned = ((), u32, u32, Hash, Hash, (), (), ());

/// A type of the data encoded as part of the transaction.
pub type SignedExtra = ((), (), (), (), Era, Compact<Nonce>, (), Compact<Balance>);

/// Number of extra bytes (excluding size of storage value itself) of storage proof, built at
/// Darwinia-like chain. This mostly depends on number of entries in the storage trie.
/// Some reserve is reserved to account future chain growth.
///
/// To compute this value, we've synced Kusama chain blocks [0; 6545733] to see if there were
/// any significant changes of the storage proof size (NO):
///
/// - at block 3072 the storage proof size overhead was 579 bytes;
/// - at block 2479616 it was 578 bytes;
/// - at block 4118528 it was 711 bytes;
/// - at block 6540800 it was 779 bytes.
///
/// The number of storage entries at the block 6546170 was 351207 and number of trie nodes in
/// the storage proof was 5 (log(16, 351207) ~ 4.6).
///
/// So the assumption is that the storage proof size overhead won't be larger than 1024 in the
/// nearest future. If it'll ever break this barrier, then we'll need to update this constant
/// at next runtime upgrade.
pub const EXTRA_STORAGE_PROOF_SIZE: u32 = 1024;

/// Maximal size (in bytes) of encoded (using `Encode::encode()`) account id.
///
/// All Darwinia-like chains are using same crypto.
pub const MAXIMAL_ENCODED_ACCOUNT_ID_SIZE: u32 = 32;

// TODO [#78] may need to be updated after https://github.com/paritytech/parity-bridges-common/issues/78
/// Maximal number of messages in single delivery transaction.
pub const MAX_MESSAGES_IN_DELIVERY_TRANSACTION: MessageNonce = 128;

/// Maximal number of unrewarded relayer entries at inbound lane.
pub const MAX_UNREWARDED_RELAYERS_IN_CONFIRMATION_TX: MessageNonce = 128;

// TODO [#438] should be selected keeping in mind:
// finality delay on both chains + reward payout cost + messages throughput.
/// Maximal number of unconfirmed messages at inbound lane.
pub const MAX_UNCONFIRMED_MESSAGES_IN_CONFIRMATION_TX: MessageNonce = 8192;

// One important thing about weight-related constants here is that actually we may have
// different weights on different Darwinia-like chains. But now all deployments are
// almost the same, so we're exporting constants from this crate.

/// Maximal weight of single message delivery confirmation transaction on Darwinia-like chain.
///
/// This value is a result of `pallet_bridge_messages::Pallet::receive_messages_delivery_proof`
/// weight formula computation for the case when single message is confirmed. The result then must
/// be rounded up to account possible future runtime upgrades.
pub const MAX_SINGLE_MESSAGE_DELIVERY_CONFIRMATION_TX_WEIGHT: Weight = 2_000_000_000;

/// Increase of delivery transaction weight on Darwinia-like chain with every additional message
/// byte.
///
/// This value is a result of
/// `pallet_bridge_messages::WeightInfoExt::storage_proof_size_overhead(1)` call. The result then
/// must be rounded up to account possible future runtime upgrades.
pub const ADDITIONAL_MESSAGE_BYTE_DELIVERY_WEIGHT: Weight = 25_000;

/// Maximal number of bytes, included in the signed Darwinia-like transaction apart from the encoded
/// call itself.
///
/// Can be computed by subtracting encoded call size from raw transaction size.
pub const TX_EXTRA_BYTES: u32 = 256;

/// Weight of single regular message delivery transaction on Darwinia-like chain.
///
/// This value is a result of `pallet_bridge_messages::Pallet::receive_messages_proof_weight()` call
/// for the case when single message of `pallet_bridge_messages::EXPECTED_DEFAULT_MESSAGE_LENGTH`
/// bytes is delivered. The message must have dispatch weight set to zero. The result then must be
/// rounded up to account possible future runtime upgrades.
pub const DEFAULT_MESSAGE_DELIVERY_TX_WEIGHT: Weight = 1_500_000_000;

/// Weight of pay-dispatch-fee operation for inbound messages at Darwinia-like chain.
///
/// This value corresponds to the result of
/// `pallet_bridge_messages::WeightInfoExt::pay_inbound_dispatch_fee_overhead()` call for your
/// chain. Don't put too much reserve there, because it is used to **decrease**
/// `DEFAULT_MESSAGE_DELIVERY_TX_WEIGHT` cost. So putting large reserve would make delivery
/// transactions cheaper.
pub const PAY_INBOUND_DISPATCH_FEE_WEIGHT: Weight = 600_000_000;

/// A simplified version of signed extensions meant for producing signed transactions
/// and signed payload in the client code.
#[derive(Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub struct SignedExtensions<Call> {
	encode_payload: SignedExtra,
	// It may be set to `None` if extensions are decoded. We are never reconstructing transactions
	// (and it makes no sense to do that) => decoded version of `SignedExtensions` is only used to
	// read fields of `encode_payload`. And when resigning transaction, we're reconstructing
	// `SignedExtensions` from the scratch.
	additional_signed: Option<AdditionalSigned>,
	_data: PhantomData<Call>,
}
impl<Call> SignedExtensions<Call> {
	pub fn new(
		spec_version: u32,
		transaction_version: u32,
		era: TransactionEraOf<DarwiniaLike>,
		genesis_hash: Hash,
		nonce: Nonce,
		tip: Balance,
	) -> Self {
		Self {
			encode_payload: (
				(),              // non-zero sender
				(),              // spec version
				(),              // tx version
				(),              // genesis
				era.frame_era(), // era
				nonce.into(),    // nonce (compact encoding)
				(),              // Check weight
				tip.into(),      // transaction payment / tip (compact encoding)
			),
			additional_signed: Some((
				(),
				spec_version,
				transaction_version,
				genesis_hash,
				era.signed_payload(genesis_hash),
				(),
				(),
				(),
			)),
			_data: Default::default(),
		}
	}

	/// Return signer nonce, used to craft transaction.
	pub fn nonce(&self) -> Nonce {
		self.encode_payload.5.into()
	}

	/// Return transaction tip.
	pub fn tip(&self) -> Balance {
		self.encode_payload.7.into()
	}
}
impl<Call> Encode for SignedExtensions<Call> {
	fn using_encoded<R, F: FnOnce(&[u8]) -> R>(&self, f: F) -> R {
		self.encode_payload.using_encoded(f)
	}
}
impl<Call> Decode for SignedExtensions<Call> {
	fn decode<I: Input>(input: &mut I) -> Result<Self, CodecError> {
		SignedExtra::decode(input).map(|encode_payload| SignedExtensions {
			encode_payload,
			additional_signed: None,
			_data: Default::default(),
		})
	}
}
impl<Call> SignedExtensionT for SignedExtensions<Call>
where
	Call: Debug + Clone + Eq + PartialEq + Sync + Send + Codec + StaticTypeInfo + Dispatchable,
{
	const IDENTIFIER: &'static str = "Not needed.";

	type AccountId = AccountId;
	type Call = Call;
	type AdditionalSigned = AdditionalSigned;
	type Pre = ();

	fn additional_signed(&self) -> Result<Self::AdditionalSigned, TransactionValidityError> {
		// we shall not ever see this error in relay, because we are never signing decoded
		// transactions. Instead we're constructing and signing new transactions. So the error code
		// is kinda random here
		self.additional_signed
			.ok_or(TransactionValidityError::Unknown(UnknownTransaction::Custom(0xFF)))
	}

	fn pre_dispatch(
		self,
		_who: &Self::AccountId,
		_call: &Self::Call,
		_info: &DispatchInfoOf<Self::Call>,
		_len: usize,
	) -> Result<Self::Pre, TransactionValidityError> {
		Ok(())
	}
}

/// Darwinia-like chain.
#[derive(RuntimeDebug)]
pub struct DarwiniaLike;
impl Chain for DarwiniaLike {
	type BlockNumber = BlockNumber;
	type Hash = Hash;
	type Hasher = Hashing;
	type Header = Header;

	type AccountId = AccountId;
	type Balance = Balance;
	type Index = Nonce;
	type Signature = Signature;

	fn max_extrinsic_size() -> u32 {
		*RuntimeBlockLength::get().max.get(DispatchClass::Normal)
	}

	fn max_extrinsic_weight() -> Weight {
		RuntimeBlockWeights::get()
			.get(DispatchClass::Normal)
			.max_extrinsic
			.unwrap_or(Weight::MAX)
	}
}

/// Convert a 256-bit hash into an AccountId.
pub struct AccountIdConverter;
impl Convert<H256, AccountId> for AccountIdConverter {
	fn convert(hash: H256) -> AccountId {
		hash.to_fixed_bytes().into()
	}
}

/// Return a storage key for account data.
///
/// This is based on FRAME storage-generation code from Substrate:
/// [link](https://github.com/paritytech/substrate/blob/c939ceba381b6313462d47334f775e128ea4e95d/frame/support/src/storage/generator/map.rs#L74)
/// The equivalent command to invoke in case full `Runtime` is known is this:
/// `let key = frame_system::Account::<Runtime>::storage_map_final_key(&account_id);`
pub fn account_info_storage_key(id: &AccountId) -> Vec<u8> {
	let module_prefix_hashed = Twox128::hash(b"System");
	let storage_prefix_hashed = Twox128::hash(b"Account");
	let key_hashed = parity_scale_codec::Encode::using_encoded(id, Blake2_128Concat::hash);

	let mut final_key = Vec::with_capacity(
		module_prefix_hashed.len() + storage_prefix_hashed.len() + key_hashed.len(),
	);

	final_key.extend_from_slice(&module_prefix_hashed[..]);
	final_key.extend_from_slice(&storage_prefix_hashed[..]);
	final_key.extend_from_slice(&key_hashed);

	final_key
}
