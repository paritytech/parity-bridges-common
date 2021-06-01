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

//! Substrate Finality Verifier Pallet
//!
//! This pallet is an on-chain GRANDPA light client for Substrate based chains.
//!
//! This pallet achieves this by trustlessly verifying GRANDPA finality proofs on-chain. Once
//! verified, finalized headers are stored in the pallet, thereby creating a sparse header chain.
//! This sparse header chain can be used as a source of truth for other higher-level applications.
//!
//! The pallet is responsible for tracking GRANDPA validator set hand-offs. We only import headers
//! with justifications signed by the current validator set we know of. The header is inspected for
//! a `ScheduledChanges` digest item, which is then used to update to next validator set.
//!
//! Since this pallet only tracks finalized headers it does not deal with forks. Forks can only
//! occur if the GRANDPA validator set on the bridged chain is either colluding or there is a severe
//! bug causing resulting in an equivocation. Such events are outside of the scope of this pallet.
//! Shall the fork occur on the bridged chain governance intervention will be required to
//! re-initialize the bridge and track the right fork.

#![cfg_attr(not(feature = "std"), no_std)]
// Runtime-generated enums
#![allow(clippy::large_enum_variant)]

use crate::weights::WeightInfo;

use bp_header_chain::justification::GrandpaJustification;
use bp_header_chain::InitializationData;
use bp_runtime::{BlockNumberOf, Chain, HashOf, HasherOf, HeaderOf, ChainId};
use finality_grandpa::voter_set::VoterSet;
use frame_support::ensure;
use frame_system::{ensure_signed, RawOrigin};
use sp_finality_grandpa::{ConsensusLog, GRANDPA_ENGINE_ID};
use sp_runtime::traits::{BadOrigin, Header as HeaderT, Zero};
use sp_std::vec::Vec;

#[cfg(test)]
mod mock;

/// Pallet containing weights for this pallet.
pub mod weights;

// #[cfg(feature = "runtime-benchmarks")]
// pub mod benchmarking;

// Re-export in crate namespace for `construct_runtime!`
pub use pallet::*;

