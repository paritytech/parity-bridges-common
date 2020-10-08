// Copyright 2020 Parity Technologies (UK) Ltd.
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

//! Configuration parameters for the Millau Substrate chain.

use bp_rialto::Header;
use hex_literal::hex;
use pallet_substrate_bridge::AuthoritySet;
use sp_core::crypto::Public;
use sp_finality_grandpa::AuthorityId;
use sp_std::vec;

/// The first header known to the pallet.
///
/// Note that this does not need to be the genesis header of the Millau
/// chain since the pallet may start at any arbitrary header.
// To get this we first need to call the `chain_getBlockHash` RPC method, and then
// we can use the result from that and call the `chain_getBlock` RPC method to get
// the rest of the info.
//
// In this case we've grabbed the genesis block of the Millau Substrate chain.
pub fn initial_header() -> Header {
	Header {
		parent_hash: Default::default(),
		number: Default::default(),
		state_root: hex!("bb65e8ba99408ebfefea9d28f74403d41da6858fa075c51fcc71dc383455c530").into(),
		extrinsics_root: hex!("03170a2e7597b7b7e3d84c05391d139a62b157e78786d8c082f29dcf4c111314").into(),
		digest: Default::default(),
	}
}

/// The first set of Grandpa authorities known to the pallet.
///
/// Note that this doesn't have to be the "genesis" authority set, as the
/// pallet can be configured to start from any height.
pub fn initial_authority_set() -> AuthoritySet {
	let set_id = 0;
	let authorities = vec![(AuthorityId::from_slice(&[1; 32]), 1)];
	AuthoritySet::new(authorities, set_id)
}
