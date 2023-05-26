// Copyright 2022 Parity Technologies (UK) Ltd.
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

//! Wrappers for public types that are implementing `MaxEncodedLen`

use crate::{Config, Error};

use bp_aleph_header_chain::{AuthorityId, AuthoritySet, ChainWithAleph};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{traits::Get, BoundedVec, RuntimeDebugNoBound};
use scale_info::TypeInfo;

use sp_std::marker::PhantomData;


/// A list of AlephBFT authorities.
pub type AuthorityList = Vec<AuthorityId>;

/// A bounded list of AlephBFT authorities with associated weights.
pub type StoredAuthorityList<MaxBridgedAuthorities> =
	BoundedVec<AuthorityId, MaxBridgedAuthorities>;

/// Adapter for using `T::BridgedChain::MAX_BRIDGED_AUTHORITIES` in `BoundedVec`.
pub struct StoredAuthorityListLimit<T, I>(PhantomData<(T, I)>);

impl<T: Config<I>, I: 'static> Get<u32> for StoredAuthorityListLimit<T, I> {
	fn get() -> u32 {
		T::BridgedChain::MAX_AUTHORITIES_COUNT
	}
}

/// A bounded AlephBFT Authority List and ID.
#[derive(Clone, Decode, Encode, TypeInfo, MaxEncodedLen, RuntimeDebugNoBound)]
#[scale_info(skip_type_params(T, I))]
pub struct StoredAuthoritySet<T: Config<I>, I: 'static> {
	/// List of AlephBFT authorities for the current round.
	pub authorities: StoredAuthorityList<StoredAuthorityListLimit<T, I>>,
}

impl<T: Config<I>, I: 'static> StoredAuthoritySet<T, I> {
	/// Try to create a new bounded AlephBFT Authority Set from unbounded list.
	///
	/// Returns error if number of authorities in the provided list is too large.
	pub fn try_new(authorities: AuthorityList) -> Result<Self, Error<T, I>> {
		Ok(Self {
			authorities: TryFrom::try_from(authorities)
				.map_err(|_| Error::TooManyAuthoritiesInSet)?,
		})
	}
}

impl<T: Config<I>, I: 'static> Default for StoredAuthoritySet<T, I> {
	fn default() -> Self {
		StoredAuthoritySet { authorities: BoundedVec::default() }
	}
}

impl<T: Config<I>, I: 'static> From<StoredAuthoritySet<T, I>> for AuthoritySet {
	fn from(t: StoredAuthoritySet<T, I>) -> Self {
		t.authorities.into()
	}
}
