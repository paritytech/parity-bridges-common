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

// --- paritytech ---
use bp_messages::{
	source_chain::{MessageDeliveryAndDispatchPayment, SenderOrigin},
	MessageNonce, UnrewardedRelayer,
};
use frame_support::{
	log,
	traits::{Currency as CurrencyT, ExistenceRequirement, Get},
};
use sp_runtime::traits::{AccountIdConversion, Saturating, Zero};
use sp_std::{
	collections::{btree_map::BTreeMap, vec_deque::VecDeque},
	ops::RangeInclusive,
};
// --- darwinia-network ---
use crate::{Config, Orders, Pallet, *};

/// Error that occurs when message fee is non-zero, but payer is not defined.
const NON_ZERO_MESSAGE_FEE_CANT_BE_PAID_BY_NONE: &str =
	"Non-zero message fee can't be paid by <None>";

pub struct FeeMarketPayment<T, I, Currency> {
	_phantom: sp_std::marker::PhantomData<(T, I, Currency)>,
}

impl<T, I, Currency> MessageDeliveryAndDispatchPayment<T::Origin, T::AccountId, RingBalance<T>>
	for FeeMarketPayment<T, I, Currency>
where
	T: frame_system::Config + pallet_bridge_messages::Config<I> + Config,
	I: 'static,
	T::Origin: SenderOrigin<T::AccountId>,
	Currency: CurrencyT<T::AccountId, Balance = T::OutboundMessageFee>,
{
	type Error = &'static str;

	fn pay_delivery_and_dispatch_fee(
		submitter: &T::Origin,
		fee: &RingBalance<T>,
		relayer_fund_account: &T::AccountId,
	) -> Result<(), Self::Error> {
		let submitter_account = match submitter.linked_account() {
			Some(submitter_account) => submitter_account,
			None if !fee.is_zero() => {
				// if we'll accept some message that has declared that the `fee` has been paid but
				// it isn't actually paid, then it'll lead to problems with delivery confirmation
				// payments (see `pay_relayer_rewards` && `confirmation_relayer` in particular)
				return Err(NON_ZERO_MESSAGE_FEE_CANT_BE_PAID_BY_NONE)
			},
			None => {
				// message lane verifier has accepted the message before, so this message
				// is unpaid **by design**
				// => let's just do nothing
				return Ok(())
			},
		};

		<T as Config>::RingCurrency::transfer(
			&submitter_account,
			relayer_fund_account,
			*fee,
			// it's fine for the submitter to go below Existential Deposit and die.
			ExistenceRequirement::AllowDeath,
		)
		.map_err(Into::into)
	}

	fn pay_relayers_rewards(
		lane_id: LaneId,
		messages_relayers: VecDeque<UnrewardedRelayer<T::AccountId>>,
		confirmation_relayer: &T::AccountId,
		received_range: &RangeInclusive<MessageNonce>,
		relayer_fund_account: &T::AccountId,
	) {
		let RewardsBook {
			messages_relayers_rewards,
			confirmation_relayer_rewards,
			assigned_relayers_rewards,
			treasury_total_rewards,
		} = slash_and_calculate_rewards::<T, I>(
			lane_id,
			messages_relayers,
			received_range,
			relayer_fund_account,
		);

		// Pay confirmation relayer rewards
		do_reward::<T>(relayer_fund_account, confirmation_relayer, confirmation_relayer_rewards);
		// Pay messages relayers rewards
		for (relayer, reward) in messages_relayers_rewards {
			do_reward::<T>(relayer_fund_account, &relayer, reward);
		}
		// Pay assign relayer reward
		for (relayer, reward) in assigned_relayers_rewards {
			do_reward::<T>(relayer_fund_account, &relayer, reward);
		}
		// Pay treasury reward
		do_reward::<T>(
			relayer_fund_account,
			&T::TreasuryPalletId::get().into_account(),
			treasury_total_rewards,
		);
	}
}

