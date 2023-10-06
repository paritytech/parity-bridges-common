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

// TODO: allow bridge owners to add "protected" relayers that have a guaranteed slot in the lane relayers
// TODO: or else ONLY allow bridge owners to add registered relayers (through XCM calls)???

// TODO: lane registration must be time-limited to force relayers to renew it and drop relayers that have stopped
// working. Otherwise one relayer may fill up all lane slots for a fixed returnable sum.
//
// Or we may introduce some fine for relayer for not delivering messages. This is near to impossible though.
//
// Or we may allow to buy lane registration using relayer rewards only - i.e. has has delivered 100 messages
// and reward is 100 DOTs. He could reserve his 100 DOTs to buy lane registration slots. Every slot costs 1 DOT.
// So after 100 slots, the registration becomes inactive. And he must renew it.

// TODO: additionally we could add a reward market - i.e. now we have boosts:
// `messages_count * per_message + per_lane`. We could add another boost if relayer wants to receive lower reward.
// E.g. if normal reward is 1 DOT per message but relayers claims that he could deliver 10 messages in exchange of
// 1 DOT, we will prefer such transaction over transaction with 10 DOTs reward.

// TODO: better (easier code) handling of reserved funds. Separate calls?

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

use bp_messages::LaneId;
use bp_relayers::{
	LaneRelayersSet, PaymentProcedure, Registration, RelayerRewardsKeyProvider, RewardsAccountParams, StakeAndSlash,
};
use bp_runtime::StorageDoubleMapKeyProvider;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{DefaultNoBound, fail};
use frame_system::Pallet as SystemPallet;
use sp_arithmetic::traits::{AtLeast32BitUnsigned, Zero};
use scale_info::TypeInfo;
use sp_runtime::{traits::{CheckedSub, One}, BoundedVec, RuntimeDebug, Saturating};
use sp_std::{collections::vec_deque::VecDeque, marker::PhantomData};

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
		/// Type of relayer reward.
		type Reward: AtLeast32BitUnsigned + Copy + Member + Parameter + MaxEncodedLen;
		/// Pay rewards scheme.
		type PaymentProcedure: PaymentProcedure<Self::AccountId, Self::Reward>;

		/// Stake and slash scheme.
		type StakeAndSlash: StakeAndSlash<Self::AccountId, BlockNumberFor<Self>, Self::Reward>;

		/// TODO
		#[pallet::constant]
		type MaxLanesPerRelayer: Get<u32>;
		/// Maximal number of relayers that can register themselves on a single lane.
		#[pallet::constant]
		type MaxRelayersPerLane: Get<u32>;

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
				let mut registration = maybe_registration
					.take()
					.unwrap_or_else(|| Registration::new(valid_till));

				// new `valid_till` must be larger (or equal) than the old one
				ensure!(
					valid_till >= registration.valid_till,
					Error::<T>::CannotReduceRegistrationLease,
				);
				registration.valid_till = valid_till;

				// reserve stake on relayer account
				registration.stake = Self::update_relayer_stake(
					&relayer,
					registration.stake,
					registration.required_stake(
						Self::base_stake(),
						Self::stake_per_lane(),
					),
				)?;

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

				// we can't deregister until `valid_till + 1`
				ensure!(
					registration.valid_till < frame_system::Pallet::<T>::block_number(),
					Error::<T>::RegistrationIsStillActive,
				);

				// we can't deregister relayer that has registered itself for some lanes
				ensure!(
					registration.lanes.is_empty(),
					Error::<T>::HasLaneRegistrations,
				);

				// if stake is non-zero, we should do unreserve
				Self::update_relayer_stake(
					&relayer,
					registration.stake,
					Zero::zero(),
				)?;

				log::trace!(target: LOG_TARGET, "Successfully deregistered relayer: {:?}", relayer);
				Self::deposit_event(Event::<T>::Deregistered { relayer: relayer.clone() });

				*maybe_registration = None;

				Ok(())
			})
		}

		/// Register relayer intention to serve given messages lane.
		///
		/// Relayer that registers itself at given message lane gets a priority boost for his message
		/// delivery transactions, **verified** at his slots (consecutive range of blocks).
		#[pallet::call_index(3)]
		#[pallet::weight(Weight::zero())] // TODO
		pub fn register_at_lane(
			origin: OriginFor<T>,
			lane: LaneId,
			expected_reward: T::Reward,
		) -> DispatchResult {
			let relayer = ensure_signed(origin)?;

			// TODO: we probably need a way for bridge owners (sibling/parent chains) to at least set a maximal
			// possible reward for their lane over XCM? + maybe change relayers set? This way they could implement
			// their own incentivization mechanisms by setting reward to zero and changing relayers set on their own.

			// TODO: check that `expected_reward` makes sense

			RegisteredRelayers::<T>::try_mutate(&relayer.clone(), move |maybe_registration| -> DispatchResult {
				// we only allow registered relayers to have priority boosts
				let mut registration = match maybe_registration.take() {
					Some(registration) => registration,
					None => fail!(Error::<T>::NotRegistered),
				};

				// cannot add another lane registration if "base" registration is inactive
				let current_block_number = SystemPallet::<T>::block_number();
				ensure!(
					registration.is_active(
						Self::base_stake(),
						Self::stake_per_lane(),
						SystemPallet::<T>::block_number(),
						Self::required_registration_lease(),
					),
					Error::<T>::RegistrationIsInactive,
				);

				// cannot add another lane registration if relayer has already max allowed
				// lane registrations
				if !registration.lanes.contains(&lane) {
					ensure!(registration.lanes.try_push(lane).is_ok(), Error::<T>::TooManyLaneRegistrations);
				}

				// TODO: ideally we shall use the candle auction here (similar to parachain slot auctions)
				// let's try to claim a slot in the next set
				LaneRelayers::<T>::try_mutate(lane, |lane_relayers_ref| {
					let mut lane_relayers = match lane_relayers_ref.take() {
						Some(lane_relayers) => lane_relayers,
						None => {
							// TODO: give some time for initial elections
							// TODO: what if all relayers that have registered for the next set then call `deregister_at_lane`
							//       before `next_set` activates? This could be used by malicious relayers - they could fill
							//       the whole `next_set` and then clear it right before it is enacted. Think we shall allow more
							//       entries in the `mnext_set` so that it'll be harder for the attacker to fill the full queue.
							LaneRelayersSet::empty(
								SystemPallet::<T>::block_number().saturating_add(One::one()).saturating_add(4u32.into()), // TODO
							)
						}
					};

					ensure!(
						lane_relayers.next_set_try_push(relayer.clone(), expected_reward),
						Error::<T>::TooLargeRewardToOccupyAnEntry,
					);

					*lane_relayers_ref = Some(lane_relayers);

					Ok::<_, Error<T>>(())
				})?;

				// the relayer need to stake additional amount for every additional lane
				registration.stake = Self::update_relayer_stake(
					&relayer,
					registration.stake,
					registration.required_stake(
						Self::base_stake(),
						Self::stake_per_lane(),
					),
				)?;

				// cannot add duplicate lane registration
				// ensure!(!registration.lanes.contains(&lane), Error::<T>::DuplicateLaneRegistration);

				*maybe_registration = Some(registration);

				Ok(())
			})?;

			Ok(())
		}

		/// TODO
		#[pallet::call_index(4)]
		#[pallet::weight(Weight::zero())] // TODO
		pub fn deregister_at_lane(origin: OriginFor<T>, lane: LaneId) -> DispatchResult {
			let relayer = ensure_signed(origin)?;

			RegisteredRelayers::<T>::try_mutate(&relayer.clone(), move |maybe_registration| -> DispatchResult {
				// if relayer doesn't have a basic registration, we know that he is not registered
				// at the lane as well
				let mut registration = match maybe_registration.take() {
					Some(registration) => registration,
					None => fail!(Error::<T>::NotRegistered),
				};

				// ensure that the relayer has lane registration
				// ensure!(registration.lanes.remove(&lane), Error::<T>::UnregisteredAtLane);

				// remove relayer from the `next_set` of lane relayers. So relayer is still
				LaneRelayers::<T>::try_mutate(lane, |lane_relayers_ref| {
					let mut lane_relayers = match lane_relayers_ref.take() {
						Some(lane_relayers) => lane_relayers,
						None => fail!(Error::<T>::NotRegisteredAtLane),
					};

					ensure!(
						lane_relayers.next_set_try_remove(&relayer),
						Error::<T>::NotRegisteredAtLane,
					);

					*lane_relayers_ref = Some(lane_relayers);

					Ok::<_, Error<T>>(())
				})?;

				*maybe_registration = Some(registration);

				Ok(())
			})?;

			Ok(())
		}

		// TODO: add another `obsolete` extension for this call of the relayers pallet?
		/// Enact next set of relayers at a given lane.
		///
		/// This will replace the set of active relayers with the next scheduled set, for given lane. Anyone could
		/// call this method at any point. If the set will be changed, the cost of transaction will be refunded to
		/// the submitter. We do not provide any on-chain means to sync between relayers on who will submit this
		/// transaction, so first transaction from anyone will be accepted and it will have the zero cost. All
		/// subsequent transactions will be paid. We suggest the first relayer from the `next_set` to submit this
		/// transaction.
		#[pallet::call_index(5)]
		#[pallet::weight(Weight::zero())] // TODO
		pub fn enact_next_relayers_set_at_lane(origin: OriginFor<T>, lane: LaneId) -> DispatchResult {
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

				let new_next_set_may_enact_at = current_block_number.saturating_add(4u32.into()); // TODO
				lane_relayers.activate_next_set(
					new_next_set_may_enact_at,
					|relayer| Self::is_registration_active(relayer)
				);

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
					Self::base_stake(),
					Self::stake_per_lane(),
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
				registration.stake,
			) {
				Ok(failed_to_slash) if failed_to_slash.is_zero() => {
					log::trace!(
						target: crate::LOG_TARGET,
						"Relayer account {:?} has been slashed for {:?}. Funds were deposited to {:?}",
						relayer,
						registration.stake,
						slash_destination,
					);
				},
				Ok(failed_to_slash) => {
					log::trace!(
						target: crate::LOG_TARGET,
						"Relayer account {:?} has been partially slashed for {:?}. Funds were deposited to {:?}. \
						Failed to slash: {:?}",
						relayer,
						registration.stake,
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
						registration.stake,
						registration.stake,
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
			>>::RequiredStake::get() // TODO
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
		/// Failed to `deregister` relayer, because he has registered himself as a
		/// relayer at some lanes using `register_at_lane`. Lane registrations must
		/// be explicitly removed using `deregister_at_lane`
		HasLaneRegistrations,
		/// Failed to perform required action, because relayer registration is inactive.
		RegistrationIsInactive,
		/// Relayer is trying to register twice on the same lane.
		DuplicateLaneRegistration,
		/// Relayer has too many lane registrations.
		TooManyLaneRegistrations,
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
		LaneRelayersSet<T::AccountId, BlockNumberFor<T>, T::Reward, T::MaxRelayersPerLane>,
		OptionQuery,
	>;
}

#[cfg(test)]
mod tests {
	use super::*;
	use mock::{RuntimeEvent as TestEvent, *};

	use crate::Event::RewardPaid;
	use bp_messages::LaneId;
	use bp_relayers::RewardsAccountOwner;
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
				Some(Registration { valid_till: 150, stake: Stake::get(), lanes: BoundedVec::new() }),
			);

			assert_eq!(
				System::<TestRuntime>::events().last(),
				Some(&EventRecord {
					phase: Phase::Initialization,
					event: TestEvent::BridgeRelayers(Event::RegistrationUpdated {
						relayer: REGISTER_RELAYER,
						registration: Registration { valid_till: 150, stake: Stake::get(), lanes: BoundedVec::new() },
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
				Registration { valid_till: 150, stake: Stake::get() + 1, lanes: BoundedVec::new() },
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
				Registration { valid_till: 150, stake: Stake::get() + 1, lanes: BoundedVec::new() },
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
				Some(Registration { valid_till: 150, stake: Stake::get(), lanes: BoundedVec::new() }),
			);

			assert_eq!(
				System::<TestRuntime>::events().last(),
				Some(&EventRecord {
					phase: Phase::Initialization,
					event: TestEvent::BridgeRelayers(Event::RegistrationUpdated {
						relayer: REGISTER_RELAYER,
						registration: Registration { valid_till: 150, stake: Stake::get(), lanes: BoundedVec::new() }
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
				Registration { valid_till: 150, stake: Stake::get() - 1, lanes: BoundedVec::new() },
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
				Registration { valid_till: 150, stake: Stake::get() - 1, lanes: BoundedVec::new() },
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
				Some(Registration { valid_till: 150, stake: Stake::get(), lanes: BoundedVec::new() }),
			);

			assert_eq!(
				System::<TestRuntime>::events().last(),
				Some(&EventRecord {
					phase: Phase::Initialization,
					event: TestEvent::BridgeRelayers(Event::RegistrationUpdated {
						relayer: REGISTER_RELAYER,
						registration: Registration { valid_till: 150, stake: Stake::get(), lanes: BoundedVec::new() }
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
	fn is_registration_active_is_false_when_stake_is_too_low() {
		run_test(|| {
			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER,
				Registration { valid_till: 150, stake: Stake::get() - 1, lanes: BoundedVec::new() },
			);
			assert!(!Pallet::<TestRuntime>::is_registration_active(&REGISTER_RELAYER));
		});
	}

	#[test]
	fn is_registration_active_is_false_when_remaining_lease_is_too_low() {
		run_test(|| {
			System::<TestRuntime>::set_block_number(150 - Lease::get());

			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER,
				Registration { valid_till: 150, stake: Stake::get(), lanes: BoundedVec::new() },
			);
			assert!(!Pallet::<TestRuntime>::is_registration_active(&REGISTER_RELAYER));
		});
	}

	#[test]
	fn is_registration_active_is_true_when_relayer_is_properly_registeered() {
		run_test(|| {
			System::<TestRuntime>::set_block_number(150 - Lease::get());

			RegisteredRelayers::<TestRuntime>::insert(
				REGISTER_RELAYER,
				Registration { valid_till: 151, stake: Stake::get(), lanes: BoundedVec::new() },
			);
			assert!(Pallet::<TestRuntime>::is_registration_active(&REGISTER_RELAYER));
		});
	}
}
