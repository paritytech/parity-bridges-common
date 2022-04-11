// Copyright 2021 Parity Technologies (UK) Ltd.
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

//! BEEFY bridge pallet.
//!
//! This pallet is an on-chain BEEFY client for Substrate-based chains that are using following
//! pallets bundle: `pallet-mmr`, `pallet-beefy` and `pallet-beefy-mmr`.
//!
//! The pallet is able to verify MMR leaf proofs, so it has a **direct** access to the following
//! data of the bridged chain:
//!
//! - header hashes;
//! - changes of BEEFY authorities;
//! - extra data of MMR leafs (e.g. parachains heads when bridged with relay chain and properly
//!   configured).
//!
//! Given the header hash (and parachain heads), other pallets are able to verify header-based
//! proofs. For example - storage proofs, transaction inclusion proofs, ...There are two options to
//! do that:
//!
//! - the cheap option only works when proof is header-proof is based on some recent header. Then
//!   the submitter may relay on the fact that the pallet is storing hashes of the most recent
//!   bridged headers. Then you may ensure that the provided header is valid by checking that the
//!   `RecentHeaderHashes` map contains an entry for your header.
//! - the expensive option works for any header that is "covered" with MMR. The proof then must
//!   include MMR proof for leaf, corresponding to the header and the header itself.

#![cfg_attr(not(feature = "std"), no_std)]

use bp_beefy::{BeefyMmrProof, ChainWithBeefy, InitializationData};
use frame_system::RawOrigin;
use sp_runtime::traits::BadOrigin;
use sp_std::prelude::*;

// Re-export in crate namespace for `construct_runtime!`
pub use pallet::*;

/// Configured bridged chain.
pub type BridgedChain<T, I> = <T as Config<I>>::BridgedChain;
/// Block number, used by configured bridged chain.
pub type BridgedBlockNumber<T, I> = bp_runtime::BlockNumberOf<BridgedChain<T, I>>;
/// Block hash, used by configured bridged chain.
pub type BridgedBlockHash<T, I> = bp_runtime::HashOf<BridgedChain<T, I>>;

/// Pallet initialization data.
pub type InitializationDataOf<T, I> =
	InitializationData<BridgedBlockNumber<T, I>, bp_beefy::BeefyValidatorIdOf<BridgedChain<T, I>>>;
/// BEEFY commitment hasher, used by configured bridged chain.
pub type BridgedBeefyCommitmentHasher<T, I> = bp_beefy::BeefyCommitmentHasher<BridgedChain<T, I>>;
/// BEEFY validator set, used by configured bridged chain.
pub type BridgedBeefyValidatorSet<T, I> = bp_beefy::BeefyValidatorSetOf<BridgedChain<T, I>>;
/// BEEFY signed commitment, used by configured bridged chain.
pub type BridgedBeefySignedCommitment<T, I> = bp_beefy::BeefySignedCommitmentOf<BridgedChain<T, I>>;
/// MMR hash algorithm, used by configured bridged chain.
pub type BridgedBeefyMmrHasher<T, I> = bp_beefy::BeefyMmrHasherOf<BridgedChain<T, I>>;
/// Unpacked MMR leaf type, used by the pallet.
pub type BridgedBeefyMmrLeafUnpacked<T, I> = bp_beefy::BeefyMmrLeafUnpackedOf<BridgedChain<T, I>>;
/// MMR leaf type, used by configured bridged chain.
pub type BridgedBeefyMmrLeaf<T, I> = bp_beefy::BeefyMmrLeafOf<BridgedChain<T, I>>;
/// A way to encode validator id to the BEEFY merkle tree leaf.
pub type BridgedBeefyValidatorIdToMerkleLeaf<T, I> =
	bp_beefy::BeefyValidatorIdToMerkleLeafOf<BridgedChain<T, I>>;
/// Imported commitment data, stored by the pallet.
pub type ImportedCommitment<T, I> =
	bp_beefy::ImportedCommitment<BridgedBlockNumber<T, I>, BridgedBlockHash<T, I>>;

