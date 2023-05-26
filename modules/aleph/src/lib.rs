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



// This pallet is largely based on the `GRANDPA` pallet, is changed to work with AlephBFT

//! Aleph bridging Pallet
//!
//! This pallet is an on-chain AlephBFT light client.
//!
//! Right now, it does not support submiting finality proofs, but only
//! being directly initialized with a header and authority set.

#![cfg_attr(not(feature = "std"), no_std)]
// Runtime-generated enums
#![allow(clippy::large_enum_variant)]

use bp_runtime::{BlockNumberOf, HashOf, HasherOf, HeaderId, HeaderOf, OwnedBridgeModule};
use bp_header_chain::{HeaderChain, StoredHeaderData, StoredHeaderDataBuilder};
use bp_aleph_header_chain::{ChainWithAleph, InitializationData};
use frame_support::sp_runtime::traits::Header;

mod storage_types;
#[cfg(test)]
mod mock;

use storage_types::StoredAuthoritySet;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

// Re-export in crate namespace for `construct_runtime!`
pub use pallet::*;

/// The target that will be used when publishing logs related to this pallet.
pub const LOG_TARGET: &str = "runtime::bridge-aleph";

/// Bridged chain from the pallet configuration.
pub type BridgedChain<T, I> = <T as Config<I>>::BridgedChain;
/// Block number of the bridged chain.
pub type BridgedBlockNumber<T, I> = BlockNumberOf<<T as Config<I>>::BridgedChain>;
/// Block hash of the bridged chain.
pub type BridgedBlockHash<T, I> = HashOf<<T as Config<I>>::BridgedChain>;
/// Block id of the bridged chain.
pub type BridgedBlockId<T, I> = HeaderId<BridgedBlockHash<T, I>, BridgedBlockNumber<T, I>>;
/// Hasher of the bridged chain.
pub type BridgedBlockHasher<T, I> = HasherOf<<T as Config<I>>::BridgedChain>;
/// Header of the bridged chain.
pub type BridgedHeader<T, I> = HeaderOf<<T as Config<I>>::BridgedChain>;
/// Header data of the bridged chain that is stored at this chain by this pallet.
pub type BridgedStoredHeaderData<T, I> =
	StoredHeaderData<BridgedBlockNumber<T, I>, BridgedBlockHash<T, I>>;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use bp_runtime::BasicOperatingMode;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config {
		type RuntimeEvent: From<Event<Self, I>>
			+ IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type BridgedChain: ChainWithAleph;
		#[pallet::constant]
		type HeadersToKeep: Get<u32>;
	}

	#[pallet::pallet]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::hooks]
	impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I> {
		fn on_initialize(_n: BlockNumberFor<T>) -> Weight {
			Weight::zero()
		}
	}

	impl<T: Config<I>, I: 'static> OwnedBridgeModule<T> for Pallet<T, I> {
		const LOG_TARGET: &'static str = LOG_TARGET;
		type OwnerStorage = PalletOwner<T, I>;
		type OperatingMode = BasicOperatingMode;
		type OperatingModeStorage = PalletOperatingMode<T, I>;
	}

	#[pallet::call]
	impl<T: Config<I>, I: 'static> Pallet<T, I> {

		/// Bootstrap the bridge pallet with an initial header and authority set from which to sync.
		///
		/// The initial configuration provided does not need to be the genesis header of the bridged
		/// chain, it can be any arbitrary header. You can also provide the next scheduled set
		/// change if it is already know.
		///
		/// This function is only allowed to be called from a trusted origin and writes to storage
		/// with practically no checks in terms of the validity of the data. It is important that
		/// you ensure that valid data is being passed in.
		/// 
		/// Difference with GRANDPA: This function can only be called by root.
		/// 
		/// Note: It cannot be called once the bridge has been initialized.
		/// To reinitialize the bridge, you must reinitialize the pallet.
		#[pallet::call_index(1)]
		#[pallet::weight((T::DbWeight::get().reads_writes(2, 5), DispatchClass::Operational))]
		pub fn initialize(
			origin: OriginFor<T>,
			init_data: super::InitializationData<BridgedHeader<T, I>>,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;

			let init_allowed = !<BestFinalized<T, I>>::exists();
			ensure!(init_allowed, <Error<T, I>>::AlreadyInitialized);
			initialize_bridge::<T, I>(init_data.clone())?;

			log::info!(
				target: LOG_TARGET,
				"Pallet has been initialized with the following parameters: {:?}",
				init_data
			);

			Ok(().into())
		}

		#[pallet::call_index(2)]
		#[pallet::weight((T::DbWeight::get().reads_writes(1, 1), DispatchClass::Operational))]
		pub fn set_operating_mode(
			origin: OriginFor<T>,
			operating_mode: BasicOperatingMode,
		) -> DispatchResult {
			<Self as OwnedBridgeModule<_>>::set_operating_mode(origin, operating_mode)
		}
	}

	/// Hash of the best finalized header.
	#[pallet::storage]
	#[pallet::getter(fn best_finalized)]
	pub type BestFinalized<T: Config<I>, I: 'static = ()> =
		StorageValue<_, BridgedBlockId<T, I>, OptionQuery>;

	/// A ring buffer of imported hashes. Ordered by the insertion time.
	#[pallet::storage]
	pub(super) type ImportedHashes<T: Config<I>, I: 'static = ()> = StorageMap<
		Hasher = Identity,
		Key = u32,
		Value = BridgedBlockHash<T, I>,
		QueryKind = OptionQuery,
		OnEmpty = GetDefault,
		MaxValues = MaybeHeadersToKeep<T, I>,
	>;

	/// Current ring buffer position.
	#[pallet::storage]
	pub(super) type ImportedHashesPointer<T: Config<I>, I: 'static = ()> =
		StorageValue<_, u32, ValueQuery>;

	/// Relevant fields of imported headers.
	#[pallet::storage]
	pub type ImportedHeaders<T: Config<I>, I: 'static = ()> = StorageMap<
		Hasher = Identity,
		Key = BridgedBlockHash<T, I>,
		Value = BridgedStoredHeaderData<T, I>,
		QueryKind = OptionQuery,
		OnEmpty = GetDefault,
		MaxValues = MaybeHeadersToKeep<T, I>,
	>;

	/// The current Aleph Authority set.
	#[pallet::storage]
	pub type CurrentAuthoritySet<T: Config<I>, I: 'static = ()> =
		StorageValue<_, StoredAuthoritySet<T, I>, ValueQuery>;

	/// Optional pallet owner.
	///
	/// Pallet owner has a right to halt all pallet operations and then resume it. If it is
	/// `None`, then there are no direct ways to halt/resume pallet operations, but other
	/// runtime methods may still be used to do that (i.e. democracy::referendum to update halt
	/// flag directly or call the `halt_operations`).
	#[pallet::storage]
	pub type PalletOwner<T: Config<I>, I: 'static = ()> =
		StorageValue<_, T::AccountId, OptionQuery>;

	/// The current operating mode of the pallet.
	///
	/// Depending on the mode either all, or no transactions will be allowed.
	#[pallet::storage]
	pub type PalletOperatingMode<T: Config<I>, I: 'static = ()> =
		StorageValue<_, BasicOperatingMode, ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config<I>, I: 'static = ()> {
		pub owner: Option<T::AccountId>,
		pub init_data: Option<super::InitializationData<BridgedHeader<T, I>>>,
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
				initialize_bridge::<T, I>(init_data).expect("genesis config is correct; qed");
			} else {
				// Since the bridge hasn't been initialized we shouldn't allow anyone to perform
				// transactions.
				<PalletOperatingMode<T, I>>::put(BasicOperatingMode::Halted);
			}
		}
	}

	#[pallet::event]
	pub enum Event<T: Config<I>, I: 'static = ()> {
		/// Best finalized chain header has been updated to the header with given number and hash.
		UpdatedBestFinalizedHeader {
			number: BridgedBlockNumber<T, I>,
			hash: BridgedBlockHash<T, I>,
		},
	}

	#[pallet::error]
	pub enum Error<T, I = ()> {
		/// The header being imported is older than the best finalized header known to the pallet.
		OldHeader,
		/// The pallet is not yet initialized.
		NotInitialized,
		/// The pallet has already been initialized.
		AlreadyInitialized,
		/// Too many authorities in the set.
		TooManyAuthoritiesInSet,
		/// Error generated by the `OwnedBridgeModule` trait.
		BridgeModule(bp_runtime::OwnedBridgeModuleError),
	}


	/// Import a previously verified header to the storage.
	///
	/// Note this function solely takes care of updating the storage and pruning old entries,
	/// but does not verify the validity of such import.
	pub(crate) fn insert_header<T: Config<I>, I: 'static>(
		header: BridgedHeader<T, I>,
		hash: BridgedBlockHash<T, I>,
	) {
		let index = <ImportedHashesPointer<T, I>>::get();
		let pruning = <ImportedHashes<T, I>>::try_get(index);
		<BestFinalized<T, I>>::put(HeaderId(*header.number(), hash));
		<ImportedHeaders<T, I>>::insert(hash, header.build());
		<ImportedHashes<T, I>>::insert(index, hash);

		// Update ring buffer pointer and remove old header.
		<ImportedHashesPointer<T, I>>::put((index + 1) % T::HeadersToKeep::get());
		if let Ok(hash) = pruning {
			log::debug!(target: LOG_TARGET, "Pruning old header: {:?}.", hash);
			<ImportedHeaders<T, I>>::remove(hash);
		}
	}

	/// Since this writes to storage with no real checks this should only be used in functions that
	/// were called by a trusted origin.
	pub(crate) fn initialize_bridge<T: Config<I>, I: 'static>(
		init_params: super::InitializationData<BridgedHeader<T, I>>,
	) -> Result<(), Error<T, I>> {
		let super::InitializationData { header, authority_list, operating_mode } =
			init_params;
		let authority_set_length = authority_list.len();
		let authority_set = StoredAuthoritySet::<T, I>::try_new(authority_list)
			.map_err(|e| {
				log::error!(
					target: LOG_TARGET,
					"Failed to initialize bridge. Number of authorities in the set {} is larger than the configured value {}",
					authority_set_length,
					T::BridgedChain::MAX_AUTHORITIES_COUNT,
				);
				e
			})?;
		let initial_hash = header.hash();

		<ImportedHashesPointer<T, I>>::put(0);
		insert_header::<T, I>(*header, initial_hash);

		<CurrentAuthoritySet<T, I>>::put(authority_set);
		<PalletOperatingMode<T, I>>::put(operating_mode);

		Ok(())
	}

	/// Adapter for using `Config::HeadersToKeep` as `MaxValues` bound in our storage maps.
	pub struct MaybeHeadersToKeep<T, I>(PhantomData<(T, I)>);

	// this implementation is required to use the struct as `MaxValues`
	impl<T: Config<I>, I: 'static> Get<Option<u32>> for MaybeHeadersToKeep<T, I> {
		fn get() -> Option<u32> {
			Some(T::HeadersToKeep::get())
		}
	}

	/// Initialize pallet so that it is ready for inserting new header.
	///
	/// The function makes sure that the new insertion will cause the pruning of some old header.
	///
	/// Returns parent header for the new header.
	#[cfg(feature = "runtime-benchmarks")]
	pub(crate) fn bootstrap_bridge<T: Config<I>, I: 'static>(
		init_params: super::InitializationData<BridgedHeader<T, I>>,
	) -> BridgedHeader<T, I> {
		let start_header = init_params.header.clone();
		initialize_bridge::<T, I>(init_params).expect("benchmarks are correct");

		// the most obvious way to cause pruning during next insertion would be to insert
		// `HeadersToKeep` headers. But it'll make our benchmarks slow. So we will just play with
		// our pruning ring-buffer.
		assert_eq!(ImportedHashesPointer::<T, I>::get(), 1);
		ImportedHashesPointer::<T, I>::put(0);

		*start_header
	}
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
	/// Get the best finalized block number.
	pub fn best_finalized_number() -> Option<BridgedBlockNumber<T, I>> {
		BestFinalized::<T, I>::get().map(|id| id.number())
	}
}