/// Block number of the bridged chain.
pub type BridgedBlockNumber<T, I> = BlockNumberOf<<T as Config<I>>::BridgedChain>;
/// Block hash of the bridged chain.
pub type BridgedBlockHash<T, I> = HashOf<<T as Config<I>>::BridgedChain>;
/// Hasher of the bridged chain.
pub type BridgedBlockHasher<T, I> = HasherOf<<T as Config<I>>::BridgedChain>;
/// Header of the bridged chain.
pub type BridgedHeader<T, I> = HeaderOf<<T as Config<I>>::BridgedChain>;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config {
		/// The chain we are bridging to here.
		type BridgedChain: Chain;

		/// The upper bound on the number of requests allowed by the pallet.
		///
		/// A request refers to an action which writes a header to storage.
		///
		/// Once this bound is reached the pallet will not allow any dispatchables to be called
		/// until the request count has decreased.
		#[pallet::constant]
		type MaxRequests: Get<u32>;

		/// Maximal number of finalized headers to keep in the storage.
		///
		/// The setting is there to prevent growing the on-chain state indefinitely. Note
		/// the setting does not relate to block numbers - we will simply keep as much items
		/// in the storage, so it doesn't guarantee any fixed timeframe for finality headers.
		#[pallet::constant]
		type HeadersToKeep: Get<u32>;

		/// Weights gathered through benchmarking.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::hooks]
	impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I> {
		fn on_initialize(_n: T::BlockNumber) -> frame_support::weights::Weight {
			let mut acc_weight = 0_u64;

			for gateway_id in <InstantiatedGatewaysMap<T, I>>::get() {
				<RequestCountMap<T, I>>::mutate(gateway_id, |count| match count {
					Some(count) => *count = count.saturating_sub(1),
					_ => *count = Some(0),
				});

				acc_weight = acc_weight
					.saturating_add(T::DbWeight::get().reads(1))
					.saturating_add(T::DbWeight::get().writes(1));
			}
			acc_weight
		}
	}

	#[pallet::call]
	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		/// Verify a target header is finalized according to the given finality proof.
		///
		/// It will use the underlying storage pallet to fetch information about the current
		/// authorities and best finalized header in order to verify that the header is finalized.
		///
		/// If successful in verification, it will write the target header to the underlying storage
		/// pallet.
		#[pallet::weight(T::WeightInfo::submit_finality_proof(
			justification.votes_ancestries.len() as u32,
			justification.commit.precommits.len() as u32,
		))]
		pub fn submit_finality_proof(
			origin: OriginFor<T>,
			finality_target: BridgedHeader<T, I>,
			justification: GrandpaJustification<BridgedHeader<T, I>>,
			gateway_id: ChainId,
			extrinsics_root: BridgedBlockHash<T, I>,
			state_root: BridgedBlockHash<T, I>,
		) -> DispatchResultWithPostInfo {
			ensure_operational_single::<T, I>(gateway_id)?;
			let _ = ensure_signed(origin)?;
			ensure!(
				Self::request_count_map(gateway_id).unwrap_or(0) < T::MaxRequests::get(),
				<Error<T, I>>::TooManyRequests
			);

			let (hash, number) = (finality_target.hash(), finality_target.number());
			log::trace!("Going to try and finalize header {:?}", finality_target);

			let best_finalized = <MultiImportedHeaders<T, I>>::get(
				gateway_id,
				<BestFinalizedMap<T, I>>::get(gateway_id).expect(
					" Every time `BestFinalized` is updated `ImportedHeaders` is also updated. Therefore
				`ImportedHeaders` must contain an entry for `BestFinalized`.",
				),
			)
			.expect("In order to reach this point the bridge must have been initialized for given gateway.");

			// We do a quick check here to ensure that our header chain is making progress and isn't
			// "travelling back in time" (which could be indicative of something bad, e.g a hard-fork).
			ensure!(best_finalized.number() < number, <Error<T, I>>::OldHeader);
			let authority_set = <CurrentAuthoritySetMap<T, I>>::get(gateway_id)
				.expect("Expects authorities to be set before verify_justification");
			let set_id = authority_set.set_id;
			verify_justification_single::<T, I>(&justification, hash, *number, authority_set, gateway_id)?;

			let _enacted = try_enact_authority_change_single::<T, I>(&finality_target, set_id, gateway_id)?;
			let index = <MultiImportedHashesPointer<T, I>>::get(gateway_id).unwrap_or_default();

			let pruning = <MultiImportedHashes<T, I>>::try_get(gateway_id, index);

			<BestFinalizedMap<T, I>>::insert(gateway_id, hash);
			<MultiImportedHeaders<T, I>>::insert(gateway_id, hash, finality_target);
			<MultiImportedHashes<T, I>>::insert(gateway_id, index, hash);
			<MultiImportedRoots<T, I>>::insert(gateway_id, hash, (extrinsics_root, state_root));
			<RequestCountMap<T, I>>::mutate(gateway_id, |count| {
				match count {
					Some(count) => *count += 1,
					None => *count = Some(1),
				}
				*count
			});

			// Update ring buffer pointer and remove old header.
			<MultiImportedHashesPointer<T, I>>::insert(gateway_id, (index + 1) % T::HeadersToKeep::get());

			if let Ok(hash) = pruning {
				log::debug!(target: "runtime::multi-finality-verifier", "Pruning old header: {:?} for gateway {:?}.", hash, gateway_id);
				<MultiImportedHeaders<T, I>>::remove(gateway_id, hash);
				<MultiImportedRoots<T, I>>::remove(gateway_id, hash);
			}
			log::info!(
				"Succesfully imported finalized header with hash {:?} for gateway {:?}!",
				hash,
				gateway_id
			);

			Ok(().into())
		}

		/// Submit finality proofs for the header and additionally preserve state and extrinics root.
		#[pallet::weight(0)]
		pub fn submit_finality_proof_and_roots(
			origin: OriginFor<T>,
			finality_target: BridgedHeader<T, I>,
			justification: GrandpaJustification<BridgedHeader<T, I>>,
			gateway_id: ChainId,
			// ToDo: Try passing Vec<u8> here and cast to appropriate header type with help of xDNS
			state_root: BridgedBlockHash<T, I>,
			extrinsics_root: BridgedBlockHash<T, I>,
		) -> DispatchResultWithPostInfo {
			Self::submit_finality_proof(
				origin,
				finality_target,
				justification,
				gateway_id,
				extrinsics_root,
				state_root,
			)?;

			log::info!(
				"submit_finality_proof_and_roots, _state_root: {:?}, _extrinsics_root: {:?}",
				state_root,
				extrinsics_root
			);

			Ok(().into())
		}

		/// Bootstrap the bridge pallet with an initial header and authority set from which to sync.
		///
		/// The initial configuration provided does not need to be the genesis header of the bridged
		/// chain, it can be any arbirary header. You can also provide the next scheduled set change
		/// if it is already know.
		///
		/// This function is only allowed to be called from a trusted origin and writes to storage
		/// with practically no checks in terms of the validity of the data. It is important that
		/// you ensure that valid data is being passed in.
		#[pallet::weight((T::DbWeight::get().reads_writes(2, 5), DispatchClass::Operational))]
		pub fn initialize_single(
			origin: OriginFor<T>,
			init_data: super::InitializationData<BridgedHeader<T, I>>,
			gateway_id: ChainId,
		) -> DispatchResultWithPostInfo {
			ensure_owner_or_root_single::<T, I>(origin, gateway_id)?;

			let init_allowed = !<BestFinalizedMap<T, I>>::contains_key(gateway_id);
			ensure!(init_allowed, <Error<T, I>>::AlreadyInitialized);
			initialize_single_bridge::<T, I>(init_data.clone(), gateway_id);

			log::debug!(
				"Pallet has been initialized with the following parameters: {:?}, {:?}",
				gateway_id,
				init_data
			);

			Ok(().into())
		}


		/// Change `PalletOwner`.
		///
		/// May only be called either by root, or by `PalletOwner`.
		#[pallet::weight((T::DbWeight::get().reads_writes(1, 1), DispatchClass::Operational))]
		pub fn set_owner(
			origin: OriginFor<T>,
			new_owner: Option<T::AccountId>,
			gateway_id: ChainId,
		) -> DispatchResultWithPostInfo {
			ensure_owner_or_root_single::<T, I>(origin, gateway_id)?;
			match new_owner {
				Some(new_owner) => {
					PalletOwnerMap::<T, I>::insert(gateway_id, &new_owner);
					log::info!("Setting pallet Owner to: {:?}", new_owner);
				}
				None => {
					PalletOwnerMap::<T, I>::remove(gateway_id);
					log::info!("Removed Owner of pallet.");
				}
			}

			Ok(().into())
		}

		/// Halt or resume all pallet operations.
		///
		/// May only be called either by root, or by `PalletOwner`.
		#[pallet::weight((T::DbWeight::get().reads_writes(1, 1), DispatchClass::Operational))]
		pub fn set_operational(
			origin: OriginFor<T>,
			operational: bool,
			gateway_id: ChainId,
		) -> DispatchResultWithPostInfo {
			ensure_owner_or_root_single::<T, I>(origin, gateway_id)?;
			<IsHaltedMap<T, I>>::insert(gateway_id, operational);

			if operational {
				log::info!("Resuming pallet operations.");
			} else {
				log::warn!("Stopping pallet operations.");
			}

			Ok(().into())
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
	#[pallet::getter(fn request_count_map)]
	pub(super) type RequestCountMap<T: Config<I>, I: 'static = ()> = StorageMap<_, Blake2_256, ChainId, u32>;

	/// Hash of the header used to bootstrap the pallet.
	#[pallet::storage]
	pub(super) type InitialHashMap<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Blake2_256, ChainId, BridgedBlockHash<T, I>>;

	/// Map of hashes of the best finalized header.
	#[pallet::storage]
	pub(super) type BestFinalizedMap<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Blake2_256, ChainId, BridgedBlockHash<T, I>>;

	/// A ring buffer of imported hashes. Ordered by the insertion time.
	#[pallet::storage]
	pub(super) type MultiImportedHashes<T: Config<I>, I: 'static = ()> =
		StorageDoubleMap<_, Blake2_256, ChainId, Identity, u32, BridgedBlockHash<T, I>>;

	/// Current ring buffer position.
	#[pallet::storage]
	pub(super) type MultiImportedHashesPointer<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Blake2_256, ChainId, u32>;

	/// Headers which have been imported into the pallet.
	#[pallet::storage]
	pub(super) type MultiImportedHeaders<T: Config<I>, I: 'static = ()> =
		StorageDoubleMap<_, Blake2_256, ChainId, Identity, BridgedBlockHash<T, I>, BridgedHeader<T, I>>;

	/// Roots (ExtrinsicsRoot + StateRoot) which have been imported into the pallet for a given gateway.
	#[pallet::storage]
	pub(super) type MultiImportedRoots<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
		_,
		Blake2_256,
		ChainId,
		Identity,
		BridgedBlockHash<T, I>,
		(BridgedBlockHash<T, I>, BridgedBlockHash<T, I>),
	>;

	/// The current GRANDPA Authority set map.
	#[pallet::storage]
	pub(super) type CurrentAuthoritySetMap<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Blake2_256, ChainId, bp_header_chain::AuthoritySet>;

	/// Optional pallet owner.
	///
	/// Pallet owner has a right to halt all pallet operations and then resume it. If it is
	/// `None`, then there are no direct ways to halt/resume pallet operations, but other
	/// runtime methods may still be used to do that (i.e. democracy::referendum to update halt
	/// flag directly or call the `halt_operations`).
	#[pallet::storage]
	pub(super) type PalletOwner<T: Config<I>, I: 'static = ()> = StorageValue<_, T::AccountId, OptionQuery>;

	/// Optional pallet owner.
	///
	/// Pallet owner has a right to halt all pallet operations and then resume it. If it is
	/// `None`, then there are no direct ways to halt/resume pallet operations, but other
	/// runtime methods may still be used to do that (i.e. democracy::referendum to update halt
	/// flag directly or call the `halt_operations`).
	#[pallet::storage]
	pub(super) type PalletOwnerMap<T: Config<I>, I: 'static = ()> = StorageMap<_, Blake2_256, ChainId, T::AccountId>;

	/// If true, all pallet transactions are failed immediately.
	#[pallet::storage]
	pub(super) type IsHalted<T: Config<I>, I: 'static = ()> = StorageValue<_, bool, ValueQuery>;

	/// If true, all pallet transactions are failed immediately.
	#[pallet::storage]
	pub(super) type IsHaltedMap<T: Config<I>, I: 'static = ()> = StorageMap<_, Blake2_256, ChainId, bool>;

	/// Map of instance ids of gateways which are active
	#[pallet::storage]
	pub(super) type InstantiatedGatewaysMap<T: Config<I>, I: 'static = ()> =
		StorageValue<_, Vec<ChainId>, ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config<I>, I: 'static = ()> {
		/// Optional module owner account.
		pub owner: Option<T::AccountId>,
		/// Optional module initialization data.
		pub init_data: Option<super::InitializationData<BridgedHeader<T, I>>>,
	}

	#[cfg(feature = "std")]
	impl<T: Config<I>, I: 'static> Default for GenesisConfig<T, I> {
		fn default() -> Self {
			Self {
				owner: None,
				init_data: None,
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config<I>, I: 'static> GenesisBuild<T, I> for GenesisConfig<T, I> {
		fn build(&self) {
			if let Some(ref owner) = self.owner {
				<PalletOwner<T, I>>::put(owner);
			}

			if let Some(init_data) = self.init_data.clone() {
				initialize_bridge::<T, I>(init_data);
			} else {
				// Since the bridge hasn't been initialized we shouldn't allow anyone to perform
				// transactions.
				<IsHalted<T, I>>::put(true);
			}
		}
	}

	#[pallet::error]
	pub enum Error<T, I = ()> {
		/// The given justification is invalid for the given header.
		InvalidJustification,
		/// The authority set from the underlying header chain is invalid.
		InvalidAuthoritySet,
		/// There are too many requests for the current window to handle.
		TooManyRequests,
		/// The header being imported is older than the best finalized header known to the pallet.
		OldHeader,
		/// The header is unknown to the pallet.
		UnknownHeader,
		/// The scheduled authority set change found in the header is unsupported by the pallet.
		///
		/// This is the case for non-standard (e.g forced) authority set changes.
		UnsupportedScheduledChange,
		/// The pallet has already been initialized.
		AlreadyInitialized,
		/// All pallet operations are halted.
		Halted,
		/// The storage proof doesn't contains storage root. So it is invalid for given header.
		StorageRootMismatch,
	}

	/// Check the given header for a GRANDPA scheduled authority set change. If a change
	/// is found it will be enacted immediately.
	///
	/// This function does not support forced changes, or scheduled changes with delays
	/// since these types of changes are indicitive of abnormal behaviour from GRANDPA.
	pub(crate) fn try_enact_authority_change_single<T: Config<I>, I: 'static>(
		header: &BridgedHeader<T, I>,
		current_set_id: sp_finality_grandpa::SetId,
		gateway_id: ChainId,
	) -> Result<bool, sp_runtime::DispatchError> {
		let mut change_enacted = false;

		// We don't support forced changes - at that point governance intervention is required.
		ensure!(
			super::find_forced_change(header).is_none(),
			<Error<T, I>>::UnsupportedScheduledChange
		);

		if let Some(change) = super::find_scheduled_change(header) {
			// GRANDPA only includes a `delay` for forced changes, so this isn't valid.
			ensure!(change.delay == Zero::zero(), <Error<T, I>>::UnsupportedScheduledChange);

			// TODO [#788]: Stop manually increasing the `set_id` here.
			let next_authorities = bp_header_chain::AuthoritySet {
				authorities: change.next_authorities,
				set_id: current_set_id + 1,
			};

			// Since our header schedules a change and we know the delay is 0, it must also enact
			// the change.
			<CurrentAuthoritySetMap<T, I>>::insert(gateway_id, &next_authorities);
			change_enacted = true;

			log::info!(
				"Transitioned from authority set {} to {}! New authorities are: {:?} for gateway: {:?}",
				current_set_id,
				current_set_id + 1,
				next_authorities,
				gateway_id,
			);
		};

		Ok(change_enacted)
	}

	/// Verify a GRANDPA justification (finality proof) for a given header.
	///
	/// Will use the GRANDPA current authorities known to the pallet.
	///
	/// If succesful it returns the decoded GRANDPA justification so we can refund any weight which
	/// was overcharged in the initial call.
	pub(crate) fn verify_justification_single<T: Config<I>, I: 'static>(
		justification: &GrandpaJustification<BridgedHeader<T, I>>,
		hash: BridgedBlockHash<T, I>,
		number: BridgedBlockNumber<T, I>,
		authority_set: bp_header_chain::AuthoritySet,
		_gateway_id: ChainId,
	) -> Result<(), sp_runtime::DispatchError> {
		use bp_header_chain::justification::verify_justification;

		let voter_set = VoterSet::new(authority_set.authorities).ok_or(<Error<T, I>>::InvalidAuthoritySet)?;
		let set_id = authority_set.set_id;

		Ok(
			verify_justification::<BridgedHeader<T, I>>((hash, number), set_id, &voter_set, &justification).map_err(
				|e| {
					log::error!("Received invalid justification for {:?}: {:?}", hash, e);
					<Error<T, I>>::InvalidJustification
				},
			)?,
		)
	}

	/// Since this writes to storage with no real checks this should only be used in functions that
	/// were called by a trusted origin.
	pub fn initialize_bridge<T: Config<I>, I: 'static>(init_params: super::InitializationData<BridgedHeader<T, I>>) {
		let default_gateway: ChainId = *b"gate";
		initialize_single_bridge::<T, I>(init_params, default_gateway)
	}

	/// Since this writes to storage with no real checks this should only be used in functions that
	/// were called by a trusted origin.
	pub(crate) fn initialize_single_bridge<T: Config<I>, I: 'static>(
		init_params: super::InitializationData<BridgedHeader<T, I>>,
		gateway_id: ChainId,
	) {
		let super::InitializationData {
			header,
			authority_list,
			set_id,
			is_halted,
		} = init_params;

		let initial_hash = header.hash();
		<InitialHashMap<T, I>>::insert(gateway_id, initial_hash);
		<BestFinalizedMap<T, I>>::insert(gateway_id, initial_hash);
		<MultiImportedHeaders<T, I>>::insert(gateway_id, initial_hash, header);

		// might get problematic
		let authority_set = bp_header_chain::AuthoritySet::new(authority_list, set_id);
		<CurrentAuthoritySetMap<T, I>>::insert(gateway_id, authority_set);
		<IsHaltedMap<T, I>>::insert(gateway_id, is_halted);

		<InstantiatedGatewaysMap<T, I>>::mutate(|gateways| {
			gateways.push(gateway_id);
			gateways.len() + 1
		});
	}

	/// Ensure that the origin is either root, or `PalletOwner`.
	fn ensure_owner_or_root_single<T: Config<I>, I: 'static>(
		origin: T::Origin,
		gateway_id: ChainId,
	) -> Result<(), BadOrigin> {
		match origin.into() {
			Ok(RawOrigin::Root) => Ok(()),
			Ok(RawOrigin::Signed(ref signer))
				if <PalletOwnerMap<T, I>>::contains_key(gateway_id)
					&& Some(signer) == <PalletOwnerMap<T, I>>::get(gateway_id).as_ref() =>
			{
				Ok(())
			}
			_ => Err(BadOrigin),
		}
	}

	/// Ensure that the pallet is in operational mode (not halted).
	fn ensure_operational_single<T: Config<I>, I: 'static>(gateway_id: ChainId) -> Result<(), Error<T, I>> {
		if <IsHaltedMap<T, I>>::get(gateway_id)
			.expect("Is halted prop is should have been set before during initialize")
		{
			Err(<Error<T, I>>::Halted)
		} else {
			Ok(())
		}
	}
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
	/// Get the best finalized header the pallet knows of.
	///
	/// Returns a dummy header if there is no best header. This can only happen
	/// if the pallet has not been initialized yet.
	pub fn best_finalized_map(gateway_id: ChainId) -> BridgedHeader<T, I> {
		let hash = <BestFinalizedMap<T, I>>::get(gateway_id).unwrap_or_default();
		<MultiImportedHeaders<T, I>>::get(gateway_id, hash).unwrap_or_else(|| {
			<BridgedHeader<T, I>>::new(
				Default::default(),
				Default::default(),
				Default::default(),
				Default::default(),
				Default::default(),
			)
		})
	}

	/// Check if a particular header is known to the bridge pallet.
	pub fn is_known_header(hash: BridgedBlockHash<T, I>, gateway_id: ChainId) -> bool {
		<MultiImportedHeaders<T, I>>::contains_key(gateway_id, hash)
	}

	/// Verify that the passed storage proof is valid, given it is crafted using
	/// known finalized header. If the proof is valid, then the `parse` callback
	/// is called and the function returns its result.
	pub fn parse_finalized_storage_proof<R>(
		hash: BridgedBlockHash<T, I>,
		storage_proof: sp_trie::StorageProof,
		parse: impl FnOnce(bp_runtime::StorageProofChecker<BridgedBlockHasher<T, I>>) -> R,
		gateway_id: ChainId,
	) -> Result<R, sp_runtime::DispatchError> {
		let header = <MultiImportedHeaders<T, I>>::get(gateway_id, hash).ok_or(Error::<T, I>::UnknownHeader)?;
		let storage_proof_checker = bp_runtime::StorageProofChecker::new(*header.state_root(), storage_proof)
			.map_err(|_| Error::<T, I>::StorageRootMismatch)?;

		Ok(parse(storage_proof_checker))
	}
}