mod commitment;
mod leaf;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod mock_chain;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config {
		/// The upper bound on the number of requests allowed by the pallet.
		///
		/// A request refers to an action which writes a header to storage.
		///
		/// Once this bound is reached the pallet will reject all commitments
		/// until the request count has decreased.
		#[pallet::constant]
		type MaxRequests: Get<u32>;

		/// Expected MMR leaf version.
		///
		/// The pallet will reject all leafs with mismatching major version.
		#[pallet::constant]
		type ExpectedMmrLeafMajorVersion: Get<u8>;

		/// Maximal number of imported commitments to keep in the storage.
		///
		/// The setting is there to prevent growing the on-chain state indefinitely. Note
		/// the setting does not relate to block numbers - we will simply keep as much items
		/// in the storage, so it doesn't guarantee any fixed timeframe for imported commitments.
		#[pallet::constant]
		type CommitmentsToKeep: Get<u32>;

		/// The chain we are bridging to here.
		type BridgedChain: ChainWithBeefy;
	}

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::hooks]
	impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I> {
		fn on_initialize(_n: T::BlockNumber) -> frame_support::weights::Weight {
			<RequestCount<T, I>>::mutate(|count| *count = count.saturating_sub(1));

			(0_u64)
				.saturating_add(T::DbWeight::get().reads(1))
				.saturating_add(T::DbWeight::get().writes(1))
		}
	}

	#[pallet::call]
	impl<T: Config<I>, I: 'static> Pallet<T, I>
	where
		BridgedBeefyMmrHasher<T, I>: 'static + Send + Sync,
	{
		/// Initialize pallet with BEEFY validator set and best finalized block number.
		#[pallet::weight((T::DbWeight::get().reads_writes(2, 4), DispatchClass::Operational))]
		pub fn initialize(
			origin: OriginFor<T>,
			init_data: InitializationDataOf<T, I>,
		) -> DispatchResult {
			ensure_owner_or_root::<T, I>(origin)?;

			let init_allowed = !<BestBlockNumber<T, I>>::exists();
			ensure!(init_allowed, <Error<T, I>>::AlreadyInitialized);

			log::info!(target: "runtime::bridge-beefy", "Initializing bridge BEEFY pallet: {:?}", init_data);
			Ok(initialize::<T, I>(init_data)?.into())
		}

		/// Halt or resume all pallet operations.
		///
		/// May only be called either by root, or by `PalletOwner`.
		#[pallet::weight((T::DbWeight::get().reads_writes(1, 1), DispatchClass::Operational))]
		pub fn set_operational(origin: OriginFor<T>, operational: bool) -> DispatchResult {
			ensure_owner_or_root::<T, I>(origin)?;
			<IsHalted<T, I>>::put(!operational);

			if operational {
				log::info!(target: "runtime::bridge-beefy", "Resuming pallet operations");
			} else {
				log::warn!(target: "runtime::bridge-beefy", "Stopping pallet operations");
			}

			Ok(().into())
		}

		/// Change `PalletOwner`.
		///
		/// May only be called either by root, or by `PalletOwner`.
		#[pallet::weight((T::DbWeight::get().reads_writes(1, 1), DispatchClass::Operational))]
		pub fn set_owner(
			origin: OriginFor<T>,
			new_owner: Option<T::AccountId>,
		) -> DispatchResultWithPostInfo {
			ensure_owner_or_root::<T, I>(origin)?;
			match new_owner {
				Some(new_owner) => {
					PalletOwner::<T, I>::put(&new_owner);
					log::info!(target: "runtime::bridge-beefy", "Setting pallet Owner to: {:?}", new_owner);
				},
				None => {
					PalletOwner::<T, I>::kill();
					log::info!(target: "runtime::bridge-beefy", "Removed Owner of pallet.");
				},
			}

			Ok(().into())
		}

		/// Submit commitment, generated by BEEFY validators.
		///
		/// Apart from the generic payload, the commitment contains the finalized (by BEEFY) block
		/// number, so they must be always be imported in order. Importing commitment gives us
		/// knowledge of header hash that has been finalized by BEEFY validators.
		#[pallet::weight(0)] // TODO: compute weights
		pub fn submit_commitment(
			origin: OriginFor<T>,
			// TODO: implement `TypeInfo` for `BridgedBeefySignedCommitment<T, I>`, `BeefyMmrProof`
			// and `BridgedBeefyMmrLeafUnpacked::<T, I>`
			encoded_commitment: Vec<u8>,
			encoded_mmr_proof: Vec<u8>,
			mmr_leaf: BridgedBeefyMmrLeafUnpacked<T, I>,
		) -> DispatchResult {
			ensure_operational::<T, I>()?;
			let _ = ensure_signed(origin)?;

			ensure!(Self::request_count() < T::MaxRequests::get(), <Error<T, I>>::TooManyRequests);

			// verify BEEFY commitment: once verification is completed, we know that BEEFY
			// validators have finalized block with given number and given MMR root
			let best_block_number =
				BestBlockNumber::<T, I>::get().ok_or(Error::<T, I>::NotInitialized)?;
			let validators =
				CurrentValidatorSet::<T, I>::get().ok_or(Error::<T, I>::NotInitialized)?;
			let commitment =
				BridgedBeefySignedCommitment::<T, I>::decode(&mut &encoded_commitment[..])
					.map_err(|e| {
						log::error!(
							target: "runtime::bridge-beefy",
							"Signed commitment decode has failed with error: {:?}",
							e,
						);

						Error::<T, I>::FailedToDecodeArgument
					})?;

			log::trace!(
				target: "runtime::bridge-beefy",
				"Importing commitment for block {:?}: {:?}",
				commitment.commitment.block_number,
				commitment,
			);

			let commitment_artifacts = commitment::verify_beefy_signed_commitment::<T, I>(
				best_block_number,
				&validators,
				&commitment,
			)?;

			// MMR proof verification
			let mmr_proof = BeefyMmrProof::decode(&mut &encoded_mmr_proof[..]).map_err(|e| {
				log::error!(
					target: "runtime::bridge-beefy",
					"MMR proof decode has failed with error: {:?}",
					e,
				);

				Error::<T, I>::FailedToDecodeArgument
			})?;
			let mmr_leaf_artifacts = leaf::verify_beefy_mmr_leaf::<T, I>(
				&validators,
				commitment_artifacts.finalized_block_number,
				mmr_leaf,
				mmr_proof,
				commitment_artifacts.mmr_root,
			)?;

			// update storage, essential for pallet operation
			RequestCount::<T, I>::mutate(|count| *count += 1);
			BestBlockNumber::<T, I>::put(commitment.commitment.block_number);
			if let Some(new_next_validator_set) = mmr_leaf_artifacts.next_validator_set {
				let next_validator_set =
					NextValidatorSet::<T, I>::get().ok_or(Error::<T, I>::NotInitialized)?;
				log::info!(
					target: "runtime::bridge-beefy",
					"Enacting new BEEFY validator set #{} with {} validators. Next validator set: #{} with {} validators.",
					next_validator_set.id(),
					next_validator_set.len(),
					new_next_validator_set.id(),
					new_next_validator_set.len(),
				);

				CurrentValidatorSet::<T, I>::put(next_validator_set);
				NextValidatorSet::<T, I>::put(new_next_validator_set);
			}

			// store imported commitment data
			let index = ImportedCommitmentNumbersPointer::<T, I>::get();
			let to_prune = ImportedCommitmentNumbers::<T, I>::try_get(index);
			ImportedCommitments::<T, I>::insert(
				commitment_artifacts.finalized_block_number,
				ImportedCommitment::<T, I> {
					parent_number_and_hash: mmr_leaf_artifacts.parent_number_and_hash,
					mmr_root: commitment_artifacts.mmr_root,
					parachain_heads: mmr_leaf_artifacts.parachain_heads,
				},
			);
			ImportedCommitmentNumbers::<T, I>::insert(
				index,
				commitment_artifacts.finalized_block_number,
			);
			ImportedCommitmentNumbersPointer::<T, I>::put(
				(index + 1) % T::CommitmentsToKeep::get(),
			);
			if let Ok(commitment_number) = to_prune {
				log::debug!(target: "runtime::bridge-beefy", "Pruning old commitment: {:?}.", commitment_number);
				ImportedCommitments::<T, I>::remove(commitment_number);
			}

			log::info!(
				target: "runtime::bridge-beefy",
				"Successfully imported commitment for block {:?}",
				commitment.commitment.block_number,
			);

			Ok(())
		}
	}

	/// The current number of requests which have written to storage.
	///
	/// If the `RequestCount` hits `MaxRequests`, no more calls will be allowed to the pallet until
	/// the request capacity is increased.
	///
	/// The `RequestCount` is decreased by one at the beginning of every block. This is to ensure
	/// that the pallet can always make progress.
	#[pallet::storage]
	#[pallet::getter(fn request_count)]
	pub type RequestCount<T: Config<I>, I: 'static = ()> = StorageValue<_, u32, ValueQuery>;

	/// Best known block number of the bridged chain, finalized by BEEFY.
	#[pallet::storage]
	pub type BestBlockNumber<T: Config<I>, I: 'static = ()> =
		StorageValue<_, BridgedBlockNumber<T, I>>;

	/// All unpruned commitments that we have imported.
	#[pallet::storage]
	pub type ImportedCommitments<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Blake2_128Concat, BridgedBlockNumber<T, I>, ImportedCommitment<T, I>>;

	/// A ring buffer of imported commitment numbers. Ordered by the insertion time
	#[pallet::storage]
	pub(super) type ImportedCommitmentNumbers<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Identity, u32, BridgedBlockNumber<T, I>>;

	/// Current ring buffer position.
	#[pallet::storage]
	pub type ImportedCommitmentNumbersPointer<T: Config<I>, I: 'static = ()> =
		StorageValue<_, u32, ValueQuery>;

	/// Current BEEFY validators set at the bridged chain.
	#[pallet::storage]
	pub type CurrentValidatorSet<T: Config<I>, I: 'static = ()> =
		StorageValue<_, BridgedBeefyValidatorSet<T, I>>;

	/// Next BEEFY validators set at the bridged chain.
	#[pallet::storage]
	pub type NextValidatorSet<T: Config<I>, I: 'static = ()> =
		StorageValue<_, BridgedBeefyValidatorSet<T, I>>;

	/// Optional pallet owner.
	///
	/// Pallet owner has a right to halt all pallet operations and then resume it. If it is
	/// `None`, then there are no direct ways to halt/resume pallet operations, but other
	/// runtime methods may still be used to do that (i.e. democracy::referendum to update halt
	/// flag directly or call the `halt_operations`).
	#[pallet::storage]
	pub type PalletOwner<T: Config<I>, I: 'static = ()> =
		StorageValue<_, T::AccountId, OptionQuery>;

	/// If true, all pallet transactions (except `set_operational`) are failed immediately.
	#[pallet::storage]
	pub(super) type IsHalted<T: Config<I>, I: 'static = ()> = StorageValue<_, bool, ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config<I>, I: 'static = ()> {
		/// Optional module owner account.
		pub owner: Option<T::AccountId>,
		/// Optional module initialization data.
		pub init_data: Option<InitializationDataOf<T, I>>,
	}

	#[cfg(feature = "std")]
	impl<T: Config<I>, I: 'static> Default for GenesisConfig<T, I> {
		fn default() -> Self {
			Self { owner: None, init_data: None }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config<I>, I: 'static> GenesisBuild<T, I> for GenesisConfig<T, I> {
		fn build(&self) {
			if let Some(ref owner) = self.owner {
				<PalletOwner<T, I>>::put(owner);
			}

			if let Some(init_data) = self.init_data.clone() {
				initialize::<T, I>(init_data)
					.expect("invalid initialization data of BEEFY bridge pallet");
			} else {
				// Since the bridge hasn't been initialized we shouldn't allow anyone to perform
				// transactions.
				<IsHalted<T, I>>::put(true);
			}
		}
	}

	#[pallet::error]
	pub enum Error<T, I = ()> {
		/// The pallet has already been initialized.
		AlreadyInitialized,
		/// Invalid initial current validator set.
		InvalidCurrentValidatorSet,
		/// Invalid initial next validator set.
		InvalidNextValidatorSet,
		/// All pallet operations are halted.
		Halted,
		/// There are too many requests for the current window to handle.
		TooManyRequests,
		/// Failed to decode method arguments (will be removed once `TypeInfo` will be
		/// implemented for all arguments).
		FailedToDecodeArgument,
		/// The pallet has not been initialized yet.
		NotInitialized,
		/// The commitment being imported is older than the best commitment known to the pallet.
		OldCommitment,
		/// The commitment is signed by unknown validator set.
		InvalidValidatorSetId,
		/// The number of signatures in the commitment is invalid.
		InvalidSignaturesLength,
		/// There are not enough correct signatures in commitment to finalize block.
		NotEnoughCorrectSignatures,
		/// MMR root is missing from the commitment.
		MmrRootMissingFromCommitment,
		/// Failed to decode MMR leaf version.
		FailedToDecodeMmrLeafVersion,
		/// The leaf has unsupported version.
		UnsupportedMmrLeafVersion,
		/// Failed to decode MMR leaf version.
		FailedToDecodeMmrLeaf,
		/// Parent header number and hash field of the MMR leaf is invalid.
		InvalidParentNumberAndHash,
		/// MMR proof verification has failed.
		MmrProofVeriricationFailed,
		/// Next validator set id is invalid.
		InvalidNextValidatorsSetId,
		/// Next validators are provided when leaf is not signalling set change.
		RedundantNextValidatorsProvided,
		/// Next validators are not provided when leaf is signalling set change.
		NextValidatorsAreNotProvided,
		/// Next validators are not matching the merkle tree root.
		InvalidNextValidatorSetRoot,
		/// Next validator set is empty.
		EmptyNextValidatorSet,
	}

	/// Initialize pallet with given parameters.
	pub(super) fn initialize<T: Config<I>, I: 'static>(
		init_data: InitializationDataOf<T, I>,
	) -> Result<(), Error<T, I>> {
		let current_set = BridgedBeefyValidatorSet::<T, I>::new(
			init_data.current_validator_set.1,
			init_data.current_validator_set.0,
		)
		.ok_or(Error::<T, I>::InvalidCurrentValidatorSet)?;
		let next_set = BridgedBeefyValidatorSet::<T, I>::new(
			init_data.next_validator_set.1,
			init_data.next_validator_set.0,
		)
		.ok_or(Error::<T, I>::InvalidNextValidatorSet)?;

		IsHalted::<T, I>::put(init_data.is_halted);
		BestBlockNumber::<T, I>::put(init_data.best_beefy_block_number);
		CurrentValidatorSet::<T, I>::put(current_set);
		NextValidatorSet::<T, I>::put(next_set);

		Ok(())
	}

	/// Ensure that the origin is either root, or `PalletOwner`.
	fn ensure_owner_or_root<T: Config<I>, I: 'static>(origin: T::Origin) -> Result<(), BadOrigin> {
		match origin.into() {
			Ok(RawOrigin::Root) => Ok(()),
			Ok(RawOrigin::Signed(ref signer))
				if Some(signer) == <PalletOwner<T, I>>::get().as_ref() =>
				Ok(()),
			_ => Err(BadOrigin),
		}
	}

	/// Ensure that the pallet is in operational mode (not halted).
	fn ensure_operational<T: Config<I>, I: 'static>() -> Result<(), Error<T, I>> {
		if <IsHalted<T, I>>::get() {
			Err(<Error<T, I>>::Halted)
		} else {
			Ok(())
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::{assert_noop, assert_ok, traits::Get};
	use mock::*;
	use mock_chain::*;

	fn next_block() {
		use frame_support::traits::OnInitialize;

		let current_number = frame_system::Pallet::<TestRuntime>::block_number();
		frame_system::Pallet::<TestRuntime>::set_block_number(current_number + 1);
		let _ = Pallet::<TestRuntime>::on_initialize(current_number);
	}

	fn import_header_chain(headers: Vec<HeaderAndCommitment>) {
		for header in headers {
			if header.commitment.is_some() {
				assert_ok!(import_commitment(header));
			}
		}
	}

	#[test]
	fn fails_to_initialize_if_not_owner_and_root() {
		run_test(|| {
			PalletOwner::<TestRuntime, ()>::put(1);
			assert_noop!(
				Pallet::<TestRuntime>::initialize(
					Origin::signed(10),
					InitializationData {
						is_halted: false,
						best_beefy_block_number: 0,
						current_validator_set: (0, validator_ids(0, 1)),
						next_validator_set: (1, validator_ids(0, 1)),
					}
				),
				BadOrigin,
			);
		})
	}

	#[test]
	fn root_is_able_to_initialize_pallet() {
		run_test(|| {
			assert_ok!(Pallet::<TestRuntime>::initialize(
				Origin::root(),
				InitializationData {
					is_halted: false,
					best_beefy_block_number: 0,
					current_validator_set: (0, validator_ids(0, 1)),
					next_validator_set: (1, validator_ids(0, 1)),
				}
			));

			assert_eq!(BestBlockNumber::<TestRuntime>::get(), Some(0));
		});
	}

	#[test]
	fn owner_is_able_to_initialize_pallet() {
		run_test(|| {
			PalletOwner::<TestRuntime>::put(10);
			assert_ok!(Pallet::<TestRuntime>::initialize(
				Origin::signed(10),
				InitializationData {
					is_halted: false,
					best_beefy_block_number: 0,
					current_validator_set: (0, validator_ids(0, 1)),
					next_validator_set: (1, validator_ids(0, 1)),
				}
			));

			assert_eq!(BestBlockNumber::<TestRuntime>::get(), Some(0));
		});
	}

	#[test]
	fn fails_to_initialize_if_already_initialized() {
		run_test_with_initialize(32, || {
			assert_noop!(
				Pallet::<TestRuntime>::initialize(
					Origin::root(),
					InitializationData {
						is_halted: false,
						best_beefy_block_number: 0,
						current_validator_set: (0, validator_ids(0, 1)),
						next_validator_set: (1, validator_ids(0, 1)),
					}
				),
				Error::<TestRuntime, ()>::AlreadyInitialized,
			);
		});
	}

	#[test]
	fn fails_to_initialize_if_current_set_is_empty() {
		run_test(|| {
			assert_noop!(
				Pallet::<TestRuntime>::initialize(
					Origin::root(),
					InitializationData {
						is_halted: false,
						best_beefy_block_number: 0,
						current_validator_set: (0, Vec::new()),
						next_validator_set: (1, validator_ids(0, 1)),
					}
				),
				Error::<TestRuntime, ()>::InvalidCurrentValidatorSet,
			);
		});
	}

	#[test]
	fn fails_to_initialize_if_next_set_is_empty() {
		run_test(|| {
			assert_noop!(
				Pallet::<TestRuntime>::initialize(
					Origin::root(),
					InitializationData {
						is_halted: false,
						best_beefy_block_number: 0,
						current_validator_set: (0, validator_ids(0, 1)),
						next_validator_set: (1, Vec::new()),
					}
				),
				Error::<TestRuntime, ()>::InvalidNextValidatorSet,
			);
		});
	}

	#[test]
	fn fails_to_change_operation_mode_if_not_owner_and_root() {
		run_test_with_initialize(1, || {
			assert_noop!(
				Pallet::<TestRuntime>::set_operational(Origin::signed(10), false),
				BadOrigin,
			);
		});
	}

	#[test]
	fn root_is_able_to_change_operation_mode() {
		run_test_with_initialize(1, || {
			assert_ok!(Pallet::<TestRuntime>::set_operational(Origin::root(), false));
			assert_eq!(IsHalted::<TestRuntime>::get(), true);

			assert_ok!(Pallet::<TestRuntime>::set_operational(Origin::root(), true));
			assert_eq!(IsHalted::<TestRuntime>::get(), false);
		});
	}

	#[test]
	fn owner_is_able_to_change_operation_mode() {
		run_test_with_initialize(1, || {
			PalletOwner::<TestRuntime>::put(10);

			assert_ok!(Pallet::<TestRuntime>::set_operational(Origin::signed(10), false));
			assert_eq!(IsHalted::<TestRuntime>::get(), true);

			assert_ok!(Pallet::<TestRuntime>::set_operational(Origin::signed(10), true));
			assert_eq!(IsHalted::<TestRuntime>::get(), false);
		});
	}

	#[test]
	fn fails_to_set_owner_if_not_owner_and_root() {
		run_test_with_initialize(1, || {
			assert_noop!(Pallet::<TestRuntime>::set_owner(Origin::signed(10), Some(42)), BadOrigin,);
		});
	}

	#[test]
	fn root_is_able_to_set_owner() {
		run_test_with_initialize(1, || {
			assert_ok!(Pallet::<TestRuntime>::set_owner(Origin::root(), Some(42)));
			assert_eq!(PalletOwner::<TestRuntime>::get(), Some(42));

			assert_ok!(Pallet::<TestRuntime>::set_owner(Origin::root(), None));
			assert_eq!(PalletOwner::<TestRuntime>::get(), None);
		});
	}

	#[test]
	fn owner_is_able_to_set_owner() {
		run_test_with_initialize(1, || {
			PalletOwner::<TestRuntime>::put(10);

			assert_ok!(Pallet::<TestRuntime>::set_owner(Origin::signed(10), Some(42)));
			assert_eq!(PalletOwner::<TestRuntime>::get(), Some(42));

			assert_ok!(Pallet::<TestRuntime>::set_owner(Origin::signed(42), None));
			assert_eq!(PalletOwner::<TestRuntime>::get(), None);
		});
	}

	#[test]
	fn fails_to_import_commitment_if_halted() {
		run_test_with_initialize(1, || {
			assert_ok!(Pallet::<TestRuntime>::set_operational(Origin::root(), false));
			assert_noop!(
				import_commitment(ChainBuilder::new(1).append_finalized_header().to_header()),
				Error::<TestRuntime, ()>::Halted,
			);
		})
	}

	#[test]
	fn fails_to_import_commitment_if_too_many_requests() {
		run_test_with_initialize(1, || {
			let max_requests = <<TestRuntime as Config>::MaxRequests as Get<u32>>::get() as u64;
			let mut chain = ChainBuilder::new(1);
			for _ in 0..max_requests + 2 {
				chain = chain.append_finalized_header();
			}

			// import `max_request` headers
			for i in 0..max_requests {
				assert_ok!(import_commitment(chain.header(i + 1)));
			}

			// try to import next header: it fails because we are no longer accepting commitments
			assert_noop!(
				import_commitment(chain.header(max_requests + 1)),
				Error::<TestRuntime, ()>::TooManyRequests,
			);

			// when next block is "started", we allow import of next header
			next_block();
			assert_ok!(import_commitment(chain.header(max_requests + 1)));

			// but we can't import two headers until next block and so on
			assert_noop!(
				import_commitment(chain.header(max_requests + 2)),
				Error::<TestRuntime, ()>::TooManyRequests,
			);
		})
	}

	#[test]
	fn fails_to_import_commitment_if_not_initialized() {
		run_test(|| {
			assert_noop!(
				import_commitment(ChainBuilder::new(1).append_finalized_header().to_header()),
				Error::<TestRuntime, ()>::NotInitialized,
			);
		})
	}

	#[test]
	fn submit_commitment_works_with_long_chain_with_handoffs() {
		run_test_with_initialize(3, || {
			let chain = ChainBuilder::new(3)
				.append_finalized_header() // 1
				.append_default_headers(16) // 2..17
				.append_finalized_header() // 18
				.append_default_headers(16) // 19..34
				.append_handoff_header(9) // 35
				.append_default_headers(8) // 36..43
				.append_finalized_header() // 44
				.append_default_headers(8) // 45..52
				.append_handoff_header(17) // 53
				.append_default_headers(4) // 54..57
				.append_finalized_header() // 58
				.append_default_headers(4); // 59..63
			import_header_chain(chain.to_chain());

			assert_eq!(BestBlockNumber::<TestRuntime>::get().unwrap(), 58);
			assert_eq!(CurrentValidatorSet::<TestRuntime>::get().unwrap().id(), 2);
			assert_eq!(CurrentValidatorSet::<TestRuntime>::get().unwrap().len(), 9);
			assert_eq!(NextValidatorSet::<TestRuntime>::get().unwrap().id(), 3);
			assert_eq!(NextValidatorSet::<TestRuntime>::get().unwrap().len(), 17);

			let imported_commitment = ImportedCommitments::<TestRuntime>::get(58).unwrap();
			assert_eq!(
				imported_commitment,
				bp_beefy::ImportedCommitment {
					parent_number_and_hash: (57, chain.header(57).header.hash()),
					mmr_root: chain.header(58).mmr_root,
					parachain_heads: parachain_heads(&chain.header(58).header),
				},
			);
		})
	}

	#[test]
	fn commitment_pruning_works() {
		run_test_with_initialize(3, || {
			let commitments_to_keep = <TestRuntime as Config<()>>::CommitmentsToKeep::get();
			let commitments_to_import: Vec<HeaderAndCommitment> = ChainBuilder::new(3)
				.append_finalized_headers(commitments_to_keep as usize + 2)
				.to_chain();

			// import exactly `CommitmentsToKeep` commitments
			for index in 0..commitments_to_keep {
				next_block();
				import_commitment(commitments_to_import[index as usize].clone())
					.expect("must succeed");
				assert_eq!(
					ImportedCommitmentNumbersPointer::<TestRuntime>::get(),
					(index + 1) % commitments_to_keep
				);
			}

			// ensure that all commitments are in the storage
			assert_eq!(
				BestBlockNumber::<TestRuntime>::get().unwrap(),
				commitments_to_keep as mock::BridgedBlockNumber
			);
			assert_eq!(ImportedCommitmentNumbersPointer::<TestRuntime>::get(), 0);
			for index in 0..commitments_to_keep {
				assert!(ImportedCommitments::<TestRuntime>::get(
					index as mock::BridgedBlockNumber + 1
				)
				.is_some());
				assert_eq!(
					ImportedCommitmentNumbers::<TestRuntime>::get(index),
					Some(index + 1).map(Into::into)
				);
			}

			// import next commitment
			next_block();
			import_commitment(commitments_to_import[commitments_to_keep as usize].clone())
				.expect("must succeed");
			assert_eq!(ImportedCommitmentNumbersPointer::<TestRuntime>::get(), 1);
			assert!(ImportedCommitments::<TestRuntime>::get(
				commitments_to_keep as mock::BridgedBlockNumber + 1
			)
			.is_some());
			assert_eq!(
				ImportedCommitmentNumbers::<TestRuntime>::get(0),
				Some(commitments_to_keep + 1).map(Into::into)
			);

			// the side effect of the import is that the commitment#1 is pruned
			assert!(ImportedCommitments::<TestRuntime>::get(1).is_none());

			// import next commitment
			next_block();
			import_commitment(commitments_to_import[commitments_to_keep as usize + 1].clone())
				.expect("must succeed");
			assert_eq!(ImportedCommitmentNumbersPointer::<TestRuntime>::get(), 2);
			assert!(ImportedCommitments::<TestRuntime>::get(
				commitments_to_keep as mock::BridgedBlockNumber + 2
			)
			.is_some());
			assert_eq!(
				ImportedCommitmentNumbers::<TestRuntime>::get(1),
				Some(commitments_to_keep + 2).map(Into::into)
			);

			// the side effect of the import is that the commitment#2 is pruned
			assert!(ImportedCommitments::<TestRuntime>::get(1).is_none());
			assert!(ImportedCommitments::<TestRuntime>::get(2).is_none());
		});
	}
}
