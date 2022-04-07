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

//! # Fee Market Pallet

#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "128"]

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
mod tests;

pub mod weight;
pub use weight::WeightInfo;

pub mod s2s;
pub mod types;

// --- paritytech ---
use bp_messages::{LaneId, MessageNonce};
use frame_support::{
	ensure,
	pallet_prelude::*,
	traits::{Currency, Get, LockIdentifier, LockableCurrency, WithdrawReasons},
	transactional, PalletId,
};
use frame_system::{ensure_signed, pallet_prelude::*};
use sp_runtime::{
	traits::{Saturating, Zero},
	Permill, SaturatedConversion,
};
use sp_std::vec::Vec;
// --- darwinia-network ---
use types::{Order, Relayer, SlashReport};

pub type AccountId<T> = <T as frame_system::Config>::AccountId;
pub type RingBalance<T> = <<T as Config>::RingCurrency as Currency<AccountId<T>>>::Balance;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		#[pallet::constant]
		type PalletId: Get<PalletId>;
		/// Some reward goes to Treasury.
		#[pallet::constant]
		type TreasuryPalletId: Get<PalletId>;
		#[pallet::constant]
		type LockId: Get<LockIdentifier>;

		/// The minimum fee for relaying.
		#[pallet::constant]
		type MinimumRelayFee: Get<RingBalance<Self>>;
		/// The collateral relayer need to lock for each order.
		#[pallet::constant]
		type CollateralPerOrder: Get<RingBalance<Self>>;
		/// The slot times set
		#[pallet::constant]
		type Slot: Get<Self::BlockNumber>;

		/// Reward parameters
		#[pallet::constant]
		type AssignedRelayersRewardRatio: Get<Permill>;
		#[pallet::constant]
		type MessageRelayersRewardRatio: Get<Permill>;
		#[pallet::constant]
		type ConfirmRelayersRewardRatio: Get<Permill>;

		/// The slash rule
		type Slasher: Slasher<Self>;
		type RingCurrency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Relayer enrollment. \[account_id, locked_collateral, relay_fee\]
		Enroll(T::AccountId, RingBalance<T>, RingBalance<T>),
		/// Update relayer locked collateral. \[account_id, new_collateral\]
		UpdateLockedCollateral(T::AccountId, RingBalance<T>),
		/// Update relayer fee. \[account_id, new_fee\]
		UpdateRelayFee(T::AccountId, RingBalance<T>),
		/// Relayer cancel enrollment. \[account_id\]
		CancelEnrollment(T::AccountId),
		/// Update collateral slash protect value. \[slash_protect_value\]
		UpdateCollateralSlashProtect(RingBalance<T>),
		/// Update market assigned relayers numbers. \[new_assigned_relayers_number\]
		UpdateAssignedRelayersNumber(u32),
		/// Slash report
		FeeMarketSlash(SlashReport<T::AccountId, T::BlockNumber, RingBalance<T>>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Insufficient balance.
		InsufficientBalance,
		/// The relayer has been enrolled.
		AlreadyEnrolled,
		/// This relayer doesn't enroll ever.
		NotEnrolled,
		/// Update locked collateral is not allow since some orders are not confirm.
		StillHasOrdersNotConfirmed,
		/// The fee is lower than MinimumRelayFee.
		RelayFeeTooLow,
		/// The relayer is occupied, and can't cancel enrollment now.
		OccupiedRelayer,
	}

	// Enrolled relayers storage
	#[pallet::storage]
	#[pallet::getter(fn relayer)]
	pub type RelayersMap<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Relayer<T::AccountId, RingBalance<T>>,
		OptionQuery,
	>;
	#[pallet::storage]
	#[pallet::getter(fn relayers)]
	pub type Relayers<T: Config> = StorageValue<_, Vec<T::AccountId>, OptionQuery>;

	// Priority relayers storage
	#[pallet::storage]
	#[pallet::getter(fn assigned_relayers)]
	pub type AssignedRelayers<T: Config> =
		StorageValue<_, Vec<Relayer<T::AccountId, RingBalance<T>>>, OptionQuery>;

	// Order storage
	#[pallet::storage]
	#[pallet::getter(fn order)]
	pub type Orders<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		(LaneId, MessageNonce),
		Order<T::AccountId, T::BlockNumber, RingBalance<T>>,
		OptionQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn collateral_slash_protect)]
	pub type CollateralSlashProtect<T: Config> = StorageValue<_, RingBalance<T>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn assigned_relayers_number)]
	pub type AssignedRelayersNumber<T: Config> =
		StorageValue<_, u32, ValueQuery, DefaultAssignedRelayersNumber>;
	#[pallet::type_value]
	pub fn DefaultAssignedRelayersNumber() -> u32 {
		3
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);
	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_finalize(_: BlockNumberFor<T>) {
			for ((lane_id, message_nonce), order) in <Orders<T>>::iter() {
				// Once the order's confirm_time is not None, we consider this order has been
				// rewarded. Hence, clean the storage.
				if order.confirm_time.is_some() {
					<Orders<T>>::remove((lane_id, message_nonce));
				}
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Any accounts can enroll to be a relayer by lock collateral. The relay fee is optional,
		/// the default value is MinimumRelayFee in runtime. (Update market needed)
		/// Note: One account can enroll only once.
		#[pallet::weight(<T as Config>::WeightInfo::enroll_and_lock_collateral())]
		#[transactional]
		pub fn enroll_and_lock_collateral(
			origin: OriginFor<T>,
			lock_collateral: RingBalance<T>,
			relay_fee: Option<RingBalance<T>>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			ensure!(!Self::is_enrolled(&who), <Error<T>>::AlreadyEnrolled);

			ensure!(
				T::RingCurrency::free_balance(&who) >= lock_collateral,
				<Error<T>>::InsufficientBalance
			);
			if let Some(fee) = relay_fee {
				ensure!(fee >= T::MinimumRelayFee::get(), <Error<T>>::RelayFeeTooLow);
			}
			let fee = relay_fee.unwrap_or_else(T::MinimumRelayFee::get);

			T::RingCurrency::set_lock(
				T::LockId::get(),
				&who,
				lock_collateral,
				WithdrawReasons::all(),
			);
			// Store enrollment detail information.
			<RelayersMap<T>>::insert(&who, Relayer::new(who.clone(), lock_collateral, fee));
			<Relayers<T>>::append(&who);

			Self::update_market();
			Self::deposit_event(Event::<T>::Enroll(who, lock_collateral, fee));
			Ok(().into())
		}

		/// Update locked collateral for enrolled relayer, only supporting lock more. (Update market
		/// needed)
		#[pallet::weight(<T as Config>::WeightInfo::update_locked_collateral())]
		#[transactional]
		pub fn update_locked_collateral(
			origin: OriginFor<T>,
			new_collateral: RingBalance<T>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			ensure!(Self::is_enrolled(&who), <Error<T>>::NotEnrolled);
			ensure!(
				T::RingCurrency::free_balance(&who) >= new_collateral,
				<Error<T>>::InsufficientBalance
			);

			// Increase the locked collateral
			if new_collateral >= Self::relayer_locked_collateral(&who) {
				T::RingCurrency::set_lock(
					T::LockId::get(),
					&who,
					new_collateral,
					WithdrawReasons::all(),
				);
			} else {
				// Decrease the locked collateral
				if let Some((_, orders_locked_collateral)) = Self::occupied(&who) {
					ensure!(
						new_collateral >= orders_locked_collateral,
						<Error<T>>::StillHasOrdersNotConfirmed
					);

					T::RingCurrency::remove_lock(T::LockId::get(), &who);
					T::RingCurrency::set_lock(
						T::LockId::get(),
						&who,
						new_collateral,
						WithdrawReasons::all(),
					);
				}
			}

			<RelayersMap<T>>::mutate(who.clone(), |relayer| {
				if let Some(ref mut r) = relayer {
					r.collateral = new_collateral;
				}
			});
			Self::update_market();
			Self::deposit_event(Event::<T>::UpdateLockedCollateral(who, new_collateral));
			Ok(().into())
		}

		/// Update relay fee for enrolled relayer. (Update market needed)
		#[pallet::weight(<T as Config>::WeightInfo::update_relay_fee())]
		#[transactional]
		pub fn update_relay_fee(
			origin: OriginFor<T>,
			new_fee: RingBalance<T>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			ensure!(Self::is_enrolled(&who), <Error<T>>::NotEnrolled);
			ensure!(new_fee >= T::MinimumRelayFee::get(), <Error<T>>::RelayFeeTooLow);

			<RelayersMap<T>>::mutate(who.clone(), |relayer| {
				if let Some(ref mut r) = relayer {
					r.fee = new_fee;
				}
			});

			Self::update_market();
			Self::deposit_event(Event::<T>::UpdateRelayFee(who, new_fee));
			Ok(().into())
		}

		/// Cancel enrolled relayer(Update market needed)
		#[pallet::weight(<T as Config>::WeightInfo::cancel_enrollment())]
		#[transactional]
		pub fn cancel_enrollment(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			ensure!(Self::is_enrolled(&who), <Error<T>>::NotEnrolled);
			ensure!(Self::occupied(&who).is_none(), <Error<T>>::OccupiedRelayer);

			Self::remove_enrolled_relayer(&who);
			Self::deposit_event(Event::<T>::CancelEnrollment(who));
			Ok(().into())
		}

		#[pallet::weight(<T as Config>::WeightInfo::set_slash_protect())]
		#[transactional]
		pub fn set_slash_protect(
			origin: OriginFor<T>,
			slash_protect: RingBalance<T>,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			CollateralSlashProtect::<T>::put(slash_protect);
			Self::deposit_event(Event::<T>::UpdateCollateralSlashProtect(slash_protect));
			Ok(().into())
		}

		#[pallet::weight(<T as Config>::WeightInfo::set_assigned_relayers_number())]
		#[transactional]
		pub fn set_assigned_relayers_number(
			origin: OriginFor<T>,
			number: u32,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			AssignedRelayersNumber::<T>::put(number);

			Self::update_market();
			Self::deposit_event(Event::<T>::UpdateAssignedRelayersNumber(number));
			Ok(().into())
		}
	}
}
pub use pallet::*;