pub(crate) fn find_scheduled_change<H: HeaderT>(header: &H) -> Option<sp_finality_grandpa::ScheduledChange<H::Number>> {
	use sp_runtime::generic::OpaqueDigestItemId;

	let id = OpaqueDigestItemId::Consensus(&GRANDPA_ENGINE_ID);

	let filter_log = |log: ConsensusLog<H::Number>| match log {
		ConsensusLog::ScheduledChange(change) => Some(change),
		_ => None,
	};

	// find the first consensus digest with the right ID which converts to
	// the right kind of consensus log.
	header.digest().convert_first(|l| l.try_to(id).and_then(filter_log))
}

/// Checks the given header for a consensus digest signalling a **forced** scheduled change and
/// extracts it.
pub(crate) fn find_forced_change<H: HeaderT>(
	header: &H,
) -> Option<(H::Number, sp_finality_grandpa::ScheduledChange<H::Number>)> {
	use sp_runtime::generic::OpaqueDigestItemId;

	let id = OpaqueDigestItemId::Consensus(&GRANDPA_ENGINE_ID);

	let filter_log = |log: ConsensusLog<H::Number>| match log {
		ConsensusLog::ForcedChange(delay, change) => Some((delay, change)),
		_ => None,
	};

	// find the first consensus digest with the right ID which converts to
	// the right kind of consensus log.
	header.digest().convert_first(|l| l.try_to(id).and_then(filter_log))
}

