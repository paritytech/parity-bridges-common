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
//! proofs. For example - storage proofs, transaction inclustion proofs, ...There are two options to
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
/// MMR leaf type, used by configured bridged chain.
pub type BridgedBeefyMmrLeaf<T, I> = bp_beefy::BeefyMmrLeafOf<BridgedChain<T, I>>;
/// TODO
pub type BridgedRawBeefyMmrLeaf<T, I> = bp_beefy::RawBeefyMmrLeafOf<BridgedChain<T, I>>;
/// A way to encode validator id to the BEEFY merkle tree leaf.
pub type BridgedBeefyValidatorIdToMerkleLeaf<T, I> =
	bp_beefy::BeefyValidatorIdToMerkleLeafOf<BridgedChain<T, I>>;

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
		#[pallet::weight(0)] // TODO: compute weights
		pub fn initialize(
			origin: OriginFor<T>,
			init_data: InitializationDataOf<T, I>,
		) -> DispatchResultWithPostInfo {
			ensure_owner_or_root::<T, I>(origin)?;

			let init_allowed = !<BestBlockNumber<T, I>>::exists();
			ensure!(init_allowed, <Error<T, I>>::AlreadyInitialized);

			log::info!(target: "runtime::bridge-beefy", "Initializing bridge BEEFY pallet: {:?}", init_data);
			Ok(initialize::<T, I>(init_data)?.into())
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
			// and `BridgedBeefyMmrLeaf::<T, I>`
			encoded_commitment: Vec<u8>,
			encoded_mmr_proof: Vec<u8>,
			encoded_mmr_leaf: Vec<u8>,
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
				"Importing commitment for block {:?}",
				commitment.commitment.block_number,
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
			let mmr_leaf = BridgedBeefyMmrLeaf::<T, I>::decode(&mut &encoded_mmr_leaf[..])
				.map_err(|e| {
					log::error!(
						target: "runtime::bridge-beefy",
						"MMR leaf decode has failed with error: {:?}",
						e,
					);

					Error::<T, I>::FailedToDecodeArgument
				})?;
			let mmr_leaf_artifacts = leaf::verify_beefy_mmr_leaf::<T, I>(
				&validators,
				&mmr_leaf,
				mmr_proof,
				commitment_artifacts.mmr_root,
			)?;

			// update storage
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
			// TODO: store parent header number => hash to eb able to verify header-based proofs
			// TODO: store MMR root + parachain heads root for verifying later proofs

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
	pub(super) type RequestCount<T: Config<I>, I: 'static = ()> = StorageValue<_, u32, ValueQuery>;

	/// Best known block number of the bridged chain, finalized by BEEFY.
	#[pallet::storage]
	pub type BestBlockNumber<T: Config<I>, I: 'static = ()> =
		StorageValue<_, BridgedBlockNumber<T, I>>;

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
		/// Failed to decode method arguments (will be removed once `TypeInfo` wiwill be
		/// implemented for all arguments).
		FailedToDecodeArgument,
		/// The pallet has not been initialized yet.
		NotInitialized,
		/// The commitment being imported is older than the best commitment known to the pallet.
		OldCommitment,
		/// The commitment is signed by unknown validator set.
		InvalidValidatorsSetId,
		/// The number of signatures in the commitment is invalid.
		InvalidSignaturesLength,
		/// There are not enough correct signatures in commitment to finalize block.
		NotEnoughCorrectSignatures,
		/// MMR root is missing from the commitment.
		MmrRootMissingFromCommitment,
		/// MMR proof verification has failed.
		MmrProofVeriricationFailed,
		/// MMR leaf version is unknown to us.
		UnsupportedMmrLeafVersion,
		/// Next validator set id is invalid.
		InvalidNextValidatorsSetId,
		/// Next validators are provided when leaf is not signalling set change.
		RedundantNextValidatorsProvided,
		/// Next validators are not matching the merkle tree root.
		InvalidNextValidatorSetRoot,
		/// Next validator set is empty.
		EmptyNextValidatorSet,
	}

	/// Initialize pallet with given parameterns.
	pub(super) fn initialize<T: Config<I>, I: 'static>(
		init_data: InitializationDataOf<T, I>,
	) -> Result<(), Error<T, I>> {
		IsHalted::<T, I>::put(init_data.is_halted);
		BestBlockNumber::<T, I>::put(init_data.best_beefy_block_number);
		CurrentValidatorSet::<T, I>::put(
			BridgedBeefyValidatorSet::<T, I>::new(
				init_data.current_validator_set.1,
				init_data.current_validator_set.0,
			)
			.ok_or(Error::<T, I>::InvalidCurrentValidatorSet)?,
		);
		NextValidatorSet::<T, I>::put(
			BridgedBeefyValidatorSet::<T, I>::new(
				init_data.next_validator_set.1,
				init_data.next_validator_set.0,
			)
			.ok_or(Error::<T, I>::InvalidNextValidatorSet)?,
		);
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
	use codec::Encode;
	use frame_support::assert_ok;
	use mock::*;
	use mock_chain::*;

	fn import_header_chain(headers: Vec<HeaderAndCommitment>) {
		for header in headers {
			if let Some(commitment) = header.commitment {
				assert_ok!(Pallet::<TestRuntime, ()>::submit_commitment(
					Origin::signed(1),
					commitment.encode(),
					header.leaf_proof.expect("TODO").encode(),
					header.leaf.expect("TODO").encode(),
				));
			}
		}
	}

	#[test]
	fn submit_commitment_works_with_long_chain() {
		let _ = env_logger::try_init();

		run_test(|| {
			initialize::<TestRuntime, ()>(InitializationData {
				is_halted: false,
				best_beefy_block_number: 0,
				current_validator_set: (0, validator_ids(0, 32)),
				next_validator_set: (1, validator_ids(0, 32)),
			})
			.expect("initialization data is correct");

			let chain = ChainBuilder::new(32)
				.append_finalized_header() // 1
				.append_default_headers(16) // 2..17
				.append_finalized_header() // 18
				.append_default_headers(16) // 19..34
				.append_handoff_header(64) // 35
				.append_default_headers(8) // 36..43
				.append_finalized_header() // 44
				.append_default_headers(8) // 45..52
				.append_handoff_header(128) // 53
				.append_default_headers(4) // 54..57
				.append_finalized_header() // 58
				.append_default_headers(4); // 59..63
			import_header_chain(chain.into());

			assert_eq!(BestBlockNumber::<TestRuntime, ()>::get().unwrap(), 58);
			assert_eq!(CurrentValidatorSet::<TestRuntime, ()>::get().unwrap().id(), 2);
			assert_eq!(CurrentValidatorSet::<TestRuntime, ()>::get().unwrap().len(), 64);
			assert_eq!(NextValidatorSet::<TestRuntime, ()>::get().unwrap().id(), 3);
			assert_eq!(NextValidatorSet::<TestRuntime, ()>::get().unwrap().len(), 128);
		})
	}
}