impl<T: Config> Pallet<T> {
	/// An important update in this pallet, need to update market information in the following
	/// cases:
	///
	/// - New relayer enroll.
	/// - The enrolled relayer wants to update fee or order capacity.
	/// - The enrolled relayer wants to cancel enrollment.
	/// - The order didn't confirm in-time, slash occurred.
	pub(crate) fn update_market() {
		// Sort all enrolled relayers who are able to accept orders.
		let mut relayers: Vec<Relayer<T::AccountId, RingBalance<T>>> = Vec::new();
		if let Some(ids) = <Relayers<T>>::get() {
			for id in ids.iter() {
				if let Some(r) = RelayersMap::<T>::get(id) {
					if Self::usable_order_capacity(&r.id) >= 1 {
						relayers.push(r)
					}
				}
			}
		}

		// Select the first `AssignedRelayersNumber` relayers as AssignedRelayer.
		let assigned_relayers_len = <AssignedRelayersNumber<T>>::get() as usize;
		if relayers.len() >= assigned_relayers_len {
			relayers.sort();

			let assigned_relayers: Vec<_> = relayers.iter().take(assigned_relayers_len).collect();
			<AssignedRelayers<T>>::put(assigned_relayers);
		} else {
			// The market fee comes from the last item in AssignedRelayers,
			// It's would be essential to wipe this storage if relayers not enough.
			<AssignedRelayers<T>>::kill();
		}
	}