/// (Re)initialize bridge with given header for using it in `pallet-bridge-messages` benchmarks.
#[cfg(feature = "runtime-benchmarks")]
pub fn initialize_for_benchmarks<T: Config<I>, I: 'static>(header: BridgedHeader<T, I>) {
	initialize_bridge::<T, I>(InitializationData {
		header,
		authority_list: sp_std::vec::Vec::new(), // we don't verify any proofs in external benchmarks
		set_id: 0,
		is_halted: false,
	});
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{run_test, test_header, Origin, TestHash, TestHeader, TestNumber, TestRuntime};
	use bp_test_utils::{
		authority_list, make_default_justification, make_justification_for_header, JustificationGeneratorParams, ALICE,
		BOB,
	};
	use codec::Encode;
	use frame_support::weights::PostDispatchInfo;
	use frame_support::{assert_err, assert_noop, assert_ok};
	use sp_runtime::{Digest, DigestItem, DispatchError};

	fn teardown_substrate_bridge() {
		let default_gateway: ChainId = *b"gate";
		// Reset storage so we can initialize the pallet again
		BestFinalizedMap::<TestRuntime>::remove(default_gateway);
		PalletOwnerMap::<TestRuntime>::remove(default_gateway);
		MultiImportedRoots::<TestRuntime>::remove_prefix(default_gateway);
		BestFinalizedMap::<TestRuntime>::remove(default_gateway);
		MultiImportedHashesPointer::<TestRuntime>::remove(default_gateway);
		MultiImportedHashes::<TestRuntime>::remove_prefix(default_gateway);
		MultiImportedHeaders::<TestRuntime>::remove_prefix(default_gateway);
		MultiImportedRoots::<TestRuntime>::remove_prefix(default_gateway);
		RequestCountMap::<TestRuntime>::remove(default_gateway);
		InstantiatedGatewaysMap::<TestRuntime>::kill();
	}

	fn initialize_substrate_bridge() {
		teardown_substrate_bridge();
		let default_gateway: ChainId = *b"gate";
		assert_ok!(init_single_gateway_with_origin(Origin::root(), default_gateway));
	}

	fn initialize_substrate_bridge_for_gateway(gateway_id: ChainId) {
		assert_ok!(init_single_gateway_with_origin(Origin::root(), gateway_id));
	}

	fn init_single_gateway_with_origin(
		origin: Origin,
		gateway_id: ChainId,
	) -> Result<InitializationData<TestHeader>, sp_runtime::DispatchErrorWithPostInfo<PostDispatchInfo>> {
		let genesis = test_header(0);

		let init_data = InitializationData {
			header: genesis,
			authority_list: authority_list(),
			set_id: 1,
			is_halted: false,
		};

		Pallet::<TestRuntime>::initialize_single(origin, init_data.clone(), gateway_id).map(|_| init_data)
	}

	fn init_with_origin(
		origin: Origin,
	) -> Result<InitializationData<TestHeader>, sp_runtime::DispatchErrorWithPostInfo<PostDispatchInfo>> {
		let genesis = test_header(0);

		let init_data = InitializationData {
			header: genesis,
			authority_list: authority_list(),
			set_id: 1,
			is_halted: false,
		};
		let default_gateway: ChainId = *b"gate";

		Pallet::<TestRuntime>::initialize_single(origin, init_data.clone(), default_gateway).map(|_| init_data)
	}

	fn submit_finality_proof(header: u8) -> frame_support::dispatch::DispatchResultWithPostInfo {
		let header = test_header(header.into());

		let justification = make_default_justification(&header);

		let default_gateway: ChainId = *b"gate";

		let default_roots = (Default::default(), Default::default());

		Pallet::<TestRuntime>::submit_finality_proof(
			Origin::signed(1),
			header,
			justification,
			default_gateway,
			default_roots.0,
			default_roots.1,
		)
	}

	fn submit_finality_proof_and_roots(
		header: u8,
		state_root: TestHash,
		extrinsics_root: TestHash,
	) -> frame_support::dispatch::DispatchResultWithPostInfo {
		let header = test_header(header.into());

		let justification = make_default_justification(&header);

		let default_gateway: ChainId = *b"gate";

		Pallet::<TestRuntime>::submit_finality_proof_and_roots(
			Origin::signed(1),
			header,
			justification,
			default_gateway,
			state_root,
			extrinsics_root,
		)
	}

	fn next_block() {
		use frame_support::traits::OnInitialize;

		let current_number = frame_system::Pallet::<TestRuntime>::block_number();
		frame_system::Pallet::<TestRuntime>::set_block_number(current_number + 1);
		let _ = Pallet::<TestRuntime>::on_initialize(current_number);
	}

	fn change_log(delay: u64) -> Digest<TestHash> {
		let consensus_log = ConsensusLog::<TestNumber>::ScheduledChange(sp_finality_grandpa::ScheduledChange {
			next_authorities: vec![(ALICE.into(), 1), (BOB.into(), 1)],
			delay,
		});

		Digest::<TestHash> {
			logs: vec![DigestItem::Consensus(GRANDPA_ENGINE_ID, consensus_log.encode())],
		}
	}

	fn forced_change_log(delay: u64) -> Digest<TestHash> {
		let consensus_log = ConsensusLog::<TestNumber>::ForcedChange(
			delay,
			sp_finality_grandpa::ScheduledChange {
				next_authorities: vec![(ALICE.into(), 1), (BOB.into(), 1)],
				delay,
			},
		);

		Digest::<TestHash> {
			logs: vec![DigestItem::Consensus(GRANDPA_ENGINE_ID, consensus_log.encode())],
		}
	}

	#[test]
	fn init_root_or_owner_origin_can_initialize_pallet() {
		let default_gateway: ChainId = *b"gate";

		run_test(|| {
			assert_noop!(init_with_origin(Origin::signed(1)), DispatchError::BadOrigin);
			assert_ok!(init_with_origin(Origin::root()));

			// Reset storage so we can initialize the pallet again
			BestFinalizedMap::<TestRuntime>::remove(default_gateway);
			PalletOwnerMap::<TestRuntime>::insert(default_gateway, 2);
			assert_ok!(init_with_origin(Origin::signed(2)));
		})
	}

	#[test]
	fn init_storage_entries_are_correctly_initialized() {
		let default_gateway: ChainId = *b"gate";

		run_test(|| {
			assert_eq!(BestFinalizedMap::<TestRuntime>::get(default_gateway), None,);
			assert_eq!(
				Pallet::<TestRuntime>::best_finalized_map(default_gateway),
				test_header(0)
			);

			let init_data = init_with_origin(Origin::root()).unwrap();

			assert!(<MultiImportedHeaders<TestRuntime>>::contains_key(
				default_gateway,
				init_data.header.hash()
			));
			assert_eq!(
				BestFinalizedMap::<TestRuntime>::get(default_gateway),
				Some(init_data.header.hash())
			);
			assert_eq!(
				CurrentAuthoritySetMap::<TestRuntime>::get(default_gateway)
					.unwrap()
					.authorities,
				init_data.authority_list
			);
			assert_eq!(IsHaltedMap::<TestRuntime>::get(default_gateway), Some(false));
		})
	}

	#[test]
	fn init_can_only_initialize_pallet_once() {
		run_test(|| {
			initialize_substrate_bridge();
			assert_noop!(
				init_with_origin(Origin::root()),
				<Error<TestRuntime>>::AlreadyInitialized
			);
		})
	}

	#[test]
	fn init_can_initialize_pallet_for_multiple_gateway_but_only_once_per_each_gateway() {
		run_test(|| {
			let gateway_a: ChainId = *b"rlta";
			let gateway_b: ChainId = *b"rltb";

			initialize_substrate_bridge_for_gateway(gateway_a);
			initialize_substrate_bridge_for_gateway(gateway_b);
			assert_noop!(
				init_single_gateway_with_origin(Origin::root(), gateway_a),
				<Error<TestRuntime>>::AlreadyInitialized
			);
		})
	}

	#[test]
	fn can_initialize_new_polka_like_bridge_with_separate_vefifier_instance() {
		run_test(|| {
			let gateway_a: ChainId = *b"rlta";

			let rh: bp_rialto::Header = bp_rialto::Header::new(
				1,
				Default::default(),
				Default::default(),
				Default::default(),
				Default::default(),
			);
			let init_data = InitializationData {
				header: rh,
				authority_list: authority_list(),
				set_id: 1,
				is_halted: false,
			};

			assert_ok!(
				mock::MultiFinalityVerifierPolkadotLike::initialize_single(
					Origin::root(),
					init_data.clone(),
					gateway_a
				)
				.map(|_| init_data)
			);
		})
	}

	#[test]
	fn pallet_owner_may_change_owner() {
		run_test(|| {
			PalletOwner::<TestRuntime>::put(2);
			let default_gateway: ChainId = *b"gate";

			assert_ok!(Pallet::<TestRuntime>::set_owner(
				Origin::root(),
				Some(1),
				default_gateway
			));
			assert_noop!(
				Pallet::<TestRuntime>::set_operational(Origin::signed(2), false, default_gateway),
				DispatchError::BadOrigin,
			);
			assert_ok!(Pallet::<TestRuntime>::set_operational(
				Origin::root(),
				false,
				default_gateway
			));

			assert_ok!(Pallet::<TestRuntime>::set_owner(
				Origin::signed(1),
				None,
				default_gateway
			));
			assert_noop!(
				Pallet::<TestRuntime>::set_operational(Origin::signed(1), true, default_gateway),
				DispatchError::BadOrigin,
			);
			assert_noop!(
				Pallet::<TestRuntime>::set_operational(Origin::signed(2), true, default_gateway),
				DispatchError::BadOrigin,
			);
			assert_ok!(Pallet::<TestRuntime>::set_operational(
				Origin::root(),
				true,
				default_gateway
			));
		});
	}

	#[test]
	fn pallet_may_be_halted_by_root() {
		let default_gateway: ChainId = *b"gate";

		run_test(|| {
			assert_ok!(Pallet::<TestRuntime>::set_operational(
				Origin::root(),
				false,
				default_gateway
			));
			assert_ok!(Pallet::<TestRuntime>::set_operational(
				Origin::root(),
				true,
				default_gateway
			));
		});
	}

	#[test]
	fn pallet_may_be_halted_by_owner() {
		let default_gateway: ChainId = *b"gate";

		run_test(|| {
			PalletOwnerMap::<TestRuntime>::insert(default_gateway, 2);

			assert_ok!(Pallet::<TestRuntime>::set_operational(
				Origin::signed(2),
				false,
				default_gateway
			));
			assert_ok!(Pallet::<TestRuntime>::set_operational(
				Origin::signed(2),
				true,
				default_gateway
			));

			assert_noop!(
				Pallet::<TestRuntime>::set_operational(Origin::signed(1), false, default_gateway),
				DispatchError::BadOrigin,
			);
			assert_noop!(
				Pallet::<TestRuntime>::set_operational(Origin::signed(1), true, default_gateway),
				DispatchError::BadOrigin,
			);

			assert_ok!(Pallet::<TestRuntime>::set_operational(
				Origin::signed(2),
				false,
				default_gateway
			));
			assert_noop!(
				Pallet::<TestRuntime>::set_operational(Origin::signed(1), true, default_gateway),
				DispatchError::BadOrigin,
			);
		});
	}

	#[test]
	fn pallet_rejects_transactions_if_halted() {
		run_test(|| {
			let gateway_a: ChainId = *b"gate";
			<IsHaltedMap<TestRuntime>>::insert(gateway_a, true);

			let header = test_header(1);
			let justification = make_default_justification(&header);

			assert_noop!(
				Pallet::<TestRuntime>::submit_finality_proof(
					Origin::signed(1),
					header,
					justification,
					gateway_a,
					Default::default(),
					Default::default()
				),
				Error::<TestRuntime>::Halted,
			);
		})
	}

	#[test]
	fn succesfully_imports_header_with_valid_finality() {
		let default_gateway: ChainId = *b"gate";
		run_test(|| {
			initialize_substrate_bridge();
			assert_ok!(submit_finality_proof(1));

			let header = test_header(1);
			assert_eq!(
				<BestFinalizedMap<TestRuntime>>::get(default_gateway),
				Some(header.hash())
			);
			assert!(<MultiImportedHeaders<TestRuntime>>::contains_key(
				default_gateway,
				header.hash()
			));
		})
	}

	#[test]
	fn rejects_justification_that_skips_authority_set_transition() {
		run_test(|| {
			initialize_substrate_bridge();

			let header = test_header(1);

			let params = JustificationGeneratorParams::<TestHeader> {
				set_id: 2,
				..Default::default()
			};
			let justification = make_justification_for_header(params);

			let default_gateway: ChainId = *b"gate";

			assert_err!(
				Pallet::<TestRuntime>::submit_finality_proof(
					Origin::signed(1),
					header,
					justification,
					default_gateway,
					Default::default(),
					Default::default()
				),
				<Error<TestRuntime>>::InvalidJustification
			);
		})
	}

	#[test]
	fn does_not_import_header_with_invalid_finality_proof() {
		run_test(|| {
			initialize_substrate_bridge();

			let header = test_header(1);
			let mut justification = make_default_justification(&header);
			justification.round = 42;
			let default_gateway: ChainId = *b"gate";

			assert_err!(
				Pallet::<TestRuntime>::submit_finality_proof(
					Origin::signed(1),
					header,
					justification,
					default_gateway,
					Default::default(),
					Default::default()
				),
				<Error<TestRuntime>>::InvalidJustification
			);
		})
	}

	#[test]
	fn disallows_invalid_authority_set() {
		run_test(|| {
			let genesis = test_header(0);

			let invalid_authority_list = vec![(ALICE.into(), u64::MAX), (BOB.into(), u64::MAX)];
			let init_data = InitializationData {
				header: genesis,
				authority_list: invalid_authority_list,
				set_id: 1,
				is_halted: false,
			};

			let default_gateway: ChainId = *b"gate";

			assert_ok!(Pallet::<TestRuntime>::initialize_single(
				Origin::root(),
				init_data,
				default_gateway
			));

			let header = test_header(1);
			let justification = make_default_justification(&header);

			assert_err!(
				Pallet::<TestRuntime>::submit_finality_proof(
					Origin::signed(1),
					header,
					justification,
					default_gateway,
					Default::default(),
					Default::default()
				),
				<Error<TestRuntime>>::InvalidAuthoritySet
			);
		})
	}

	#[test]
	fn importing_header_ensures_that_chain_is_extended() {
		run_test(|| {
			initialize_substrate_bridge();

			assert_ok!(submit_finality_proof(4));
			assert_err!(submit_finality_proof(3), Error::<TestRuntime>::OldHeader);
			assert_ok!(submit_finality_proof(5));
		})
	}

	#[test]
	fn importing_header_enacts_new_authority_set() {
		run_test(|| {
			initialize_substrate_bridge();

			let next_set_id = 2;
			let next_authorities = vec![(ALICE.into(), 1), (BOB.into(), 1)];

			// Need to update the header digest to indicate that our header signals an authority set
			// change. The change will be enacted when we import our header.
			let mut header = test_header(2);
			header.digest = change_log(0);

			let justification = make_default_justification(&header);

			let default_gateway: ChainId = *b"gate";

			// Let's import our test header
			assert_ok!(Pallet::<TestRuntime>::submit_finality_proof(
				Origin::signed(1),
				header.clone(),
				justification,
				default_gateway,
				Default::default(),
				Default::default(),
			));

			// Make sure that our header is the best finalized
			assert_eq!(
				<BestFinalizedMap<TestRuntime>>::get(default_gateway),
				Some(header.hash())
			);
			assert!(<MultiImportedHeaders<TestRuntime>>::contains_key(
				default_gateway,
				header.hash()
			));

			// Make sure that the authority set actually changed upon importing our header
			assert_eq!(
				<CurrentAuthoritySetMap<TestRuntime>>::get(default_gateway),
				Some(bp_header_chain::AuthoritySet::new(next_authorities, next_set_id)),
			);
		})
	}

	#[test]
	fn importing_header_rejects_header_with_scheduled_change_delay() {
		run_test(|| {
			initialize_substrate_bridge();

			// Need to update the header digest to indicate that our header signals an authority set
			// change. However, the change doesn't happen until the next block.
			let mut header = test_header(2);
			header.digest = change_log(1);

			let justification = make_default_justification(&header);

			let default_gateway: ChainId = *b"gate";

			// Should not be allowed to import this header
			assert_err!(
				Pallet::<TestRuntime>::submit_finality_proof(
					Origin::signed(1),
					header,
					justification,
					default_gateway,
					Default::default(),
					Default::default()
				),
				<Error<TestRuntime>>::UnsupportedScheduledChange
			);
		})
	}

	#[test]
	fn importing_header_rejects_header_with_forced_changes() {
		run_test(|| {
			initialize_substrate_bridge();

			// Need to update the header digest to indicate that it signals a forced authority set
			// change.
			let mut header = test_header(2);
			header.digest = forced_change_log(0);

			let justification = make_default_justification(&header);

			let default_gateway: ChainId = *b"gate";

			// Should not be allowed to import this header
			assert_err!(
				Pallet::<TestRuntime>::submit_finality_proof(
					Origin::signed(1),
					header,
					justification,
					default_gateway,
					Default::default(),
					Default::default()
				),
				<Error<TestRuntime>>::UnsupportedScheduledChange
			);
		})
	}

	#[test]
	fn parse_finalized_storage_proof_rejects_proof_on_unknown_header() {
		let default_gateway: ChainId = *b"gate";

		run_test(|| {
			assert_noop!(
				Pallet::<TestRuntime>::parse_finalized_storage_proof(
					Default::default(),
					sp_trie::StorageProof::new(vec![]),
					|_| (),
					default_gateway,
				),
				Error::<TestRuntime>::UnknownHeader,
			);
		});
	}

	#[test]
	fn parse_finalized_storage_accepts_valid_proof() {
		let default_gateway: ChainId = *b"gate";

		run_test(|| {
			let (state_root, storage_proof) = bp_runtime::craft_valid_storage_proof();

			let mut header = test_header(2);
			header.set_state_root(state_root);

			let hash = header.hash();
			<BestFinalizedMap<TestRuntime>>::insert(default_gateway, hash);
			<MultiImportedHeaders<TestRuntime>>::insert(default_gateway, hash, header);

			assert_ok!(
				Pallet::<TestRuntime>::parse_finalized_storage_proof(hash, storage_proof, |_| (), default_gateway),
				(),
			);
		});
	}

	#[test]
	fn rate_limiter_disallows_imports_once_limit_is_hit_in_single_block() {
		run_test(|| {
			initialize_substrate_bridge();

			assert_ok!(submit_finality_proof(1));
			assert_ok!(submit_finality_proof(2));
			assert_err!(submit_finality_proof(3), <Error<TestRuntime>>::TooManyRequests);
		})
	}

	#[test]
	fn rate_limiter_invalid_requests_do_not_count_towards_request_count() {
		let default_gateway: ChainId = *b"gate";
		run_test(|| {
			let submit_invalid_request = || {
				let header = test_header(1);
				let mut invalid_justification = make_default_justification(&header);
				invalid_justification.round = 42;

				Pallet::<TestRuntime>::submit_finality_proof(
					Origin::signed(1),
					header,
					invalid_justification,
					default_gateway,
					Default::default(),
					Default::default(),
				)
			};

			initialize_substrate_bridge();

			for _ in 0..<TestRuntime as Config>::MaxRequests::get() + 1 {
				// Notice that the error here *isn't* `TooManyRequests`
				assert_err!(submit_invalid_request(), <Error<TestRuntime>>::InvalidJustification);
			}

			// Can still submit `MaxRequests` requests afterwards
			assert_ok!(submit_finality_proof(1));
			assert_ok!(submit_finality_proof(2));
			assert_err!(submit_finality_proof(3), <Error<TestRuntime>>::TooManyRequests);
		})
	}

	#[test]
	fn rate_limiter_allows_request_after_new_block_has_started() {
		run_test(|| {
			initialize_substrate_bridge();
			assert_ok!(submit_finality_proof(1));
			assert_ok!(submit_finality_proof(2));

			next_block();
			assert_ok!(submit_finality_proof(3));
		})
	}

	#[test]
	fn rate_limiter_disallows_imports_once_limit_is_hit_across_different_blocks() {
		run_test(|| {
			initialize_substrate_bridge();
			assert_ok!(submit_finality_proof(1));
			assert_ok!(submit_finality_proof(2));

			next_block();
			assert_ok!(submit_finality_proof(3));
			assert_err!(submit_finality_proof(4), <Error<TestRuntime>>::TooManyRequests);
		})
	}

	#[test]
	fn rate_limiter_allows_max_requests_after_long_time_with_no_activity() {
		run_test(|| {
			initialize_substrate_bridge();
			assert_ok!(submit_finality_proof(1));
			assert_ok!(submit_finality_proof(2));

			next_block();
			next_block();

			next_block();
			assert_ok!(submit_finality_proof(5));
			assert_ok!(submit_finality_proof(7));
		})
	}

	#[test]
	fn should_prune_headers_over_headers_to_keep_parameter() {
		let default_gateway: ChainId = *b"gate";

		run_test(|| {
			initialize_substrate_bridge();
			assert_ok!(submit_finality_proof(1));
			let first_header = Pallet::<TestRuntime>::best_finalized_map(default_gateway);
			next_block();

			assert_ok!(submit_finality_proof(2));
			next_block();
			assert_ok!(submit_finality_proof(3));
			next_block();
			assert_ok!(submit_finality_proof(4));
			next_block();
			assert_ok!(submit_finality_proof(5));
			next_block();

			assert_ok!(submit_finality_proof(6));

			assert!(
				!Pallet::<TestRuntime>::is_known_header(first_header.hash(), default_gateway),
				"First header should be pruned."
			);
		})
	}
}