/// Bridge Aleph pallet as header chain.
pub type AlephChainHeaders<T, I> = Pallet<T, I>;

impl<T: Config<I>, I: 'static> HeaderChain<BridgedChain<T, I>> for AlephChainHeaders<T, I> {
	fn finalized_header_state_root(
		header_hash: HashOf<BridgedChain<T, I>>,
	) -> Option<HashOf<BridgedChain<T, I>>> {
		ImportedHeaders::<T, I>::get(header_hash).map(|h| h.state_root)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{
		run_test, test_header, RuntimeOrigin, System,
		TestHeader, TestRuntime, MAX_BRIDGED_AUTHORITIES,
	};
	use bp_aleph_header_chain::AuthorityId;
	use bp_runtime::BasicOperatingMode;
	use bp_test_utils::{
		generate_owned_bridge_module_tests,
		ALICE, BOB, CHARLIE, Account,
	};
	use frame_support::{
		assert_noop, assert_ok,
		dispatch::PostDispatchInfo,
	};
	use sp_runtime::DispatchError;

	fn into_authority_set(accounts: Vec<Account>) -> Vec<AuthorityId> {
		accounts.into_iter().map(|a| AuthorityId::from(a)).collect()
	}

	fn initialize_substrate_bridge() {
		System::set_block_number(1);
		System::reset_events();

		assert_ok!(init_with_origin(RuntimeOrigin::root()));
	}

	fn init_with_origin(
		origin: RuntimeOrigin,
	) -> Result<
		InitializationData<TestHeader>,
		sp_runtime::DispatchErrorWithPostInfo<PostDispatchInfo>,
	> {
		let genesis = test_header(0);

		let init_data = InitializationData {
			header: Box::new(genesis),
			authority_list: into_authority_set(vec![ALICE, BOB, CHARLIE]),
			operating_mode: BasicOperatingMode::Normal,
		};

		Pallet::<TestRuntime>::initialize(origin, init_data.clone()).map(|_| init_data)
	}

	generate_owned_bridge_module_tests!(BasicOperatingMode::Normal, BasicOperatingMode::Halted);

	#[test]
	fn init_root_origin_can_initialize_pallet() {
		run_test(|| {
			assert_ok!(init_with_origin(RuntimeOrigin::root()));
		})
	}

	#[test]
	fn init_random_user_cannot_initialize_pallet() {
		run_test(|| {
			assert_noop!(init_with_origin(RuntimeOrigin::signed(1)), DispatchError::BadOrigin);
		})
	}

	#[test]
	fn init_owner_cannot_initialize_pallet() {
		run_test(|| {
			PalletOwner::<TestRuntime>::put(2);
			assert_noop!(init_with_origin(RuntimeOrigin::signed(2)), DispatchError::BadOrigin);
		})
	}

	#[test]
	fn init_storage_entries_are_correctly_initialized() {
		run_test(|| {
			assert_eq!(BestFinalized::<TestRuntime>::get(), None,);
			assert_eq!(Pallet::<TestRuntime>::best_finalized(), None);
			assert_eq!(PalletOperatingMode::<TestRuntime>::try_get(), Err(()));

			let init_data = init_with_origin(RuntimeOrigin::root()).unwrap();

			assert!(<ImportedHeaders<TestRuntime>>::contains_key(init_data.header.hash()));
			assert_eq!(BestFinalized::<TestRuntime>::get().unwrap().1, init_data.header.hash());
			assert_eq!(
				CurrentAuthoritySet::<TestRuntime>::get().authorities,
				init_data.authority_list
			);
			assert_eq!(
				PalletOperatingMode::<TestRuntime>::try_get(),
				Ok(BasicOperatingMode::Normal)
			);
		})
	}

	#[test]
	fn init_can_only_initialize_pallet_once() {
		run_test(|| {
			initialize_substrate_bridge();
			assert_noop!(
				init_with_origin(RuntimeOrigin::root()),
				<Error<TestRuntime>>::AlreadyInitialized
			);
		})
	}

	#[test]
	fn init_fails_if_there_are_too_many_authorities_in_the_set() {
		run_test(|| {
			let genesis = test_header(0);
			let init_data = InitializationData {
				header: Box::new(genesis),
				authority_list: into_authority_set((0..(MAX_BRIDGED_AUTHORITIES as u16)+1).map(|x| Account(x)).collect()),
				operating_mode: BasicOperatingMode::Normal,
			};

			assert_noop!(
				Pallet::<TestRuntime>::initialize(RuntimeOrigin::root(), init_data),
				Error::<TestRuntime>::TooManyAuthoritiesInSet,
			);
		});
	}
}