	/// Update relayer after slash occurred, this will changes RelayersMap storage. (Update market
	/// needed)
	pub(crate) fn update_relayer_after_slash(
		who: &T::AccountId,
		new_collateral: RingBalance<T>,
		report: SlashReport<T::AccountId, T::BlockNumber, RingBalance<T>>,
	) {
		T::RingCurrency::set_lock(T::LockId::get(), &who, new_collateral, WithdrawReasons::all());
		<RelayersMap<T>>::mutate(who.clone(), |relayer| {
			if let Some(ref mut r) = relayer {
				r.collateral = new_collateral;
			}
		});

		Self::update_market();
		Self::deposit_event(<Event<T>>::FeeMarketSlash(report));
	}

	/// Remove enrolled relayer, then update market fee. (Update market needed)
	pub(crate) fn remove_enrolled_relayer(who: &T::AccountId) {
		T::RingCurrency::remove_lock(T::LockId::get(), who);

		<RelayersMap<T>>::remove(who.clone());
		<Relayers<T>>::mutate(|relayers| {
			if let Some(ref mut r) = relayers {
				r.retain(|x| x != who)
			}
		});
		<AssignedRelayers<T>>::mutate(|assigned_relayers| {
			if let Some(relayers) = assigned_relayers {
				relayers.retain(|x| x.id != *who);
			}
		});
		Self::update_market();
	}

