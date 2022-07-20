// Copyright 2019-2021 Parity Technologies (UK) Ltd.
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

//! Common types/functions that may be used by runtimes of all bridged chains.

#![cfg_attr(not(feature = "std"), no_std)]

pub mod messages;
pub mod messages_api;
pub mod messages_benchmarking;
pub mod messages_extension;
pub mod parachains_benchmarking;

#[cfg(feature = "integrity-test")]
pub mod integrity;

#[macro_export]
macro_rules! generate_reject_obsolete_headers_and_messages {
	($runtime:ident, $($filter_call:ty),*) => {
		#[derive(Clone, codec::Decode, codec::Encode, Eq, PartialEq, frame_support::RuntimeDebug, scale_info::TypeInfo)]
		pub struct RejectObsoleteHeadersAndMessages;
		impl sp_runtime::traits::SignedExtension for RejectObsoleteHeadersAndMessages {
			const IDENTIFIER: &'static str = "BridgeRejectObsoleteHeadersAndMessages";
			type AccountId = <$runtime as frame_system::Config>::AccountId;
			type Call = <$runtime as frame_system::Config>::Call;
			type AdditionalSigned = ();
			type Pre = ();

			fn additional_signed(&self) -> sp_std::result::Result<
				(),
				sp_runtime::transaction_validity::TransactionValidityError,
			> {
				Ok(())
			}

			fn validate(
				&self,
				_who: &Self::AccountId,
				call: &Self::Call,
				_info: &DispatchInfoOf<Self::Call>,
				_len: usize,
			) -> sp_runtime::transaction_validity::TransactionValidity {
				use bp_runtime::FilterCall;

				let valid = sp_runtime::transaction_validity::ValidTransaction::default();
				$(let valid = valid.combine_with(<$filter_call>::validate(call)?);)*
				Ok(valid)
			}

			fn pre_dispatch(
				self,
				who: &Self::AccountId,
				call: &Self::Call,
				info: &sp_runtime::traits::DispatchInfoOf<Self::Call>,
				len: usize,
			) -> Result<Self::Pre, sp_runtime::transaction_validity::TransactionValidityError> {
				self.validate(who, call, info, len).map(drop)
			}
		}
	};
}
