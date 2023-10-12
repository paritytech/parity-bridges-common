// Copyright (C) Parity Technologies (UK) Ltd.
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

//! Runtime module that is used to store relayer rewards and to coordinate relations
//! between relayers.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

use bp_messages::LaneId;
use bp_relayers::{
	LaneRelayersSet, PaymentProcedure, Registration, RelayerRewardsKeyProvider, RewardAtSource,
	RewardsAccountParams, StakeAndSlash,
};
use bp_runtime::StorageDoubleMapKeyProvider;
use frame_support::fail;
use frame_system::Pallet as SystemPallet;
use sp_arithmetic::traits::{AtLeast32BitUnsigned, Zero};
use sp_runtime::{traits::CheckedSub, Saturating};
use sp_std::marker::PhantomData;

pub use pallet::*;
pub use payment_adapter::DeliveryConfirmationPaymentsAdapter;
pub use stake_adapter::StakeAndSlashNamed;
pub use weights::WeightInfo;
pub use weights_ext::WeightInfoExt;

mod mock;
mod payment_adapter;
mod stake_adapter;
mod weights_ext;

pub mod benchmarking;
pub mod extension;
pub mod weights;

/// The target that will be used when publishing logs related to this pallet.
pub const LOG_TARGET: &str = "runtime::bridge-relayers";

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// `RelayerRewardsKeyProvider` for given configuration.
	type RelayerRewardsKeyProviderOf<T> =
		RelayerRewardsKeyProvider<<T as frame_system::Config>::AccountId, <T as Config>::Reward>;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		/// Type of relayer reward, that is paid at this chain.
		type Reward: AtLeast32BitUnsigned + Copy + Member + Parameter + MaxEncodedLen;
		/// Pay rewards scheme.
		type PaymentProcedure: PaymentProcedure<Self::AccountId, Self::Reward>;

		/// Stake and slash scheme.
		type StakeAndSlash: StakeAndSlash<Self::AccountId, BlockNumberFor<Self>, Self::Reward>;

		/// Maximal number of lanes, where relayer may be registered.
		///
		/// This is an artificial limit that only exists to make PoV size predictable.
		#[pallet::constant]
		type MaxLanesPerRelayer: Get<u32>;
		/// Maximal number of relayers that can reside in the active lane relayers set on a single
		/// lane.
		///
		/// Lowering this value leads to additional concurrency between relayers, potentially
		/// making messages cheaper. So it shall not be too large.
		#[pallet::constant]
		type MaxActiveRelayersPerLane: Get<u32>;
		/// Maximal number of relayers that can reside in the next lane relayers set on a single
		/// lane.
		///
		/// Relayers set is a bounded priority queue, where relayers with lower expected reward are
		/// prioritized over greedier relayers. At the end of epoch, we select top
		/// `MaxActiveRelayersPerLane` relayers from the next set and move them to the next set. To
		/// alleviate possible spam attacks, where relayers are registering at lane with zero reward
		/// (pushing out actual relayers with larger expected reward) and then deregistering
		/// themselves right before epoch end, we make the next relayers set larger than the active
		/// set. It would make it more expensive for attackers to fill the whole next set.
		///
		/// This value must be larger than or equal to the [`Self::MaxActiveRelayersPerLane`].
		#[pallet::constant]
		type MaxNextRelayersPerLane: Get<u32>;

		/// Length of initial relayer elections in chain blocks.
		///
		/// When the first relayer registers itself on the lane, we give some time to other relayers
		/// to register as well. Otherwise there'll be only one relayer in active set, for the whole
		/// (first) epoch.
		type InitialElectionLength: Get<BlockNumberFor<Self>>;
		/// Length of slots in chain blocks.
		///
		/// Registered relayer may explicitly register himself at some lane to get priority boost
		/// for message delivery transactions on that lane (that is done using `register_at_lane`
		/// pallet call). All relayers, registered at the lane form an ordered queue and only
		/// relayer at the head of that queue receives a boost at his slot (which has a length of
		/// `SlotLength` blocks). Then the "best" relayer is removed and pushed to the tail of the
		/// queue and next relayer gets the boost during next `SlotLength` blocks. And so on...
		///
		/// Shall not be too low to have an effect, because there's some (at least one block) lag
		/// between moments when priority is computed and when active slot changes.
		type SlotLength: Get<BlockNumberFor<Self>>;
		/// Length of epoch in chain blocks.
		///
		/// Epoch is a set of slots, where a fixed set of lane relayers receives a priority boost
		/// for their message delivery transactions. Epochs transition is a manual action, performed
		/// by the `advance_lane_epoch` call.
		///
		/// This value should allow every relayer from the active set to have at least one slot. So
		/// it shall be not less than the `Self::MaxActiveRelayersPerLane::get() *
		/// Self::SlotLength::get()`. Normally, it should allow more than one slot for each relayer
		/// (given max relayers in the set).
		type EpochLength: Get<BlockNumberFor<Self>>;

		/// Priority boost that the registered relayer gets for every additional message in the
		/// message delivery transaction.
		type PriorityBoostPerMessage: Get<TransactionPriority>;
		/// Additional priority boost, that is added to regular `PriorityBoostPerMessage` boost for
		/// message delivery transactions, submitted by relayer at the head of the lane relayers
		/// queue.
		///
		/// In other words, if relayer has registered at some `lane`, using `register_at_lane` call
		/// AND he is currently at the head of the lane relayers queue, his message delivery
		/// transaction will get the following additional priority boost:
		///
		/// ```nocompile
		/// T::PriorityBoostForActiveLaneRelayer::get() + T::PriorityBoostPerMessage::get() * (msgs - 1)
		/// ```
		type PriorityBoostForActiveLaneRelayer: Get<TransactionPriority>;

		/// Pallet call weights.
		type WeightInfo: WeightInfoExt;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Claim accumulated rewards.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::claim_rewards())]
		pub fn claim_rewards(
			origin: OriginFor<T>,
			rewards_account_params: RewardsAccountParams,
		) -> DispatchResult {
			let relayer = ensure_signed(origin)?;

			RelayerRewards::<T>::try_mutate_exists(
				&relayer,
				rewards_account_params,
				|maybe_reward| -> DispatchResult {
					let reward = maybe_reward.take().ok_or(Error::<T>::NoRewardForRelayer)?;
					T::PaymentProcedure::pay_reward(&relayer, rewards_account_params, reward)
						.map_err(|e| {
							log::trace!(
								target: LOG_TARGET,
								"Failed to pay {:?} rewards to {:?}: {:?}",
								rewards_account_params,
								relayer,
								e,
							);
							Error::<T>::FailedToPayReward
						})?;

					Self::deposit_event(Event::<T>::RewardPaid {
						relayer: relayer.clone(),
						rewards_account_params,
						reward,
					});
					Ok(())
				},
			)
		}

		/// Register relayer or update its registration.
		///
		/// Registration allows relayer to get priority boost for its message delivery transactions.
		/// Honest block authors will choose prioritized transactions when there are transactions
		/// from registered and unregistered relayers. However, registered relayers take additional
		/// responsibility to submit only valid transactions. If they submit an invalid transaction,
		/// their stake will be slashed and registration will be lost.
		///
		/// Relayers may get additional priority boost by registering their intention to relay
		/// messages at given lanes, using `register_at_lane` method.
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::register())]
		pub fn register(origin: OriginFor<T>, valid_till: BlockNumberFor<T>) -> DispatchResult {
			let relayer = ensure_signed(origin)?;

			// valid till must be larger than the current block number and the lease must be larger
			// than the `RequiredRegistrationLease`
			let lease = valid_till.saturating_sub(frame_system::Pallet::<T>::block_number());
			ensure!(
				lease > Pallet::<T>::required_registration_lease(),
				Error::<T>::InvalidRegistrationLease
			);

			RegisteredRelayers::<T>::try_mutate(&relayer, |maybe_registration| -> DispatchResult {
				let mut registration =
					maybe_registration.take().unwrap_or_else(|| Registration::new(valid_till));

				// new `valid_till` must be larger (or equal) than the old one
				ensure!(
					valid_till >= registration.valid_till_ignore_lanes(),
					Error::<T>::CannotReduceRegistrationLease,
				);
				registration.set_valid_till(valid_till);

				// reserve stake on relayer account
				registration.set_stake(Self::update_relayer_stake(
					&relayer,
					registration.current_stake(),
					registration.required_stake(Self::base_stake(), Self::stake_per_lane()),
				)?);

				log::trace!(target: LOG_TARGET, "Successfully registered relayer: {:?}", relayer);
				Self::deposit_event(Event::<T>::RegistrationUpdated {
					relayer: relayer.clone(),
					registration: registration.clone(),
				});

				*maybe_registration = Some(registration);

				Ok(())
			})
		}

		/// `Deregister` relayer.
		///
		/// After this call, message delivery transactions of the relayer won't get any priority
		/// boost. Keep in mind that the relayer can't deregister until `valid_till` block, which
		/// he has specified in the registration call. The relayer is also unregistered from all
		/// lanes, where he has explicitly registered using `register_at_lane`.
		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::deregister())]
		pub fn deregister(origin: OriginFor<T>) -> DispatchResult {
			let relayer = ensure_signed(origin)?;

			RegisteredRelayers::<T>::try_mutate(&relayer, |maybe_registration| -> DispatchResult {
				let registration = match maybe_registration.take() {
					Some(registration) => registration,
					None => fail!(Error::<T>::NotRegistered),
				};

				// we can't deregister until `valid_till + 1` block and while relayer has active
				// lane registerations
				ensure!(
					registration
						.valid_till()
						.map(|valid_till| valid_till < frame_system::Pallet::<T>::block_number())
						.unwrap_or(false),
					Error::<T>::RegistrationIsStillActive,
				);

				// if stake is non-zero, we should do unreserve
				Self::update_relayer_stake(&relayer, registration.current_stake(), Zero::zero())?;

				log::trace!(target: LOG_TARGET, "Successfully deregistered relayer: {:?}", relayer);
				Self::deposit_event(Event::<T>::Deregistered { relayer: relayer.clone() });

				*maybe_registration = None;

				Ok(())
			})
		}

		/// Register relayer intention to serve given messages lane.
		///
		/// Relayer that registers itself at given message lane gets a priority boost for his
		/// message delivery transactions, **verified** at his slots (consecutive range of blocks).
		///
		/// Every lane registration requires additional stake. Relayer registration is considered
		/// active while it is registered at least at one lane.
		///
		/// Relayer may request large reward here (using `expected_reward`), but in the end, the
		/// reward amount is computed at the bridged (source chain). In the case if
		/// [`DeliveryConfirmationPaymentsAdapter`] is used to register rewards, the maximal reward
		/// per message is limited by the `MaxRewardPerMessage` parameter.
		#[pallet::call_index(3)]
		#[pallet::weight(Weight::zero())] // TODO
		pub fn register_at_lane(
			origin: OriginFor<T>,
			lane: LaneId,
			expected_reward: RewardAtSource,
		) -> DispatchResult {
			let relayer = ensure_signed(origin)?;

			RegisteredRelayers::<T>::try_mutate(
				&relayer.clone(),
				move |maybe_registration| -> DispatchResult {
					// we only allow registered relayers to have priority boosts
					let mut registration = match maybe_registration.take() {
						Some(registration) => registration,
						None => fail!(Error::<T>::NotRegistered),
					};

					// cannot add another lane registration if relayer has already max allowed
					// lane registrations OR if it is already registered at that lane
					ensure!(
						registration.register_at_lane(lane),
						Error::<T>::FailedToRegisterAtLane
					);

					// let's try to claim a slot in the next set
					LaneRelayers::<T>::try_mutate(lane, |maybe_lane_relayers| {
						let mut lane_relayers = match maybe_lane_relayers.take() {
							Some(lane_relayers) => lane_relayers,
							None => LaneRelayersSet::empty(
								SystemPallet::<T>::block_number()
									.saturating_add(T::InitialElectionLength::get()),
							),
						};

						ensure!(
							lane_relayers.next_set_try_push(relayer.clone(), expected_reward),
							Error::<T>::TooLargeRewardToOccupyAnEntry,
						);

						*maybe_lane_relayers = Some(lane_relayers);

						Ok::<_, Error<T>>(())
					})?;

					// the relayer need to stake additional amount for every additional lane
					registration.set_stake(Self::update_relayer_stake(
						&relayer,
						registration.current_stake(),
						registration.required_stake(Self::base_stake(), Self::stake_per_lane()),
					)?);

					*maybe_registration = Some(registration);

					Ok(())
				},
			)?;

			Ok(())
		}

		/// TODO
		#[pallet::call_index(4)]
		#[pallet::weight(Weight::zero())] // TODO
		pub fn deregister_at_lane(origin: OriginFor<T>, lane: LaneId) -> DispatchResult {
			let relayer = ensure_signed(origin)?;

			RegisteredRelayers::<T>::try_mutate(
				&relayer.clone(),
				move |maybe_registration| -> DispatchResult {
					// if relayer doesn't have a basic registration, we know that he is not
					// registered at the lane as well
					let mut registration = match maybe_registration.take() {
						Some(registration) => registration,
						None => fail!(Error::<T>::NotRegistered),
					};

					// remove relayer from the next set. It still may be in the active set
					ensure!(registration.deregister_at_lane(lane), Error::<T>::NotRegisteredAtLane);

					// remove relayer from the `next_set` of lane relayers. Relayer still remains
					// in the active set until current epoch ends
					LaneRelayers::<T>::try_mutate(lane, |lane_relayers_ref| {
						let mut lane_relayers = match lane_relayers_ref.take() {
							Some(lane_relayers) => lane_relayers,
							None => fail!(Error::<T>::NotRegisteredAtLane),
						};

						// remove relayer from the next set if it is there
						lane_relayers.next_set_try_remove(&relayer);

						// make sure that the `valid_till` covers current epoch if relayer is in the
						// active lane relayers set
						let is_in_active_set = lane_relayers
							.active_relayers()
							.iter()
							.filter(|r| *r.relayer() == relayer)
							.next()
							.is_some();
						if is_in_active_set {
							registration.set_valid_till(sp_std::cmp::max(
								registration.valid_till_ignore_lanes(),
								lane_relayers
									.next_set_may_enact_at()
									.saturating_add(Self::required_registration_lease()),
							));
						}

						*lane_relayers_ref = Some(lane_relayers);

						Ok::<_, Error<T>>(())
					})?;

					*maybe_registration = Some(registration);

					Ok(())
				},
			)?;

			Ok(())
		}

		// TODO: add another `obsolete` extension for this call of the relayers pallet?
		/// Enact next set of relayers at a given lane.
		///
		/// This will replace the set of active relayers with the next scheduled set, for given
		/// lane. Anyone could call this method at any point. If the set will be changed, the cost
		/// of transaction will be refunded to the submitter. We do not provide any on-chain means
		/// to sync between relayers on who will submit this transaction, so first transaction from
		/// anyone will be accepted and it will have the zero cost. All subsequent transactions will
		/// be paid. We suggest the first relayer from the `next_set` to submit this transaction.
		#[pallet::call_index(5)]
		#[pallet::weight(Weight::zero())] // TODO
		pub fn advance_lane_epoch(origin: OriginFor<T>, lane: LaneId) -> DispatchResult {
			let _ = ensure_signed(origin)?;

			// remove relayer from the `next_set` of lane relayers. So relayer is still
			LaneRelayers::<T>::try_mutate(lane, |lane_relayers_ref| {
				let mut lane_relayers = match lane_relayers_ref.take() {
					Some(lane_relayers) => lane_relayers,
					None => fail!(Error::<T>::NoRelayersAtLane),
				};

				// ensure that the current block number allows us to enact next set
				let current_block_number = SystemPallet::<T>::block_number();
				ensure!(
					lane_relayers.next_set_may_enact_at() >= current_block_number,
					Error::<T>::TooEarlyToActivateNextRelayersSet,
				);

				let new_next_set_may_enact_at =
					current_block_number.saturating_add(T::EpochLength::get());
				lane_relayers.activate_next_set(new_next_set_may_enact_at);

				*lane_relayers_ref = Some(lane_relayers);

				Ok::<_, Error<T>>(())
			})?;

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Returns true if given relayer registration is active at current block.
		///
		/// This call respects both `RequiredStake` and `RequiredRegistrationLease`, meaning that
		/// it'll return false if registered stake is lower than required or if remaining lease
		/// is less than `RequiredRegistrationLease`.
		pub fn is_registration_active(relayer: &T::AccountId) -> bool {
			match Self::registered_relayer(relayer) {
				Some(registration) => registration.is_active(
					SystemPallet::<T>::block_number(),
					Self::required_registration_lease(),
				),
				None => false,
			}
		}

		/// Slash and `deregister` relayer. This function slashes all staked balance.
		///
		/// It may fail inside, but error is swallowed and we only log it.
		pub fn slash_and_deregister(
			relayer: &T::AccountId,
			slash_destination: RewardsAccountParams,
		) {
			// TODO: also remove from all lanes?

			let registration = match RegisteredRelayers::<T>::take(relayer) {
				Some(registration) => registration,
				None => {
					log::trace!(
						target: crate::LOG_TARGET,
						"Cannot slash unregistered relayer {:?}",
						relayer,
					);

					return
				},
			};

			match T::StakeAndSlash::repatriate_reserved(
				relayer,
				slash_destination,
				registration.current_stake(),
			) {
				Ok(failed_to_slash) if failed_to_slash.is_zero() => {
					log::trace!(
						target: crate::LOG_TARGET,
						"Relayer account {:?} has been slashed for {:?}. Funds were deposited to {:?}",
						relayer,
						registration.current_stake(),
						slash_destination,
					);
				},
				Ok(failed_to_slash) => {
					log::trace!(
						target: crate::LOG_TARGET,
						"Relayer account {:?} has been partially slashed for {:?}. Funds were deposited to {:?}. \
						Failed to slash: {:?}",
						relayer,
						registration.current_stake(),
						slash_destination,
						failed_to_slash,
					);
				},
				Err(e) => {
					// TODO: document this. Where?

					// it may fail if there's no beneficiary account. For us it means that this
					// account must exists before we'll deploy the bridge
					log::debug!(
						target: crate::LOG_TARGET,
						"Failed to slash relayer account {:?}: {:?}. Maybe beneficiary account doesn't exist? \
						Beneficiary: {:?}, amount: {:?}, failed to slash: {:?}",
						relayer,
						e,
						slash_destination,
						registration.current_stake(),
						registration.current_stake(),
					);
				},
			}
		}

		/// Register reward for given relayer.
		pub fn register_relayer_reward(
			rewards_account_params: RewardsAccountParams,
			relayer: &T::AccountId,
			reward: T::Reward,
		) {
			if reward.is_zero() {
				return
			}

			RelayerRewards::<T>::mutate(
				relayer,
				rewards_account_params,
				|old_reward: &mut Option<T::Reward>| {
					let new_reward = old_reward.unwrap_or_else(Zero::zero).saturating_add(reward);
					*old_reward = Some(new_reward);

					log::trace!(
						target: crate::LOG_TARGET,
						"Relayer {:?} can now claim reward for serving payer {:?}: {:?}",
						relayer,
						rewards_account_params,
						new_reward,
					);
				},
			);
		}

		/// Return required registration lease.
		pub(crate) fn required_registration_lease() -> BlockNumberFor<T> {
			<T::StakeAndSlash as StakeAndSlash<
				T::AccountId,
				BlockNumberFor<T>,
				T::Reward,
			>>::RequiredRegistrationLease::get()
		}

		/// Return required base stake.
		pub(crate) fn base_stake() -> T::Reward {
			<T::StakeAndSlash as StakeAndSlash<
				T::AccountId,
				BlockNumberFor<T>,
				T::Reward,
			>>::RequiredStake::get()
		}

		/// Return required stake per lane.
		pub(crate) fn stake_per_lane() -> T::Reward {
			<T::StakeAndSlash as StakeAndSlash<
				T::AccountId,
				BlockNumberFor<T>,
				T::Reward,
			>>::RequiredLaneStake::get()
		}

		/// Update relayer stake.
		fn update_relayer_stake(
			relayer: &T::AccountId,
			current_stake: T::Reward,
			required_stake: T::Reward,
		) -> Result<T::Reward, sp_runtime::DispatchError> {
			// regarding stake, there are three options:
			// - if relayer stake is larger than required stake, we may do unreserve
			// - if relayer stake equals to required stake, we do nothing
			// - if relayer stake is smaller than required stake, we do additional reserve
			if let Some(to_unreserve) = current_stake.checked_sub(&required_stake) {
				let failed_to_unreserve = T::StakeAndSlash::unreserve(relayer, to_unreserve);
				if !failed_to_unreserve.is_zero() {
					log::trace!(
						target: LOG_TARGET,
						"Failed to unreserve {:?}/{:?} on relayer {:?} account",
						failed_to_unreserve,
						to_unreserve,
						relayer,
					);

					fail!(Error::<T>::FailedToUnreserve)
				}
			} else if let Some(to_reserve) = required_stake.checked_sub(&current_stake) {
				let reserve_result = T::StakeAndSlash::reserve(&relayer, to_reserve);
				if let Err(e) = reserve_result {
					log::trace!(
						target: LOG_TARGET,
						"Failed to reserve {:?} on relayer {:?} account: {:?}",
						to_reserve,
						relayer,
						e,
					);

					fail!(Error::<T>::FailedToReserve)
				}
			}

			Ok(required_stake)
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Reward has been paid to the relayer.
		RewardPaid {
			/// Relayer account that has been rewarded.
			relayer: T::AccountId,
			/// Relayer has received reward from this account.
			rewards_account_params: RewardsAccountParams,
			/// Reward amount.
			reward: T::Reward,
		},
		/// Relayer registration has been added or updated.
		RegistrationUpdated {
			/// Relayer account that has been registered.
			relayer: T::AccountId,
			/// Relayer registration.
			registration: Registration<BlockNumberFor<T>, T::Reward, T::MaxLanesPerRelayer>,
		},
		/// Relayer has been `deregistered`.
		Deregistered {
			/// Relayer account that has been `deregistered`.
			relayer: T::AccountId,
		},
		/// Relayer has been slashed and `deregistered`.
		SlashedAndDeregistered {
			/// Relayer account that has been `deregistered`.
			relayer: T::AccountId,
			/// Registration that was removed.
			registration: Registration<BlockNumberFor<T>, T::Reward, T::MaxLanesPerRelayer>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// No reward can be claimed by given relayer.
		NoRewardForRelayer,
		/// Reward payment procedure has failed.
		FailedToPayReward,
		/// The relayer has tried to register for past block or registration lease
		/// is too short.
		InvalidRegistrationLease,
		/// New registration lease is less than the previous one.
		CannotReduceRegistrationLease,
		/// Failed to reserve enough funds on relayer account.
		FailedToReserve,
		/// Failed to `unreserve` enough funds on relayer account.
		FailedToUnreserve,
		/// Cannot `deregister` if not registered.
		NotRegistered,
		/// Failed to `deregister` relayer, because lease is still active.
		RegistrationIsStillActive,
		/// Failed to perform required action, because relayer registration is inactive.
		RegistrationIsInactive,
		/// Relayer is trying to register twice on the same lane.
		DuplicateLaneRegistration,
		/// Relayer has too many lane registrations.
		FailedToRegisterAtLane,
		///
		TooManyScheduledChanges,
		///
		TooManyLaneRelayersAtLane,
		///
		TooLargeRewardToOccupyAnEntry,
		///
		NotRegisteredAtLane,
		///
		NoRelayersAtLane,
		///
		TooEarlyToActivateNextRelayersSet,
	}

	/// Map of the relayer => accumulated reward.
	#[pallet::storage]
	#[pallet::getter(fn relayer_reward)]
	pub type RelayerRewards<T: Config> = StorageDoubleMap<
		_,
		<RelayerRewardsKeyProviderOf<T> as StorageDoubleMapKeyProvider>::Hasher1,
		<RelayerRewardsKeyProviderOf<T> as StorageDoubleMapKeyProvider>::Key1,
		<RelayerRewardsKeyProviderOf<T> as StorageDoubleMapKeyProvider>::Hasher2,
		<RelayerRewardsKeyProviderOf<T> as StorageDoubleMapKeyProvider>::Key2,
		<RelayerRewardsKeyProviderOf<T> as StorageDoubleMapKeyProvider>::Value,
		OptionQuery,
	>;

	/// Relayers that have reserved some of their balance to get free priority boost
	/// for their message delivery transactions.
	///
	/// Other relayers may submit transactions as well, but they will have default
	/// priority and will be rejected (without significant tip) in case if registered
	/// relayer is present.
	#[pallet::storage]
	#[pallet::getter(fn registered_relayer)]
	pub type RegisteredRelayers<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Registration<BlockNumberFor<T>, T::Reward, T::MaxLanesPerRelayer>,
		OptionQuery,
	>;

	// TODO: split active set and `LaneRelayers`! Active set is read at every delivery transaction
	// and other fields we need only in pallet calls.

	// TODO: make it ValueQuery? After it is created, it is never removed. But it is not default

	/// A set of relayers that have explicitly registered themselves at a given lane.
	///
	/// Every relayer inside this set receives additional priority boost when it submits
	/// message delivers messages at given lane. The boost only happens inside the slot,
	/// assigned to relayer.
	#[pallet::storage]
	#[pallet::getter(fn lane_relayers)]
	pub type LaneRelayers<T: Config> = StorageMap<
		_,
		Identity,
		LaneId,
		LaneRelayersSet<
			T::AccountId,
			BlockNumberFor<T>,
			T::MaxActiveRelayersPerLane,
			T::MaxNextRelayersPerLane,
		>,
		OptionQuery,
	>;
}

#[cfg(test)]
mod tests {
	use super::*;
	use mock::{RuntimeEvent as TestEvent, *};

	use crate::Event::RewardPaid;
	use bp_messages::LaneId;
	use bp_relayers::{RelayerAndReward, RewardsAccountOwner};
	use frame_support::{
		assert_noop, assert_ok,
		traits::fungible::{Inspect, Mutate},
	};
	use frame_system::{EventRecord, Pallet as System, Phase};
	use sp_runtime::DispatchError;

	fn get_ready_for_events() {
		System::<TestRuntime>::set_block_number(1);
		System::<TestRuntime>::reset_events();
	}

	fn registration(
		valid_till: ThisChainBlockNumber,
		stake: ThisChainBalance,
	) -> Registration<ThisChainBlockNumber, ThisChainBalance, MaxLanesPerRelayer> {
		let mut registration = Registration::new(valid_till);
		registration.set_stake(stake);
		registration
	}

	#[test]
	fn root_cant_claim_anything() {
		run_test(|| {
			assert_noop!(
				Pallet::<TestRuntime>::claim_rewards(
					RuntimeOrigin::root(),
					test_reward_account_param()
				),
				DispatchError::BadOrigin,
			);
		});
	}

	#[test]
	fn relayer_cant_claim_if_no_reward_exists() {
		run_test(|| {
			assert_noop!(
				Pallet::<TestRuntime>::claim_rewards(
					RuntimeOrigin::signed(REGULAR_RELAYER),
					test_reward_account_param()
				),
				Error::<TestRuntime>::NoRewardForRelayer,
			);
		});
	}

	#[test]
	fn relayer_cant_claim_if_payment_procedure_fails() {
		run_test(|| {
			RelayerRewards::<TestRuntime>::insert(
				FAILING_RELAYER,
				test_reward_account_param(),
				100,
			);
			assert_noop!(
				Pallet::<TestRuntime>::claim_rewards(
					RuntimeOrigin::signed(FAILING_RELAYER),
					test_reward_account_param()
				),
				Error::<TestRuntime>::FailedToPayReward,
			);
		});
	}

	#[test]
	fn relayer_can_claim_reward() {
		run_test(|| {
			get_ready_for_events();

			RelayerRewards::<TestRuntime>::insert(
				REGULAR_RELAYER,
				test_reward_account_param(),
				100,
			);
			assert_ok!(Pallet::<TestRuntime>::claim_rewards(
				RuntimeOrigin::signed(REGULAR_RELAYER),
				test_reward_account_param()
			));
			assert_eq!(
				RelayerRewards::<TestRuntime>::get(REGULAR_RELAYER, test_reward_account_param()),
				None
			);

			// Check if the `RewardPaid` event was emitted.
			assert_eq!(
				System::<TestRuntime>::events().last(),
				Some(&EventRecord {
					phase: Phase::Initialization,
					event: TestEvent::BridgeRelayers(RewardPaid {
						relayer: REGULAR_RELAYER,
						rewards_account_params: test_reward_account_param(),
						reward: 100
					}),
					topics: vec![],
				}),
			);
		});
	}

	#[test]
	fn pay_reward_from_account_actually_pays_reward() {
		type Balances = pallet_balances::Pallet<TestRuntime>;
		type PayLaneRewardFromAccount =
			bp_relayers::PayRewardFromAccount<Balances, ThisChainAccountId>;

		run_test(|| {
			let in_lane_0 = RewardsAccountParams::new(
				LaneId::new(1, 2),
				*b"test",
				RewardsAccountOwner::ThisChain,
			);
			let out_lane_1 = RewardsAccountParams::new(
				LaneId::new(1, 3),
				*b"test",
				RewardsAccountOwner::BridgedChain,
			);

			let in_lane0_rewards_account = PayLaneRewardFromAccount::rewards_account(in_lane_0);
			let out_lane1_rewards_account = PayLaneRewardFromAccount::rewards_account(out_lane_1);

			Balances::mint_into(&in_lane0_rewards_account, 100).unwrap();
			Balances::mint_into(&out_lane1_rewards_account, 100).unwrap();
			assert_eq!(Balances::balance(&in_lane0_rewards_account), 100);
			assert_eq!(Balances::balance(&out_lane1_rewards_account), 100);
			assert_eq!(Balances::balance(&1), 0);

			PayLaneRewardFromAccount::pay_reward(&1, in_lane_0, 100).unwrap();
			assert_eq!(Balances::balance(&in_lane0_rewards_account), 0);
			assert_eq!(Balances::balance(&out_lane1_rewards_account), 100);
			assert_eq!(Balances::balance(&1), 100);

			PayLaneRewardFromAccount::pay_reward(&1, out_lane_1, 100).unwrap();
			assert_eq!(Balances::balance(&in_lane0_rewards_account), 0);
			assert_eq!(Balances::balance(&out_lane1_rewards_account), 0);
			assert_eq!(Balances::balance(&1), 200);
		});
	}

	#[test]
	fn register_fails_if_valid_till_is_a_past_block() {
		run_test(|| {
			System::<TestRuntime>::set_block_number(100);

			assert_noop!(
				Pallet::<TestRuntime>::register(RuntimeOrigin::signed(REGISTER_RELAYER), 50),
				Error::<TestRuntime>::InvalidRegistrationLease,
			);
		});
	}

	#[test]
	fn register_fails_if_valid_till_lease_is_less_than_required() {
		run_test(|| {
			System::<TestRuntime>::set_block_number(100);

			assert_noop!(
				Pallet::<TestRuntime>::register(
					RuntimeOrigin::signed(REGISTER_RELAYER),
					99 + Lease::get()
				),
				Error::<TestRuntime>::InvalidRegistrationLease,
			);
		});
	}

	#[test]
	fn register_works() {
		run_test(|| {
			get_ready_for_events();

			assert_ok!(Pallet::<TestRuntime>::register(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				150
			));
			assert_eq!(Balances::reserved_balance(REGISTER_RELAYER), Stake::get());
			assert_eq!(
				Pallet::<TestRuntime>::registered_relayer(REGISTER_RELAYER),
				Some(registration(150, Stake::get())),
			);

			assert_eq!(
				System::<TestRuntime>::events().last(),
				Some(&EventRecord {
					phase: Phase::Initialization,
					event: TestEvent::BridgeRelayers(Event::RegistrationUpdated {
						relayer: REGISTER_RELAYER,
						registration: registration(150, Stake::get()),
					}),
					topics: vec![],
				}),
			);
		});
	}

	#[test]
	fn register_fails_if_new_valid_till_is_lesser_than_previous() {
		run_test(|| {
			assert_ok!(Pallet::<TestRuntime>::register(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				150
			));

			assert_noop!(
				Pallet::<TestRuntime>::register(RuntimeOrigin::signed(REGISTER_RELAYER), 125),
				Error::<TestRuntime>::CannotReduceRegistrationLease,
			);
		});
	}

	#[test]
	fn register_fails_if_it_cant_unreserve_some_balance_if_required_stake_decreases() {
		run_test(|| {
			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER,
				registration(150, Stake::get() + 1),
			);

			assert_noop!(
				Pallet::<TestRuntime>::register(RuntimeOrigin::signed(REGISTER_RELAYER), 150),
				Error::<TestRuntime>::FailedToUnreserve,
			);
		});
	}

	#[test]
	fn register_unreserves_some_balance_if_required_stake_decreases() {
		run_test(|| {
			get_ready_for_events();

			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER,
				registration(150, Stake::get() + 1),
			);
			TestStakeAndSlash::reserve(&REGISTER_RELAYER, Stake::get() + 1).unwrap();
			assert_eq!(Balances::reserved_balance(REGISTER_RELAYER), Stake::get() + 1);
			let free_balance = Balances::free_balance(REGISTER_RELAYER);

			assert_ok!(Pallet::<TestRuntime>::register(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				150
			));
			assert_eq!(Balances::reserved_balance(REGISTER_RELAYER), Stake::get());
			assert_eq!(Balances::free_balance(REGISTER_RELAYER), free_balance + 1);
			assert_eq!(
				Pallet::<TestRuntime>::registered_relayer(REGISTER_RELAYER),
				Some(registration(150, Stake::get())),
			);

			assert_eq!(
				System::<TestRuntime>::events().last(),
				Some(&EventRecord {
					phase: Phase::Initialization,
					event: TestEvent::BridgeRelayers(Event::RegistrationUpdated {
						relayer: REGISTER_RELAYER,
						registration: registration(150, Stake::get()),
					}),
					topics: vec![],
				}),
			);
		});
	}

	#[test]
	fn register_fails_if_it_cant_reserve_some_balance() {
		run_test(|| {
			Balances::set_balance(&REGISTER_RELAYER, 0);
			assert_noop!(
				Pallet::<TestRuntime>::register(RuntimeOrigin::signed(REGISTER_RELAYER), 150),
				Error::<TestRuntime>::FailedToReserve,
			);
		});
	}

	#[test]
	fn register_fails_if_it_cant_reserve_some_balance_if_required_stake_increases() {
		run_test(|| {
			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER,
				registration(150, Stake::get() - 1),
			);
			Balances::set_balance(&REGISTER_RELAYER, 0);

			assert_noop!(
				Pallet::<TestRuntime>::register(RuntimeOrigin::signed(REGISTER_RELAYER), 150),
				Error::<TestRuntime>::FailedToReserve,
			);
		});
	}

	#[test]
	fn register_reserves_some_balance_if_required_stake_increases() {
		run_test(|| {
			get_ready_for_events();

			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER,
				registration(150, Stake::get() - 1),
			);
			TestStakeAndSlash::reserve(&REGISTER_RELAYER, Stake::get() - 1).unwrap();

			let free_balance = Balances::free_balance(REGISTER_RELAYER);
			assert_ok!(Pallet::<TestRuntime>::register(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				150
			));
			assert_eq!(Balances::reserved_balance(REGISTER_RELAYER), Stake::get());
			assert_eq!(Balances::free_balance(REGISTER_RELAYER), free_balance - 1);
			assert_eq!(
				Pallet::<TestRuntime>::registered_relayer(REGISTER_RELAYER),
				Some(registration(150, Stake::get())),
			);

			assert_eq!(
				System::<TestRuntime>::events().last(),
				Some(&EventRecord {
					phase: Phase::Initialization,
					event: TestEvent::BridgeRelayers(Event::RegistrationUpdated {
						relayer: REGISTER_RELAYER,
						registration: registration(150, Stake::get()),
					}),
					topics: vec![],
				}),
			);
		});
	}

	#[test]
	fn deregister_fails_if_not_registered() {
		run_test(|| {
			assert_noop!(
				Pallet::<TestRuntime>::deregister(RuntimeOrigin::signed(REGISTER_RELAYER)),
				Error::<TestRuntime>::NotRegistered,
			);
		});
	}

	#[test]
	fn deregister_fails_if_registration_is_still_active() {
		run_test(|| {
			assert_ok!(Pallet::<TestRuntime>::register(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				150
			));

			System::<TestRuntime>::set_block_number(100);

			assert_noop!(
				Pallet::<TestRuntime>::deregister(RuntimeOrigin::signed(REGISTER_RELAYER)),
				Error::<TestRuntime>::RegistrationIsStillActive,
			);
		});
	}

	#[test]
	fn deregister_works() {
		run_test(|| {
			get_ready_for_events();

			assert_ok!(Pallet::<TestRuntime>::register(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				150
			));

			System::<TestRuntime>::set_block_number(151);

			let reserved_balance = Balances::reserved_balance(REGISTER_RELAYER);
			let free_balance = Balances::free_balance(REGISTER_RELAYER);
			assert_ok!(Pallet::<TestRuntime>::deregister(RuntimeOrigin::signed(REGISTER_RELAYER)));
			assert_eq!(
				Balances::reserved_balance(REGISTER_RELAYER),
				reserved_balance - Stake::get()
			);
			assert_eq!(Balances::free_balance(REGISTER_RELAYER), free_balance + Stake::get());

			assert_eq!(
				System::<TestRuntime>::events().last(),
				Some(&EventRecord {
					phase: Phase::Initialization,
					event: TestEvent::BridgeRelayers(Event::Deregistered {
						relayer: REGISTER_RELAYER
					}),
					topics: vec![],
				}),
			);
		});
	}

	#[test]
	fn is_registration_active_is_false_for_unregistered_relayer() {
		run_test(|| {
			assert!(!Pallet::<TestRuntime>::is_registration_active(&REGISTER_RELAYER));
		});
	}

	#[test]
	fn is_registration_active_is_true_when_stake_is_too_low() {
		run_test(|| {
			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER,
				registration(150, Stake::get() - 1),
			);
			assert!(Pallet::<TestRuntime>::is_registration_active(&REGISTER_RELAYER));
		});
	}

	#[test]
	fn is_registration_active_is_false_when_remaining_lease_is_too_low() {
		run_test(|| {
			System::<TestRuntime>::set_block_number(150 - Lease::get());

			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER,
				registration(150, Stake::get()),
			);
			assert!(!Pallet::<TestRuntime>::is_registration_active(&REGISTER_RELAYER));
		});
	}

	#[test]
	fn is_registration_active_is_true_when_relayer_is_registered_at_lanes() {
		run_test(|| {
			System::<TestRuntime>::set_block_number(150 - Lease::get());

			let mut registration = registration(150, Stake::get());
			assert!(registration.register_at_lane(LaneId::new(1, 2)));

			RegisteredRelayers::<TestRuntime>::insert(REGISTER_RELAYER, registration);
			assert!(Pallet::<TestRuntime>::is_registration_active(&REGISTER_RELAYER));
		});
	}

	#[test]
	fn is_registration_active_is_true_when_relayer_is_properly_registered() {
		run_test(|| {
			System::<TestRuntime>::set_block_number(150 - Lease::get());

			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER,
				registration(151, Stake::get()),
			);
			assert!(Pallet::<TestRuntime>::is_registration_active(&REGISTER_RELAYER));
		});
	}

	#[test]
	fn register_at_lane_fails_for_unregistered_relayer() {
		run_test(|| {
			assert_noop!(
				Pallet::<TestRuntime>::register_at_lane(
					RuntimeOrigin::signed(REGISTER_RELAYER),
					test_lane_id(),
					0
				),
				Error::<TestRuntime>::NotRegistered,
			);
		});
	}

	#[test]
	fn register_at_lane_fails_if_relayer_has_max_lane_registrations() {
		run_test(|| {
			let mut registration = registration(151, Stake::get());
			for i in 0..MaxLanesPerRelayer::get() {
				assert!(registration.register_at_lane(LaneId::new(42, i)));
			}

			RegisteredRelayers::<TestRuntime>::insert(REGISTER_RELAYER, registration);

			assert_noop!(
				Pallet::<TestRuntime>::register_at_lane(
					RuntimeOrigin::signed(REGISTER_RELAYER),
					LaneId::new(77, 77),
					0
				),
				Error::<TestRuntime>::FailedToRegisterAtLane,
			);
		});
	}

	#[test]
	fn register_at_lane_fails_if_relayer_requests_too_large_reward_to_claim_the_slot() {
		run_test(|| {
			let mut lane_relayers = LaneRelayersSet::empty(100);
			for i in 1..=MAX_NEXT_RELAYERS_PER_LANE as u64 {
				assert!(lane_relayers.next_set_try_push(REGISTER_RELAYER + i, 0));
			}
			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER,
				registration(151, Stake::get()),
			);
			LaneRelayers::<TestRuntime>::insert(test_lane_id(), lane_relayers);

			assert_noop!(
				Pallet::<TestRuntime>::register_at_lane(
					RuntimeOrigin::signed(REGISTER_RELAYER),
					test_lane_id(),
					1
				),
				Error::<TestRuntime>::TooLargeRewardToOccupyAnEntry,
			);
		});
	}

	#[test]
	fn register_at_lane_fails_if_relayer_does_not_have_required_balance() {
		run_test(|| {
			RegisteredRelayers::<TestRuntime>::insert(
				FAILING_RELAYER,
				registration(151, Stake::get()),
			);

			assert_noop!(
				Pallet::<TestRuntime>::register_at_lane(
					RuntimeOrigin::signed(FAILING_RELAYER),
					test_lane_id(),
					0
				),
				Error::<TestRuntime>::FailedToReserve,
			);
		});
	}

	#[test]
	fn register_at_lane_works() {
		run_test(|| {
			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER,
				registration(151, Stake::get()),
			);
			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER_2,
				registration(151, Stake::get()),
			);

			// when first relayer registers, we allow other relayers to register in next
			// `InitialElectionLength` blocks
			assert_ok!(Pallet::<TestRuntime>::register_at_lane(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				test_lane_id(),
				1
			));
			let lane_relayers = LaneRelayers::<TestRuntime>::get(test_lane_id()).unwrap();
			assert_eq!(lane_relayers.next_set_may_enact_at(), InitialElectionLength::get());
			assert_eq!(lane_relayers.active_relayers(), &[]);
			assert_eq!(
				lane_relayers.next_relayers(),
				&[RelayerAndReward::new(REGISTER_RELAYER, 1)]
			);

			// next relayer registers, it occupies the correct slot in the set
			assert_ok!(Pallet::<TestRuntime>::register_at_lane(
				RuntimeOrigin::signed(REGISTER_RELAYER_2),
				test_lane_id(),
				0
			));
			let lane_relayers = LaneRelayers::<TestRuntime>::get(test_lane_id()).unwrap();
			assert_eq!(lane_relayers.next_set_may_enact_at(), InitialElectionLength::get());
			assert_eq!(lane_relayers.active_relayers(), &[]);
			assert_eq!(
				lane_relayers.next_relayers(),
				&[
					RelayerAndReward::new(REGISTER_RELAYER_2, 0),
					RelayerAndReward::new(REGISTER_RELAYER, 1)
				]
			);
		});
	}

	#[test]
	fn register_at_lane_may_be_used_to_change_expected_reward() {
		run_test(|| {
			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER,
				registration(151, Stake::get()),
			);

			// at first we want reward `1`
			assert_ok!(Pallet::<TestRuntime>::register_at_lane(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				test_lane_id(),
				1
			));
			let lane_relayers = LaneRelayers::<TestRuntime>::get(test_lane_id()).unwrap();
			assert_eq!(
				lane_relayers.next_relayers(),
				&[RelayerAndReward::new(REGISTER_RELAYER, 1)]
			);

			// but then we change our expected reward
			assert_ok!(Pallet::<TestRuntime>::register_at_lane(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				test_lane_id(),
				0
			));
			let lane_relayers = LaneRelayers::<TestRuntime>::get(test_lane_id()).unwrap();
			assert_eq!(
				lane_relayers.next_relayers(),
				&[RelayerAndReward::new(REGISTER_RELAYER, 0)]
			);
		});
	}

	#[test]
	fn relayer_still_has_lane_registration_after_he_is_pushed_out_of_next_set() {
		run_test(|| {
			// leave one free entry in next set by relayers with bid = 10
			let mut lane_relayers = LaneRelayersSet::empty(100);
			for i in 1..MAX_NEXT_RELAYERS_PER_LANE as u64 {
				assert!(lane_relayers.next_set_try_push(REGISTER_RELAYER + 100 + i, 10));
			}
			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER,
				registration(151, Stake::get()),
			);
			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER_2,
				registration(151, Stake::get()),
			);
			LaneRelayers::<TestRuntime>::insert(test_lane_id(), lane_relayers);

			// occupy last entry by `REGISTER_RELAYER` with bid = 15
			assert_ok!(Pallet::<TestRuntime>::register_at_lane(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				test_lane_id(),
				15
			),);
			let lane_relayers = LaneRelayers::<TestRuntime>::get(test_lane_id()).unwrap();
			assert_eq!(lane_relayers.next_relayers().len() as u32, MAX_NEXT_RELAYERS_PER_LANE);
			assert_eq!(
				lane_relayers.next_relayers().last(),
				Some(&RelayerAndReward::new(REGISTER_RELAYER, 15))
			);

			// then the `REGISTER_RELAYER_2` comes with better bid = 14
			assert_ok!(Pallet::<TestRuntime>::register_at_lane(
				RuntimeOrigin::signed(REGISTER_RELAYER_2),
				test_lane_id(),
				14
			),);
			let lane_relayers = LaneRelayers::<TestRuntime>::get(test_lane_id()).unwrap();
			assert_eq!(lane_relayers.next_relayers().len() as u32, MAX_NEXT_RELAYERS_PER_LANE);
			assert_eq!(
				lane_relayers.next_relayers().last(),
				Some(&RelayerAndReward::new(REGISTER_RELAYER_2, 14))
			);

			// => `REGISTER_RELAYER` is pushed out of the next set, but it still has the lane in
			// his base "registration" structure, so it can rejoin anytime by calling
			// `register_at_lane` with updated reward
			assert_ok!(Pallet::<TestRuntime>::register_at_lane(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				test_lane_id(),
				13
			),);

			let lane_relayers = LaneRelayers::<TestRuntime>::get(test_lane_id()).unwrap();
			assert_eq!(lane_relayers.next_relayers().len() as u32, MAX_NEXT_RELAYERS_PER_LANE);
			assert_eq!(
				lane_relayers.next_relayers().last(),
				Some(&RelayerAndReward::new(REGISTER_RELAYER, 13))
			);
		});
	}
}
