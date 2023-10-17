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
	ActiveLaneRelayersSet, NextLaneRelayersSet, PaymentProcedure, Registration,
	RelayerRewardAtSource, RelayerRewardsKeyProvider, RewardsAccountParams, StakeAndSlash,
};
use bp_runtime::StorageDoubleMapKeyProvider;
use frame_support::{dispatch::PostDispatchInfo, fail};
use frame_system::Pallet as SystemPallet;
use sp_arithmetic::traits::{AtLeast32BitUnsigned, Zero};
use sp_runtime::{traits::CheckedSub, Saturating};
use sp_std::{marker::PhantomData, vec::Vec};

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
		/// (pushing out actual relayers with larger expected reward) and then `deregistering`
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

		/// Reserve some funds on relayer account to be able to register later.
		///
		/// Basic (`register` call) and lane registration require some funds on relayer account to
		/// be reserved. This stake is slashed if relayer is submitting invalid transaction when
		/// his registration is active. In exchange, relayer gets priority boosts for his message
		/// delivery transactions and, as a result, reward for delivering messages.
		///
		/// This call must be followed by `register` and (optionally) `register_at_lane` calls to
		/// activate priority boosts.
		#[pallet::call_index(1)]
		#[pallet::weight(Weight::zero())] // TODO
		pub fn increase_stake(
			origin: OriginFor<T>,
			additional_amount: T::Reward,
		) -> DispatchResult {
			let relayer = ensure_signed(origin)?;

			RegisteredRelayers::<T>::try_mutate(&relayer, |maybe_registration| -> DispatchResult {
				// by default registration is valid until block `0`, which means that it is not
				// active. To activate it, relayer must use the `register` call
				let mut registration =
					maybe_registration.take().unwrap_or_else(|| Registration::new(Zero::zero()));

				registration.set_stake(Self::update_relayer_stake(
					&relayer,
					registration.current_stake(),
					registration.current_stake().saturating_add(additional_amount),
				)?);

				*maybe_registration = Some(registration);

				Ok(())
			})
		}

		/// Unreserve some or all funds, reserved previously by the `reserve_funds` call.
		///
		/// The reserved amount after this call must cover basic registration and all lane
		/// registrations that relayer has.
		#[pallet::call_index(2)]
		#[pallet::weight(Weight::zero())] // TODO
		pub fn decrease_stake(origin: OriginFor<T>, to_unreserve: T::Reward) -> DispatchResult {
			let relayer = ensure_signed(origin)?;

			RegisteredRelayers::<T>::try_mutate(&relayer, |maybe_registration| -> DispatchResult {
				let mut registration = match maybe_registration.take() {
					Some(registration) => registration,
					None => fail!(Error::<T>::NotRegistered),
				};

				// check if reserved amount after the call is enough to cover all remaining
				// registrations
				let stake_before = registration.current_stake();
				let stake_after = stake_before.saturating_sub(to_unreserve);
				let required_stake = Self::required_stake(&registration);
				ensure!(stake_after >= required_stake, Error::<T>::StakeIsTooLow);

				// ok, now we know that we can increase the stake => let's do it
				registration.set_stake(Self::update_relayer_stake(
					&relayer,
					stake_before,
					stake_after,
				)?);

				*maybe_registration = Some(registration);

				Ok(())
			})
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
		#[pallet::call_index(3)]
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

			// registration must have been created by the `increase_stake` before
			let mut registration = match Self::registered_relayer(&relayer) {
				Some(registration) => registration,
				None => fail!(Error::<T>::NotRegistered),
			};

			// ensure that the stake is enough
			ensure!(Self::is_stake_enough(&registration), Error::<T>::StakeIsTooLow);

			// new `valid_till` must be larger (or equal) than the old one
			ensure!(
				valid_till >= registration.valid_till_ignore_lanes(),
				Error::<T>::CannotReduceRegistrationLease,
			);
			registration.set_valid_till(valid_till);

			// deposit event
			log::trace!(target: LOG_TARGET, "Successfully registered relayer: {:?}", relayer);
			Self::deposit_event(Event::<T>::RegistrationUpdated {
				relayer: relayer.clone(),
				registration: registration.clone(),
			});

			// update registration in the runtime storage
			RegisteredRelayers::<T>::insert(relayer, registration);

			Ok(())
		}

		/// `Deregister` relayer.
		///
		/// After this call, message delivery transactions of the relayer won't get any priority
		/// boost. Keep in mind that the relayer can't deregister until `valid_till` block, which
		/// he has specified in the registration call. The relayer is also unregistered from all
		/// lanes, where he has explicitly registered using `register_at_lane`.
		///
		/// The stake on relayer account is unreserved.
		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::deregister())]
		pub fn deregister(origin: OriginFor<T>) -> DispatchResult {
			let relayer = ensure_signed(origin)?;

			// only registered relayers can deregister
			let registration = match Self::registered_relayer(&relayer) {
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

			// deposit event
			log::trace!(target: LOG_TARGET, "Successfully deregistered relayer: {:?}", relayer);
			Self::deposit_event(Event::<T>::Deregistered { relayer: relayer.clone() });

			// update runtime storage
			RegisteredRelayers::<T>::remove(&relayer);

			Ok(())
		}

		/// Register relayer intention to deliver inbound messages at given messages lane.
		///
		/// Relayer that registers itself at given message lane gets a priority boost for his
		/// message delivery transactions, **verified** at his slots (consecutive range of blocks).
		///
		/// Every lane registration requires additional stake. Relayer registration is considered
		/// active while it is registered at least at one lane.
		///
		/// This call (if successful), puts relayer in the relayers set that will be active during
		/// next epoch. So boost is not immediate - it will be activated after `advance_lane_epoch`
		/// call. However, before that call, relayer may be pushed from the next set by relayers,
		/// offering lower `expected_reward`. If that happens, relayer may either try to re-register
		/// itself by repeating the `register_at_lane` call, offering lower reward. Or it may claim
		/// his lane stake back, by updating his registration with `register` call or deregistering
		/// at all using `deregister` call.
		///
		/// Relayer may request large reward here (using `expected_reward`), but in the end, the
		/// reward amount is computed at the bridged (source chain). In the case if
		/// [`DeliveryConfirmationPaymentsAdapter`] is used to register rewards, the maximal reward
		/// per message is limited by the `MaxRewardPerMessage` parameter.
		#[pallet::call_index(5)]
		#[pallet::weight(Weight::zero())] // TODO
		pub fn register_at_lane(
			origin: OriginFor<T>,
			lane: LaneId,
			expected_relayer_reward_per_message: RelayerRewardAtSource,
		) -> DispatchResult {
			let relayer = ensure_signed(origin)?;

			// we only allow registered relayers to have priority boosts
			let mut registration = match Self::registered_relayer(&relayer) {
				Some(registration) => registration,
				None => fail!(Error::<T>::NotRegistered),
			};

			// cannot add another lane registration if relayer has already max allowed
			// lane registrations
			ensure!(registration.register_at_lane(lane), Error::<T>::FailedToRegisterAtLane);

			// ensure that the relayer stake is enough
			ensure!(Self::is_stake_enough(&registration), Error::<T>::StakeIsTooLow);

			// read or create next lane relayers
			let mut next_lane_relayers = match NextLaneRelayers::<T>::get(lane) {
				Some(lane_relayers) => lane_relayers,
				None => NextLaneRelayersSet::empty(
					SystemPallet::<T>::block_number()
						.saturating_add(T::InitialElectionLength::get()),
				),
			};

			// try to push relayer to the next set
			ensure!(
				next_lane_relayers.try_push(relayer.clone(), expected_relayer_reward_per_message),
				Error::<T>::TooLargeRewardToOccupyAnEntry,
			);

			// the relayer need to stake additional amount for every additional lane
			registration.set_stake(Self::update_relayer_stake(
				&relayer,
				registration.current_stake(),
				registration.required_stake(Self::base_stake(), Self::stake_per_lane()),
			)?);

			// update basic and lane registration in the runtime storage
			RegisteredRelayers::<T>::insert(&relayer, registration);
			NextLaneRelayers::<T>::insert(lane, next_lane_relayers);

			Ok(())
		}

		/// Deregister relayer intention to deliver inbound messages at given messages lane.
		///
		/// After deregistration, relayer won't get lane-specific boost for message delivery
		/// transactions at that lane. It would still get the basic boost until the `deregister`
		/// call.
		///
		/// This call (if successful), removes relayer from the relayers set that will be active
		/// during next epoch. If relayer is still in the active set, it keeps getting additional
		/// priority boost for his message delivery transaction at that lane. The relayer will be
		/// able to claim his lane stake back when it is removed from both active and the next set.
		#[pallet::call_index(6)]
		#[pallet::weight(Weight::zero())] // TODO
		pub fn deregister_at_lane(origin: OriginFor<T>, lane: LaneId) -> DispatchResult {
			let relayer = ensure_signed(origin)?;

			// if relayer is in the active set, we can not simply remove lane registration and let
			// him unreserve some portion of his stake. Instead, we remember the fact that he has
			// removed lane registration recently and once lane epoch advances
			let is_in_active_set = Self::active_lane_relayers(lane).relayer(&relayer).is_some();
			if !is_in_active_set {
				// if relayer doesn't have a basic registration, we know that he is not
				// registered at the lane as well
				let mut registration = match Self::registered_relayer(&relayer) {
					Some(registration) => registration,
					None => fail!(Error::<T>::NotRegistered),
				};

				// forget lane registration
				Self::remove_lane_from_relayer_registration(&mut registration, lane, false);

				// update registration in the runtime storage
				RegisteredRelayers::<T>::insert(&relayer, registration);
			}

			// remove relayer from the `next_set` of lane relayers
			NextLaneRelayers::<T>::mutate_extant(lane, |next_lane_relayers| {
				next_lane_relayers.try_remove(&relayer);

				// we can't remove `NextLaneRelayers` entry here (if there are no more relayers
				// in the set), bnecause the `may_enact_at` is important too
			});

			Ok(())
		}

		/// Enact next set of relayers at a given lane.
		///
		/// This will replace the set of active relayers with the next scheduled set, for given
		/// lane. Anyone could call this method at any point. If the set will be changed, the cost
		/// of transaction will be refunded to the submitter. We do not provide any on-chain means
		/// to sync between relayers on who will submit this transaction, so first transaction from
		/// anyone will be accepted and it will have the zero cost. All subsequent transactions will
		/// be paid. We suggest the first relayer from the `next_set` to submit this transaction.
		#[pallet::call_index(7)]
		#[pallet::weight(Weight::zero())] // TODO
		pub fn advance_lane_epoch(
			origin: OriginFor<T>,
			lane: LaneId,
		) -> DispatchResultWithPostInfo {
			let _ = ensure_signed(origin)?;

			let current_block_number = SystemPallet::<T>::block_number();
			let mut active_lane_relayers = Self::active_lane_relayers(lane);
			let mut next_lane_relayers = match Self::next_lane_relayers(lane) {
				Some(lane_relayers) => lane_relayers,
				None => fail!(Error::<T>::NoRelayersAtLane),
			};

			// TODO: the same `Self::registered_relayer(relayer).map(|reg|
			// reg.lanes().contains(&lane))` is called in `activate_next_set` and later in `for
			// old_relayer in old_active_set`. May dedup to decrease weight

			// activate next set of relayers
			let old_active_set = active_lane_relayers
				.relayers()
				.iter()
				.map(|r| r.relayer().clone())
				.collect::<Vec<_>>();
			ensure!(
				active_lane_relayers.activate_next_set(
					current_block_number,
					next_lane_relayers.clone(),
					|relayer| Self::registered_relayer(relayer)
						.map(|reg| reg.lanes().contains(&lane))
						.unwrap_or(false),
				),
				Error::<T>::TooEarlyToActivateNextRelayersSet,
			);

			// update new epoch end in the next set
			next_lane_relayers
				.set_may_enact_at(current_block_number.saturating_add(T::EpochLength::get()));

			// for every relayer, who was in the active set, but is missing from the next
			// set, remove lane registration
			//
			// technically, this is incorrect, because relaye may have wanted to keep lane
			// registration. But there's no difference between such state and state when the relayer
			// has deregistered
			for old_relayer in old_active_set {
				if next_lane_relayers.relayer(&old_relayer).is_some() {
					continue
				}

				RegisteredRelayers::<T>::mutate_extant(&old_relayer, |registration| {
					Self::remove_lane_from_relayer_registration(registration, lane, true);
				});
			}

			// update relayer sets in the storage
			ActiveLaneRelayers::<T>::insert(lane, active_lane_relayers);
			NextLaneRelayers::<T>::insert(lane, next_lane_relayers);

			Ok(PostDispatchInfo { actual_weight: None, pays_fee: Pays::No })
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

		/// Returns total stake that the relayer must hold reserved on his account.
		fn required_stake(
			registration: &Registration<BlockNumberFor<T>, T::Reward, T::MaxLanesPerRelayer>,
		) -> T::Reward {
			registration.required_stake(Self::base_stake(), Self::stake_per_lane())
		}

		/// Returns true if relayer stake is enough to cover basic and all lane registrations.
		fn is_stake_enough(
			registration: &Registration<BlockNumberFor<T>, T::Reward, T::MaxLanesPerRelayer>,
		) -> bool {
			registration.current_stake() >= Self::required_stake(registration)
		}

		/// Remove lane from basic relayer registration. It shall only be called if relayer is
		/// already removed from both active and next relayers set.
		///
		/// Returns true if registration has been modified.
		fn remove_lane_from_relayer_registration(
			registration: &mut Registration<BlockNumberFor<T>, T::Reward, T::MaxLanesPerRelayer>,
			lane: LaneId,
			was_in_active_set: bool,
		) -> bool {
			// if we are already removed from the
			if !registration.deregister_at_lane(lane) {
				return false
			}

			// if relayer was not in the active set, we don't need to prolong his registration
			if !was_in_active_set {
				return true
			}

			// since relayer may get priority boost for transactions, verified at **this** block, we
			// require its registration for aty least another `required_registration_lease` blocks.
			// We do not need to do that if relayer still have other lane registrations - the valid
			// till only works when relayer has no lane registrations and when it'll be
			// deregistering from the last lane, we will increase it.
			if let Some(valid_till) = registration.valid_till() {
				registration.set_valid_till(sp_std::cmp::max(
					valid_till,
					SystemPallet::<T>::block_number()
						.saturating_add(Self::required_registration_lease()),
				));
			}

			true
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
		/// The expected reward, specified by relayer during `register_at_lane` call is too large
		/// to occupy an entry in the next relayer set.
		TooLargeRewardToOccupyAnEntry,
		/// Lane has no relayers set.
		NoRelayersAtLane,
		/// Next set of lane relayers cannot be activated now. It can be activated later, once
		/// at `next_set_may_enact_at` block.
		TooEarlyToActivateNextRelayersSet,
		/// Relayer stake is too low to add a basic registration and/or lane registration.
		StakeIsTooLow,
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

	/// An active set of relayers that have explicitly registered themselves at a given lane.
	///
	/// Every relayer inside this set receives additional priority boost when it submits
	/// message delivers messages at given lane. The boost only happens inside the slot,
	/// assigned to relayer.
	#[pallet::storage]
	#[pallet::getter(fn active_lane_relayers)]
	pub type ActiveLaneRelayers<T: Config> = StorageMap<
		_,
		Identity,
		LaneId,
		ActiveLaneRelayersSet<T::AccountId, BlockNumberFor<T>, T::MaxActiveRelayersPerLane>,
		ValueQuery,
	>;

	/// A next set of relayers that have explicitly registered themselves at a given lane.
	///
	/// This set may replace the [`ActiveLaneRelayers`] after current epoch ends.
	#[pallet::storage]
	#[pallet::getter(fn next_lane_relayers)]
	pub type NextLaneRelayers<T: Config> = StorageMap<
		_,
		Identity,
		LaneId,
		NextLaneRelayersSet<T::AccountId, BlockNumberFor<T>, T::MaxNextRelayersPerLane>,
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
	use sp_runtime::{traits::ConstU32, DispatchError};

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
				BridgeRelayers::claim_rewards(RuntimeOrigin::root(), test_reward_account_param()),
				DispatchError::BadOrigin,
			);
		});
	}

	#[test]
	fn relayer_cant_claim_if_no_reward_exists() {
		run_test(|| {
			assert_noop!(
				BridgeRelayers::claim_rewards(
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
				BridgeRelayers::claim_rewards(
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
			assert_ok!(BridgeRelayers::claim_rewards(
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
	fn increase_stake_requires_signed_origin() {
		run_test(|| {
			assert_noop!(
				BridgeRelayers::increase_stake(RuntimeOrigin::root(), 1),
				DispatchError::BadOrigin
			);
		});
	}

	#[test]
	fn increase_stake_creates_empty_registration() {
		run_test(|| {
			assert_ok!(BridgeRelayers::increase_stake(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				Stake::get() + 42
			));
			assert_eq!(
				BridgeRelayers::registered_relayer(&REGISTER_RELAYER),
				Some(registration(0, Stake::get() + 42))
			);
			assert_eq!(Balances::reserved_balance(REGISTER_RELAYER), Stake::get() + 42);
		});
	}

	#[test]
	fn increase_stake_works_for_existing_registration() {
		run_test(|| {
			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER,
				registration(150, Stake::get() + 42),
			);

			assert_ok!(BridgeRelayers::increase_stake(RuntimeOrigin::signed(REGISTER_RELAYER), 8));
			assert_eq!(
				BridgeRelayers::registered_relayer(&REGISTER_RELAYER),
				Some(registration(150, Stake::get() + 50))
			);
			assert_eq!(Balances::reserved_balance(REGISTER_RELAYER), 8);
		});
	}

	#[test]
	fn increase_stake_fails_if_it_fails_to_reserve_additional_stake() {
		run_test(|| {
			assert_noop!(
				BridgeRelayers::increase_stake(
					RuntimeOrigin::signed(REGISTER_RELAYER),
					ThisChainBalance::MAX
				),
				Error::<TestRuntime>::FailedToReserve,
			);
		});
	}

	#[test]
	fn decrease_stake_requires_signed_origin() {
		run_test(|| {
			assert_noop!(
				BridgeRelayers::decrease_stake(RuntimeOrigin::root(), 1),
				DispatchError::BadOrigin
			);
		});
	}

	#[test]
	fn decrease_stake_fails_if_relayer_is_not_registered() {
		run_test(|| {
			assert_noop!(
				BridgeRelayers::decrease_stake(RuntimeOrigin::signed(REGISTER_RELAYER), 1),
				Error::<TestRuntime>::NotRegistered
			);
		});
	}

	#[test]
	fn decrease_stake_fails_if_stake_after_call_is_too_low_to_cover_all_registrations() {
		run_test(|| {
			let lane_1_id = test_lane_id();
			let lane_2_id = LaneId::new(lane_1_id, lane_1_id);

			// first, check that it accounts both basic and lane registrations
			let mut reg = registration(150, Stake::get() + LaneStake::get() + LaneStake::get());
			assert!(reg.register_at_lane(lane_1_id));
			assert!(reg.register_at_lane(lane_2_id));
			RegisteredRelayers::<TestRuntime>::insert(REGISTER_RELAYER, reg);
			assert_noop!(
				BridgeRelayers::decrease_stake(RuntimeOrigin::signed(REGISTER_RELAYER), 1),
				Error::<TestRuntime>::StakeIsTooLow
			);

			// first, check that it accounts basic registration
			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER,
				registration(150, Stake::get()),
			);
			assert_noop!(
				BridgeRelayers::decrease_stake(RuntimeOrigin::signed(REGISTER_RELAYER), 1),
				Error::<TestRuntime>::StakeIsTooLow
			);
		});
	}

	#[test]
	fn decrease_stake_fails_if_it_cant_unreserve_stake() {
		run_test(|| {
			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER,
				registration(150, Stake::get() + 1),
			);
			assert_noop!(
				BridgeRelayers::decrease_stake(RuntimeOrigin::signed(REGISTER_RELAYER), 1),
				Error::<TestRuntime>::FailedToUnreserve
			);
		});
	}

	#[test]
	fn decrease_stake_works() {
		run_test(|| {
			let lane_1_id = test_lane_id();
			let lane_2_id = LaneId::new(lane_1_id, lane_1_id);

			// check that it works for both basic and lane registrations
			TestStakeAndSlash::reserve(
				&REGISTER_RELAYER,
				Stake::get() + LaneStake::get() + LaneStake::get() + 1,
			)
			.unwrap();
			let mut reg = registration(150, Stake::get() + LaneStake::get() + LaneStake::get() + 1);
			assert!(reg.register_at_lane(lane_1_id));
			assert!(reg.register_at_lane(lane_2_id));
			RegisteredRelayers::<TestRuntime>::insert(REGISTER_RELAYER, reg);
			assert_ok!(BridgeRelayers::decrease_stake(RuntimeOrigin::signed(REGISTER_RELAYER), 1));
			assert_eq!(
				Balances::reserved_balance(REGISTER_RELAYER),
				Stake::get() + LaneStake::get() + LaneStake::get()
			);

			// first, check that it works for basic registration
			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER,
				registration(150, Stake::get() + 1),
			);
			assert_ok!(BridgeRelayers::decrease_stake(RuntimeOrigin::signed(REGISTER_RELAYER), 1));
			assert_eq!(
				Balances::reserved_balance(REGISTER_RELAYER),
				Stake::get() + LaneStake::get() + LaneStake::get() - 1
			);
		});
	}

	#[test]
	fn register_fails_if_valid_till_is_a_past_block() {
		run_test(|| {
			System::<TestRuntime>::set_block_number(100);

			assert_noop!(
				BridgeRelayers::register(RuntimeOrigin::signed(REGISTER_RELAYER), 50),
				Error::<TestRuntime>::InvalidRegistrationLease,
			);
		});
	}

	#[test]
	fn register_fails_if_valid_till_lease_is_less_than_required() {
		run_test(|| {
			System::<TestRuntime>::set_block_number(100);

			assert_noop!(
				BridgeRelayers::register(
					RuntimeOrigin::signed(REGISTER_RELAYER),
					99 + Lease::get()
				),
				Error::<TestRuntime>::InvalidRegistrationLease,
			);
		});
	}

	#[test]
	fn register_fails_if_stake_is_not_enough() {
		run_test(|| {
			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER,
				registration(0, Stake::get() - 1),
			);

			assert_noop!(
				BridgeRelayers::register(RuntimeOrigin::signed(REGISTER_RELAYER), 150),
				Error::<TestRuntime>::StakeIsTooLow,
			);
		});
	}

	#[test]
	fn register_works() {
		run_test(|| {
			get_ready_for_events();
			assert_ok!(BridgeRelayers::increase_stake(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				Stake::get()
			));
			assert_eq!(Balances::reserved_balance(REGISTER_RELAYER), Stake::get());

			assert_ok!(BridgeRelayers::register(RuntimeOrigin::signed(REGISTER_RELAYER), 150));
			assert_eq!(
				BridgeRelayers::registered_relayer(REGISTER_RELAYER),
				Some(registration(150, Stake::get()))
			);
			assert_eq!(Balances::reserved_balance(REGISTER_RELAYER), Stake::get());

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
			assert_ok!(BridgeRelayers::increase_stake(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				Stake::get()
			));

			assert_ok!(BridgeRelayers::register(RuntimeOrigin::signed(REGISTER_RELAYER), 150));

			assert_noop!(
				BridgeRelayers::register(RuntimeOrigin::signed(REGISTER_RELAYER), 125),
				Error::<TestRuntime>::CannotReduceRegistrationLease,
			);
		});
	}

	#[test]
	fn deregister_fails_if_not_registered() {
		run_test(|| {
			assert_noop!(
				BridgeRelayers::deregister(RuntimeOrigin::signed(REGISTER_RELAYER)),
				Error::<TestRuntime>::NotRegistered,
			);
		});
	}

	#[test]
	fn deregister_fails_if_registration_is_still_active() {
		run_test(|| {
			assert_ok!(BridgeRelayers::increase_stake(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				Stake::get()
			));
			assert_ok!(BridgeRelayers::register(RuntimeOrigin::signed(REGISTER_RELAYER), 150));

			System::<TestRuntime>::set_block_number(100);

			assert_noop!(
				BridgeRelayers::deregister(RuntimeOrigin::signed(REGISTER_RELAYER)),
				Error::<TestRuntime>::RegistrationIsStillActive,
			);
		});
	}

	#[test]
	fn deregister_fails_if_relayer_has_lanes_registrations() {
		run_test(|| {
			assert_ok!(BridgeRelayers::increase_stake(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				Stake::get() + LaneStake::get()
			));
			assert_ok!(BridgeRelayers::register(RuntimeOrigin::signed(REGISTER_RELAYER), 150));
			assert_ok!(BridgeRelayers::register_at_lane(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				test_lane_id(),
				0,
			));

			System::<TestRuntime>::set_block_number(151);

			assert_noop!(
				BridgeRelayers::deregister(RuntimeOrigin::signed(REGISTER_RELAYER)),
				Error::<TestRuntime>::RegistrationIsStillActive,
			);
		});
	}

	#[test]
	fn deregister_works() {
		run_test(|| {
			get_ready_for_events();

			assert_ok!(BridgeRelayers::increase_stake(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				Stake::get()
			));
			assert_ok!(BridgeRelayers::register(RuntimeOrigin::signed(REGISTER_RELAYER), 150));

			System::<TestRuntime>::set_block_number(151);

			let reserved_balance = Balances::reserved_balance(REGISTER_RELAYER);
			let free_balance = Balances::free_balance(REGISTER_RELAYER);
			assert_ok!(BridgeRelayers::deregister(RuntimeOrigin::signed(REGISTER_RELAYER)));
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
	fn deregister_works_after_last_lane_registration_is_removed() {
		run_test(|| {
			assert_ok!(BridgeRelayers::increase_stake(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				Stake::get() + LaneStake::get()
			));
			assert_ok!(BridgeRelayers::register(RuntimeOrigin::signed(REGISTER_RELAYER), 150));
			assert_ok!(BridgeRelayers::register_at_lane(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				test_lane_id(),
				0,
			));
			assert_ok!(BridgeRelayers::deregister_at_lane(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				test_lane_id(),
			));

			System::<TestRuntime>::set_block_number(151);

			assert_ok!(BridgeRelayers::deregister(RuntimeOrigin::signed(REGISTER_RELAYER)));
		});
	}

	#[test]
	fn is_registration_active_is_false_for_unregistered_relayer() {
		run_test(|| {
			assert!(!BridgeRelayers::is_registration_active(&REGISTER_RELAYER));
		});
	}

	#[test]
	fn is_registration_active_is_true_when_stake_is_too_low() {
		run_test(|| {
			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER,
				registration(150, Stake::get() - 1),
			);
			assert!(BridgeRelayers::is_registration_active(&REGISTER_RELAYER));
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
			assert!(!BridgeRelayers::is_registration_active(&REGISTER_RELAYER));
		});
	}

	#[test]
	fn is_registration_active_is_true_when_relayer_is_registered_at_lanes() {
		run_test(|| {
			System::<TestRuntime>::set_block_number(151 - Lease::get());

			let mut registration = registration(150, Stake::get());
			assert!(registration.register_at_lane(LaneId::new(1, 2)));

			RegisteredRelayers::<TestRuntime>::insert(REGISTER_RELAYER, registration);
			assert!(BridgeRelayers::is_registration_active(&REGISTER_RELAYER));
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
			assert!(BridgeRelayers::is_registration_active(&REGISTER_RELAYER));
		});
	}

	#[test]
	fn register_at_lane_fails_for_unregistered_relayer() {
		run_test(|| {
			assert_noop!(
				BridgeRelayers::register_at_lane(
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
			let mut registration = registration(151, Stake::get() + LaneStake::get());
			for i in 0..MaxLanesPerRelayer::get() {
				assert!(registration.register_at_lane(LaneId::new(42, i)));
			}

			RegisteredRelayers::<TestRuntime>::insert(REGISTER_RELAYER, registration);

			assert_noop!(
				BridgeRelayers::register_at_lane(
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
			let mut lane_relayers = NextLaneRelayersSet::empty(100);
			for i in 1..=MaxNextRelayersPerLane::get() as u64 {
				assert!(lane_relayers.try_push(REGISTER_RELAYER + i, 0));
			}
			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER,
				registration(151, Stake::get() + LaneStake::get()),
			);
			NextLaneRelayers::<TestRuntime>::insert(test_lane_id(), lane_relayers);

			assert_noop!(
				BridgeRelayers::register_at_lane(
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
				registration(151, Stake::get() + LaneStake::get() - 1),
			);

			assert_noop!(
				BridgeRelayers::register_at_lane(
					RuntimeOrigin::signed(FAILING_RELAYER),
					test_lane_id(),
					0
				),
				Error::<TestRuntime>::StakeIsTooLow,
			);
		});
	}

	#[test]
	fn register_at_lane_works() {
		run_test(|| {
			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER,
				registration(151, Stake::get() + LaneStake::get()),
			);
			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER_2,
				registration(151, Stake::get() + LaneStake::get()),
			);

			// when first relayer registers, we allow other relayers to register in next
			// `InitialElectionLength` blocks
			assert_ok!(BridgeRelayers::register_at_lane(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				test_lane_id(),
				1
			));
			let active_lane_relayers = BridgeRelayers::active_lane_relayers(test_lane_id());
			let next_lane_relayers = BridgeRelayers::next_lane_relayers(test_lane_id()).unwrap();
			assert_eq!(active_lane_relayers.relayers(), &[]);
			assert_eq!(next_lane_relayers.may_enact_at(), InitialElectionLength::get());
			assert_eq!(
				next_lane_relayers.relayers(),
				&[RelayerAndReward::new(REGISTER_RELAYER, 1)]
			);

			// next relayer registers, it occupies the correct slot in the set
			assert_ok!(BridgeRelayers::register_at_lane(
				RuntimeOrigin::signed(REGISTER_RELAYER_2),
				test_lane_id(),
				0
			));
			let active_lane_relayers = BridgeRelayers::active_lane_relayers(test_lane_id());
			let next_lane_relayers = BridgeRelayers::next_lane_relayers(test_lane_id()).unwrap();
			assert_eq!(active_lane_relayers.relayers(), &[]);
			assert_eq!(next_lane_relayers.may_enact_at(), InitialElectionLength::get());
			assert_eq!(
				next_lane_relayers.relayers(),
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
				registration(151, Stake::get() + LaneStake::get()),
			);

			// at first we want reward `1`
			assert_ok!(BridgeRelayers::register_at_lane(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				test_lane_id(),
				1
			));
			let next_lane_relayers = BridgeRelayers::next_lane_relayers(test_lane_id()).unwrap();
			assert_eq!(
				next_lane_relayers.relayers(),
				&[RelayerAndReward::new(REGISTER_RELAYER, 1)]
			);

			// but then we change our expected reward
			assert_ok!(BridgeRelayers::register_at_lane(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				test_lane_id(),
				0
			));
			let next_lane_relayers = BridgeRelayers::next_lane_relayers(test_lane_id()).unwrap();
			assert_eq!(
				next_lane_relayers.relayers(),
				&[RelayerAndReward::new(REGISTER_RELAYER, 0)]
			);
		});
	}

	#[test]
	fn relayer_still_has_lane_registration_after_he_is_pushed_out_of_next_set() {
		run_test(|| {
			// leave one free entry in next set by relayers with bid = 10
			let mut lane_relayers = NextLaneRelayersSet::empty(100);
			for i in 1..MaxNextRelayersPerLane::get() as u64 {
				assert!(lane_relayers.try_push(REGISTER_RELAYER + 100 + i, 10));
			}
			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER,
				registration(151, Stake::get() + LaneStake::get()),
			);
			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER_2,
				registration(151, Stake::get() + LaneStake::get()),
			);
			NextLaneRelayers::<TestRuntime>::insert(test_lane_id(), lane_relayers);

			// occupy last entry by `REGISTER_RELAYER` with bid = 15
			assert_ok!(BridgeRelayers::register_at_lane(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				test_lane_id(),
				15
			),);
			let next_lane_relayers = BridgeRelayers::next_lane_relayers(test_lane_id()).unwrap();
			assert_eq!(next_lane_relayers.relayers().len() as u32, MaxNextRelayersPerLane::get());
			assert_eq!(
				next_lane_relayers.relayers().last(),
				Some(&RelayerAndReward::new(REGISTER_RELAYER, 15))
			);

			// then the `REGISTER_RELAYER_2` comes with better bid = 14
			assert_ok!(BridgeRelayers::register_at_lane(
				RuntimeOrigin::signed(REGISTER_RELAYER_2),
				test_lane_id(),
				14
			),);
			let next_lane_relayers = BridgeRelayers::next_lane_relayers(test_lane_id()).unwrap();
			assert_eq!(next_lane_relayers.relayers().len() as u32, MaxNextRelayersPerLane::get());
			assert_eq!(
				next_lane_relayers.relayers().last(),
				Some(&RelayerAndReward::new(REGISTER_RELAYER_2, 14))
			);

			// => `REGISTER_RELAYER` is pushed out of the next set, but it still has the lane in
			// his base "registration" structure, so it can rejoin anytime by calling
			// `register_at_lane` with updated reward
			assert_ok!(BridgeRelayers::register_at_lane(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				test_lane_id(),
				13
			),);

			let next_lane_relayers = BridgeRelayers::next_lane_relayers(test_lane_id()).unwrap();
			assert_eq!(next_lane_relayers.relayers().len() as u32, MaxNextRelayersPerLane::get());
			assert_eq!(
				next_lane_relayers.relayers().last(),
				Some(&RelayerAndReward::new(REGISTER_RELAYER, 13))
			);
		});
	}

	#[test]
	fn deregister_at_lane_fails_for_unregistered_relayer() {
		run_test(|| {
			assert_noop!(
				BridgeRelayers::deregister_at_lane(
					RuntimeOrigin::signed(REGISTER_RELAYER),
					test_lane_id(),
				),
				Error::<TestRuntime>::NotRegistered,
			);
		});
	}

	#[test]
	fn deregister_at_lane_does_not_fail_if_next_lane_relayers_are_missing() {
		run_test(|| {
			let mut registration = registration(151, Stake::get() + LaneStake::get());
			registration.register_at_lane(test_lane_id());
			RegisteredRelayers::<TestRuntime>::insert(REGISTER_RELAYER, registration);

			// when relayer is not in the active set
			assert_ok!(BridgeRelayers::deregister_at_lane(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				test_lane_id(),
			));

			// when relayer is in the active set
			let mut active_lane_relayers = ActiveLaneRelayersSet::default();
			assert!(active_lane_relayers.activate_next_set(
				0,
				{
					let mut next_lane_relayers: NextLaneRelayersSet<_, _, ConstU32<1>> =
						NextLaneRelayersSet::empty(0);
					assert!(next_lane_relayers.try_push(REGISTER_RELAYER, 0));
					next_lane_relayers
				},
				|_| true
			));
			ActiveLaneRelayers::<TestRuntime>::insert(test_lane_id(), active_lane_relayers);
			assert_ok!(BridgeRelayers::deregister_at_lane(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				test_lane_id(),
			));
		})
	}

	#[test]
	fn deregister_at_lane_works_when_relayer_is_not_in_active_set() {
		run_test(|| {
			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER,
				registration(150, Stake::get() + LaneStake::get()),
			);

			// register at lane
			assert_ok!(BridgeRelayers::register_at_lane(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				test_lane_id(),
				0
			));
			assert_eq!(
				BridgeRelayers::registered_relayer(REGISTER_RELAYER).unwrap().valid_till(),
				None
			);
			assert_eq!(
				BridgeRelayers::registered_relayer(REGISTER_RELAYER)
					.unwrap()
					.valid_till_ignore_lanes(),
				150
			);
			assert_eq!(
				BridgeRelayers::registered_relayer(REGISTER_RELAYER).unwrap().lanes(),
				&[test_lane_id()]
			);
			assert_eq!(BridgeRelayers::active_lane_relayers(test_lane_id()).relayers(), &[]);
			assert_eq!(
				BridgeRelayers::next_lane_relayers(test_lane_id()).unwrap().relayers(),
				&[RelayerAndReward::new(REGISTER_RELAYER, 0)]
			);

			// and then deregister at lane before going into active set
			System::<TestRuntime>::set_block_number(150);
			assert_ok!(BridgeRelayers::deregister_at_lane(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				test_lane_id()
			));
			assert_eq!(
				BridgeRelayers::registered_relayer(REGISTER_RELAYER).unwrap().valid_till(),
				Some(150)
			);
			assert_eq!(BridgeRelayers::registered_relayer(REGISTER_RELAYER).unwrap().lanes(), &[]);
			assert_eq!(BridgeRelayers::active_lane_relayers(test_lane_id()).relayers(), &[]);
			assert_eq!(BridgeRelayers::next_lane_relayers(test_lane_id()).unwrap().relayers(), &[]);
		});
	}

	#[test]
	fn deregister_at_lane_works_when_relayer_is_in_active_set() {
		run_test(|| {
			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER,
				registration(150, Stake::get() + LaneStake::get()),
			);

			// register at lane
			assert_ok!(BridgeRelayers::register_at_lane(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				test_lane_id(),
				0
			));

			// activate next lane epoch
			System::<TestRuntime>::set_block_number(
				BridgeRelayers::next_lane_relayers(test_lane_id()).unwrap().may_enact_at(),
			);
			assert_ok!(BridgeRelayers::advance_lane_epoch(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				test_lane_id(),
			));

			// and then deregister
			assert_ok!(BridgeRelayers::deregister_at_lane(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				test_lane_id()
			));
			assert_eq!(
				BridgeRelayers::registered_relayer(REGISTER_RELAYER).unwrap().valid_till(),
				None
			);
			assert_eq!(
				BridgeRelayers::registered_relayer(REGISTER_RELAYER)
					.unwrap()
					.valid_till_ignore_lanes(),
				150
			);
			assert_eq!(
				BridgeRelayers::registered_relayer(REGISTER_RELAYER).unwrap().lanes(),
				&[test_lane_id()]
			);
			assert_eq!(
				BridgeRelayers::active_lane_relayers(test_lane_id()).relayers(),
				&[RelayerAndReward::new(REGISTER_RELAYER, 0)]
			);
			assert_eq!(BridgeRelayers::next_lane_relayers(test_lane_id()).unwrap().relayers(), &[]);
		});
	}

	#[test]
	fn advance_lane_epoch_requires_signed_origin() {
		run_test(|| {
			assert_noop!(
				BridgeRelayers::advance_lane_epoch(RuntimeOrigin::root(), test_lane_id(),),
				DispatchError::BadOrigin
			);
		});
	}

	#[test]
	fn advance_lane_epoch_fails_if_lane_relayers_are_missing() {
		run_test(|| {
			assert_noop!(
				BridgeRelayers::advance_lane_epoch(
					RuntimeOrigin::signed(REGISTER_RELAYER),
					test_lane_id(),
				),
				Error::<TestRuntime>::NoRelayersAtLane
			);
		});
	}

	#[test]
	fn advance_lane_epoch_fails_if_next_set_may_not_be_enacted_yet() {
		run_test(|| {
			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER,
				registration(151, Stake::get() + LaneStake::get()),
			);
			assert_ok!(BridgeRelayers::register_at_lane(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				test_lane_id(),
				0,
			));

			assert_noop!(
				BridgeRelayers::advance_lane_epoch(
					RuntimeOrigin::signed(REGISTER_RELAYER),
					test_lane_id()
				),
				Error::<TestRuntime>::TooEarlyToActivateNextRelayersSet,
			);
		});
	}

	#[test]
	fn advance_lane_epoch_works() {
		run_test(|| {
			// when first relayer registers, we allow other relayers to register for
			// `InitialElectionLength` blocks
			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER,
				registration(151, Stake::get() + LaneStake::get()),
			);
			assert_ok!(BridgeRelayers::register_at_lane(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				test_lane_id(),
				0,
			));

			let next_lane_relayers = BridgeRelayers::next_lane_relayers(test_lane_id()).unwrap();
			assert_eq!(next_lane_relayers.may_enact_at(), InitialElectionLength::get());

			// when active epoch is advanced, new epoch starts at the block, where it has been
			// actually started, not the epoch where previous epoch was supposed to end
			System::<TestRuntime>::set_block_number(next_lane_relayers.may_enact_at() + 77);
			let result = BridgeRelayers::advance_lane_epoch(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				test_lane_id(),
			);
			assert_ok!(result);
			assert_eq!(result.unwrap().pays_fee, frame_support::dispatch::Pays::No);

			let next_lane_relayers = BridgeRelayers::next_lane_relayers(test_lane_id()).unwrap();
			assert_eq!(
				next_lane_relayers.may_enact_at(),
				InitialElectionLength::get() + 77 + EpochLength::get()
			);
		});
	}

	#[test]
	fn advance_lane_epoch_removes_dangling_lane_registrations() {
		run_test(|| {
			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER,
				registration(150, Stake::get() + LaneStake::get()),
			);

			// register at lane
			assert_ok!(BridgeRelayers::register_at_lane(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				test_lane_id(),
				0
			));

			// activate next lane epoch
			System::<TestRuntime>::set_block_number(
				BridgeRelayers::next_lane_relayers(test_lane_id()).unwrap().may_enact_at(),
			);
			assert_ok!(BridgeRelayers::advance_lane_epoch(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				test_lane_id(),
			));

			// and then deregister
			assert_ok!(BridgeRelayers::deregister_at_lane(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				test_lane_id()
			));

			// when the next epoch is activated, the lane registration is removed + registration is
			// prolonged
			let current_block_number =
				BridgeRelayers::next_lane_relayers(test_lane_id()).unwrap().may_enact_at();
			System::<TestRuntime>::set_block_number(current_block_number);
			assert_ok!(BridgeRelayers::advance_lane_epoch(
				RuntimeOrigin::signed(REGISTER_RELAYER),
				test_lane_id(),
			));

			// since relayer registration should have ended before, it is prolonged by
			// `required_registration_lease` blocks
			assert_eq!(
				BridgeRelayers::registered_relayer(REGISTER_RELAYER).unwrap().valid_till(),
				Some(current_block_number + Lease::get())
			);
			assert_eq!(BridgeRelayers::registered_relayer(REGISTER_RELAYER).unwrap().lanes(), &[]);
			assert_eq!(BridgeRelayers::active_lane_relayers(test_lane_id()).relayers(), &[]);
			assert_eq!(BridgeRelayers::next_lane_relayers(test_lane_id()).unwrap().relayers(), &[]);
		});
	}
}
