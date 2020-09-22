// Copyright 2019-2020 Parity Technologies (UK) Ltd.
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

use crate::client::Client;

use jsonrpsee::common::{DeserializeOwned, Serialize};
use sp_core::Pair;
use sp_runtime::traits::Member;

/// Substrate-based chain from minimal relay-client point of view.
pub trait Chain: frame_system::Trait {
	/// Block type.
	type SignedBlock: Member + Serialize + DeserializeOwned;
}

/// Substrate-based chain transactions signing scheme.
pub trait TransactionSignScheme {
	/// Chain that this scheme is to be used.
	type Chain: Chain;
	/// Type of key pairs used to sign transactions.
	type AccountKeyPair: Pair;
	/// Signed transaction.
	type SignedTransaction;

	/// Create transaction for given runtime call, signed by given account.
	fn sign_transaction(
		client: &Client<Self::Chain>,
		signer: &Self::AccountKeyPair,
		signer_nonce: <Self::Chain as frame_system::Trait>::Index,
		call: <Self::Chain as frame_system::Trait>::Call,
	) -> Self::SignedTransaction;
}
