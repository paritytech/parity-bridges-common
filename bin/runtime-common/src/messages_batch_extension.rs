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

//! Signed extension that refunds relayer if he has delivered some new messages.
//! It also refunds transacation cost if the transaction is an `utility.batchAll()`
//! with calls that are: delivering new messsage and all necessary underlying headers
//! (parachain or relay chain).

use codec::{Decode, Encode};
use frame_support::{dispatch::Dispatchable, Parameter, RuntimeDebug};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{DispatchInfoOf, PostDispatchInfoOf, SignedExtension},
	transaction_validity::{TransactionValidity, TransactionValidityError, ValidTransaction},
	DispatchResult,
};
use sp_std::marker::PhantomData;

// TODO: is it possible to impl it for several bridges at once? Like what we have in
// `BridgeRejectObsoleteHeadersAndMessages`? If it is hard to do now - just submit an issue

#[derive(Clone, Decode, Encode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct RefundRelayerForMessagesDeliveryFromParachain<AccountId, Call>(
	PhantomData<(AccountId, Call)>,
);

impl<AccountId, Call> SignedExtension
	for RefundRelayerForMessagesDeliveryFromParachain<AccountId, Call>
where
	AccountId: 'static + Parameter + Send + Sync,
	Call: 'static + Dispatchable + Parameter + Send + Sync,
{
	const IDENTIFIER: &'static str = "RefundRelayerForMessagesDeliveryFromParachain";
	type AccountId = AccountId;
	type Call = Call;
	type AdditionalSigned = ();
	type Pre = ();

	fn additional_signed(&self) -> Result<(), TransactionValidityError> {
		Ok(())
	}

	fn validate(
		&self,
		_who: &Self::AccountId,
		_call: &Self::Call,
		_info: &DispatchInfoOf<Self::Call>,
		_len: usize,
	) -> TransactionValidity {
		Ok(ValidTransaction::default())
	}

	fn pre_dispatch(
		self,
		_who: &Self::AccountId,
		_call: &Self::Call,
		_post_info: &DispatchInfoOf<Self::Call>,
		_len: usize,
	) -> Result<(), TransactionValidityError> {
		// TODO: for every call from the batch - call the `BridgesExtension` to ensure that every
		// call transaction brings something new and reject obsolete transactions
		Ok(())
	}

	fn post_dispatch(
		_pre: Option<Self::Pre>,
		_info: &DispatchInfoOf<Self::Call>,
		_post_info: &PostDispatchInfoOf<Self::Call>,
		_len: usize,
		_result: &DispatchResult,
	) -> Result<(), TransactionValidityError> {
		// TODO:
		// if batch matches!(relay-chain-finality-call, parachain-finality-call,
		// deliver-messages-call) then:
		//
		// 0) ensure that the `result` is `Ok(_)`
		//
		// 1) ensure that all calls are dependent (relaychain -> parachain of this relaychain and
		// proof is using new relaychain header -> messages of this parachain and proof is using new
		// parachain head). Otherwise - no refund!!!
		//
		// 2) ensure that the parachain finality transaction is for single parachain (our special
		// case). Otherwise - no refund!!!
		//
		// 3) check result of every call - e.g. parachain finality call succeeds even if it is not
		// updating anything. But we are only interesting in refunding calls that have succeeded. If
		// at least one call has not succeeded - no refund!!!
		//
		// 4) now we may refund using `pallet-bridge-relayers`. The sum of refund is
		// `pallet_transaction_payment::Pallet::<Runtime>::compute_actual_fee(len as u32, info,
		// post_info, tip)`

		// TODO:
		// if batch matches!(parachain-finality-call, deliver-messages-call) then:
		//
		// 0) ensure that the `result` is `Ok(_)`
		//
		// 1) ensure that all calls are dependent (parachain -> messages of this parachain and proof
		// is using new parachain head). Otherwise - no refund!!!
		//
		// 2) ensure that the parachain finality transaction is for single parachain (our special
		// case). Otherwise - no refund!!!
		//
		// 3) check result of every call - e.g. parachain finality call succeeds even if it is not
		// updating anything. But we are only interesting in refunding calls that have succeeded. If
		// at least one call has not succeeded - no refund!!!
		//
		// 4) now we may refund using
		// `pallet-bridge-relayers`. The sum of refund is
		// `pallet_transaction_payment::Pallet::<Runtime>::compute_actual_fee(len as u32, info,
		// post_info, tip)`

		// TODO: if the call is just deliver-message-call then:
		//
		// 0) ensure that the `result` is `Ok(_)`
		//
		// 1) check that at least some messages were accepted. Otherwise - no refund!!!;
		//
		// 2) now we may refund using `pallet-bridge-relayers`. The sum of refund is
		// `pallet_transaction_payment::Pallet::<Runtime>::compute_actual_fee(len as u32, info,
		// post_info, tip)`

		Ok(())
	}
}
