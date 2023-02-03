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

#![cfg_attr(not(feature = "std"), no_std)]

use bp_messages::*;
pub use bp_polkadot_core::{
	AccountId, AccountInfoStorageMapKeyProvider, AccountPublic, Balance, BlockNumber, Hash, Hasher,
	Hashing, Header, Index, Nonce, Perbill, PolkadotSignedExtension, Signature, SignedBlock,
	UncheckedExtrinsic, EXTRA_STORAGE_PROOF_SIZE, TX_EXTRA_BYTES,
};
use frame_support::{
	dispatch::DispatchClass,
	parameter_types,
	sp_runtime::{MultiAddress, MultiSigner},
	weights::constants,
};
use frame_system::limits;
use sp_std::time::Duration;

/// Average block interval in Cumulus-based parachains.
///
/// Corresponds to the `MILLISECS_PER_BLOCK` from `parachains_common` crate.
pub const AVERAGE_BLOCK_INTERVAL: Duration = Duration::from_secs(12);

/// All cumulus bridge hubs allow normal extrinsics to fill block up to 75 percent.
///
/// This is a copy-paste from the cumulus repo's `parachains-common` crate.
pub const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

/// All cumulus bridge hubs chains allow for 0.5 seconds of compute with a 6-second average block
/// time.
///
/// This is a copy-paste from the cumulus repo's `parachains-common` crate.
// TODO: https://github.com/paritytech/parity-bridges-common/issues/1543 - remove `set_proof_size`
const MAXIMUM_BLOCK_WEIGHT: Weight = Weight::from_ref_time(constants::WEIGHT_REF_TIME_PER_SECOND)
	.saturating_div(2)
	.set_proof_size(polkadot_primitives::v2::MAX_POV_SIZE as u64);

/// All cumulus bridge hubs assume that about 5 percent of the block weight is consumed by
/// `on_initialize` handlers. This is used to limit the maximal weight of a single extrinsic.
///
/// This is a copy-paste from the cumulus repo's `parachains-common` crate.
pub const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(5);

parameter_types! {
	pub BlockLength: limits::BlockLength = limits::BlockLength::max_with_normal_ratio(
		5 * 1024 * 1024,
		NORMAL_DISPATCH_RATIO,
	);

	pub const BlockExecutionWeight: Weight = Weight::from_ref_time(constants::WEIGHT_REF_TIME_PER_NANOS).saturating_mul(5_000_000);

	pub const ExtrinsicBaseWeight: Weight = Weight::from_ref_time(constants::WEIGHT_REF_TIME_PER_NANOS).saturating_mul(125_000);

	pub BlockWeights: limits::BlockWeights = limits::BlockWeights::builder()
		.base_block(BlockExecutionWeight::get())
		.for_class(DispatchClass::all(), |weights| {
			weights.base_extrinsic = ExtrinsicBaseWeight::get();
		})
		.for_class(DispatchClass::Normal, |weights| {
			weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
		})
		.for_class(DispatchClass::Operational, |weights| {
			weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
			// Operational transactions have an extra reserved space, so that they
			// are included even if block reached `MAXIMUM_BLOCK_WEIGHT`.
			weights.reserved = Some(
				MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT,
			);
		})
		.avg_block_initialization(AVERAGE_ON_INITIALIZE_RATIO)
		.build_or_panic();
}

/// Public key of the chain account that may be used to verify signatures.
pub type AccountSigner = MultiSigner;

/// The address format for describing accounts.
pub type Address = MultiAddress<AccountId, ()>;

pub use bp_polkadot_core::BridgeSignedExtension as SignedExtension;

// Note about selecting values of two following constants:
//
// Normal transactions have limit of 75% of 1/2 second weight for Cumulus parachains. Let's keep
// some reserve for the rest of stuff there => let's select values that fit in 50% of maximal limit.
//
// Using current constants, the limit would be:
//
// `75% * WEIGHT_REF_TIME_PER_SECOND * 1 / 2 * 50% = 0.75 * 1_000_000_000_000 / 2 * 0.5 =
// 187_500_000_000`
//
// According to (preliminary) weights of messages pallet, cost of additional message is zero and the
// cost of additional relayer is `8_000_000 + db read + db write`. Let's say we want no more than
// 4096 unconfirmed messages (no any scientific justification for that - it just looks large
// enough). And then we can't have more than 4096 relayers. E.g. for 1024 relayers is (using
// `RocksDbWeight`):
//
// `1024 * (8_000_000 + db read + db write) = 1024 * (8_000_000 + 25_000_000 + 100_000_000) =
// 136_192_000_000`
//
// So 1024 looks like good approximation for the number of relayers. If something is wrong in those
// assumptions, or something will change, it shall be caught by the
// `ensure_able_to_receive_confirmation` test.

/// Maximal number of unrewarded relayer entries at inbound lane for Cumulus-based parachains.
pub const MAX_UNREWARDED_RELAYERS_IN_CONFIRMATION_TX: MessageNonce = 1024;

/// Maximal number of unconfirmed messages at inbound lane for Cumulus-based parachains.
pub const MAX_UNCONFIRMED_MESSAGES_IN_CONFIRMATION_TX: MessageNonce = 4096;