	/// Whether the relayer has enrolled
	pub(crate) fn is_enrolled(who: &T::AccountId) -> bool {
		<Relayers<T>>::get().map_or(false, |rs| rs.iter().any(|r| *r == *who))
	}

	/// Get market fee, If there is not enough relayers have order capacity to accept new order,
	/// return None.
	pub fn market_fee() -> Option<RingBalance<T>> {
		Self::assigned_relayers().and_then(|relayers| relayers.last().map(|r| r.fee))
	}

	/// Get order indexes in the storage
	pub fn in_process_orders() -> Vec<(LaneId, MessageNonce)> {
		Orders::<T>::iter().map(|(k, _v)| k).collect()
	}

	/// Get the relayer locked collateral value
	pub fn relayer_locked_collateral(who: &T::AccountId) -> RingBalance<T> {
		RelayersMap::<T>::get(who).map_or(RingBalance::<T>::zero(), |r| r.collateral)
	}

	/// Whether the enrolled relayer is occupied(Responsible for order relaying)
	/// Whether the enrolled relayer is occupied, If occupied, return the number of orders and
	/// orders locked collateral, otherwise, return None.
	pub(crate) fn occupied(who: &T::AccountId) -> Option<(u32, RingBalance<T>)> {
		let mut count = 0u32;
		let mut orders_locked_collateral = RingBalance::<T>::zero();
		for (_, order) in <Orders<T>>::iter() {
			if order.relayers_slice().iter().any(|r| r.id == *who) && !order.is_confirmed() {
				count += 1;
				orders_locked_collateral =
					orders_locked_collateral.saturating_add(order.locked_collateral);
			}
		}

		if count == 0 {
			return None
		}
		Some((count, orders_locked_collateral))
	}

	/// The relayer collateral is composed of two part: fee_collateral and orders_locked_collateral.
	/// Calculate the order capacity with fee_collateral
	pub(crate) fn usable_order_capacity(who: &T::AccountId) -> u32 {
		let relayer_locked_collateral = Self::relayer_locked_collateral(&who);
		if let Some((_, orders_locked_collateral)) = Self::occupied(&who) {
			let free_collateral =
				relayer_locked_collateral.saturating_sub(orders_locked_collateral);
			return Self::collateral_to_order_capacity(free_collateral)
		}
		Self::collateral_to_order_capacity(relayer_locked_collateral)
	}

	fn collateral_to_order_capacity(collateral: RingBalance<T>) -> u32 {
		(collateral / T::CollateralPerOrder::get()).saturated_into::<u32>()
	}
}

pub trait Slasher<T: Config> {
	fn slash(locked_collateral: RingBalance<T>, timeout: T::BlockNumber) -> RingBalance<T>;
}