/// Slash and calculate rewards for messages_relayers, confirmation relayers, treasury,
/// assigned_relayers
pub fn slash_and_calculate_rewards<T, I>(
	lane_id: LaneId,
	messages_relayers: VecDeque<UnrewardedRelayer<T::AccountId>>,
	received_range: &RangeInclusive<MessageNonce>,
	relayer_fund_account: &T::AccountId,
) -> RewardsBook<T::AccountId, RingBalance<T>>
where
	T: frame_system::Config + pallet_bridge_messages::Config<I> + Config,
	I: 'static,
{
	let mut confirmation_rewards = RingBalance::<T>::zero();
	let mut messages_rewards = BTreeMap::<T::AccountId, RingBalance<T>>::new();
	let mut assigned_relayers_rewards = BTreeMap::<T::AccountId, RingBalance<T>>::new();
	let mut treasury_total_rewards = RingBalance::<T>::zero();

	for entry in messages_relayers {
		let nonce_begin = sp_std::cmp::max(entry.messages.begin, *received_range.start());
		let nonce_end = sp_std::cmp::min(entry.messages.end, *received_range.end());

		for message_nonce in nonce_begin..nonce_end + 1 {
			// The order created when message was accepted, so we can always get the order info
			// below.
			if let Some(order) = <Orders<T>>::get(&(lane_id, message_nonce)) {
				// The confirm_time of the order is set in the `OnDeliveryConfirmed` callback. And
				// the callback function was called as source chain received message delivery proof,
				// before the reward payment.
				let order_confirm_time =
					order.confirm_time.unwrap_or_else(|| frame_system::Pallet::<T>::block_number());
				let message_fee = order.fee();

				let message_reward;
				let confirm_reward;

				if let Some((who, base_fee)) =
					order.required_delivery_relayer_for_time(order_confirm_time)
				{
					// message fee - base fee => treasury
					let treasury_reward = message_fee.saturating_sub(base_fee);
					treasury_total_rewards = treasury_total_rewards.saturating_add(treasury_reward);

					// 60% * base fee => assigned_relayers_rewards
					let assigned_relayers_reward = T::AssignedRelayersRewardRatio::get() * base_fee;
					assigned_relayers_rewards
						.entry(who)
						.and_modify(|r| *r = r.saturating_add(assigned_relayers_reward))
						.or_insert(assigned_relayers_reward);

					let bridger_relayers_reward = base_fee.saturating_sub(assigned_relayers_reward);

					// 80% * (1 - 60%) * base_fee => message relayer
					message_reward = T::MessageRelayersRewardRatio::get() * bridger_relayers_reward;
					// 20% * (1 - 60%) * base_fee => confirm relayer
					confirm_reward = T::ConfirmRelayersRewardRatio::get() * bridger_relayers_reward;
				} else {
					// The order delivery is delay
					let mut total_slash = message_fee;

					// calculate slash amount
					let mut amount: RingBalance<T> = T::Slasher::slash(
						order.locked_collateral,
						order.delivery_delay().unwrap_or_default(),
					);
					if let Some(slash_protect) = Pallet::<T>::collateral_slash_protect() {
						amount = sp_std::cmp::min(amount, slash_protect);
					}

					// Slash order's assigned relayers
					let mut assigned_relayers_slash = RingBalance::<T>::zero();
					for assigned_relayer in order.relayers_slice() {
						let report = SlashReport::new(&order, assigned_relayer.id.clone(), amount);
						let slashed = do_slash::<T>(
							&assigned_relayer.id,
							relayer_fund_account,
							amount,
							report,
						);
						assigned_relayers_slash += slashed;
					}
					total_slash += assigned_relayers_slash;

					// 80% total slash => confirm relayer
					message_reward = T::MessageRelayersRewardRatio::get() * total_slash;
					// 20% total slash => confirm relayer
					confirm_reward = T::ConfirmRelayersRewardRatio::get() * total_slash;
				}

				// Update confirmation relayer total rewards
				confirmation_rewards = confirmation_rewards.saturating_add(confirm_reward);
				// Update message relayers total rewards
				messages_rewards
					.entry(entry.relayer.clone())
					.and_modify(|r| *r = r.saturating_add(message_reward))
					.or_insert(message_reward);
			}
		}
	}

	RewardsBook {
		messages_relayers_rewards: messages_rewards,
		confirmation_relayer_rewards: confirmation_rewards,
		assigned_relayers_rewards,
		treasury_total_rewards,
	}
}

/// Do slash for absent assigned relayers
pub(crate) fn do_slash<T: Config>(
	who: &T::AccountId,
	fund_account: &T::AccountId,
	amount: RingBalance<T>,
	report: SlashReport<T::AccountId, T::BlockNumber, RingBalance<T>>,
) -> RingBalance<T> {
	let locked_collateral = Pallet::<T>::relayer_locked_collateral(&who);
	T::RingCurrency::remove_lock(T::LockId::get(), &who);
	debug_assert!(
		locked_collateral >= amount,
		"The locked collateral must alway greater than slash max"
	);

	let pay_result = <T as Config>::RingCurrency::transfer(
		who,
		fund_account,
		amount,
		ExistenceRequirement::AllowDeath,
	);
	match pay_result {
		Ok(_) => {
			crate::Pallet::<T>::update_relayer_after_slash(
				who,
				locked_collateral.saturating_sub(amount),
				report,
			);
			log::trace!("Slash {:?} amount: {:?}", who, amount);
			return amount
		},
		Err(e) => {
			crate::Pallet::<T>::update_relayer_after_slash(who, locked_collateral, report);
			log::error!("Slash {:?} amount {:?}, err {:?}", who, amount, e)
		},
	}

	RingBalance::<T>::zero()
}

/// Do reward
pub(crate) fn do_reward<T: Config>(from: &T::AccountId, to: &T::AccountId, reward: RingBalance<T>) {
	if reward.is_zero() {
		return
	}

	let pay_result = <T as Config>::RingCurrency::transfer(
		from,
		to,
		reward,
		// the relayer fund account must stay above ED (needs to be pre-funded)
		ExistenceRequirement::KeepAlive,
	);

	match pay_result {
		Ok(_) => log::trace!("Reward, from {:?} to {:?} reward: {:?}", from, to, reward),
		Err(e) => log::error!("Reward, from {:?} to {:?} reward {:?}: {:?}", from, to, reward, e,),
	}
}

/// Record the calculation rewards result
pub struct RewardsBook<AccountId, Balance> {
	pub messages_relayers_rewards: BTreeMap<AccountId, Balance>,
	pub confirmation_relayer_rewards: Balance,
	pub assigned_relayers_rewards: BTreeMap<AccountId, Balance>,
	pub treasury_total_rewards: Balance,
}
