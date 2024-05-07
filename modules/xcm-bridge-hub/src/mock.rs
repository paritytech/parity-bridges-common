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

#![cfg(test)]

use crate as pallet_xcm_bridge_hub;

use bp_messages::{
	target_chain::{DispatchMessage, MessageDispatch},
	ChainWithMessages, LaneId, MessageNonce,
};
use bp_runtime::{messages::MessageDispatchResult, Chain, ChainId};
use bp_xcm_bridge_hub::{BridgeId, LocalXcmChannelManager};
use codec::Encode;
use frame_support::{
	derive_impl, parameter_types,
	traits::{EnsureOrigin, OriginTrait},
	weights::RuntimeDbWeight,
};
use polkadot_parachain_primitives::primitives::Sibling;
use sp_core::H256;
use sp_runtime::{
	testing::Header as SubstrateHeader,
	traits::{BlakeTwo256, IdentityLookup},
	AccountId32, BuildStorage, StateVersion,
};
use xcm::prelude::*;
use xcm_builder::{DispatchBlob, DispatchBlobError, ParentIsPreset, SiblingParachainConvertsVia};

pub type AccountId = AccountId32;
pub type Balance = u64;

type Block = frame_system::mocking::MockBlock<TestRuntime>;

pub const SIBLING_ASSET_HUB_ID: u32 = 2001;
pub const THIS_BRIDGE_HUB_ID: u32 = 2002;
pub const BRIDGED_ASSET_HUB_ID: u32 = 1001;

frame_support::construct_runtime! {
	pub enum TestRuntime {
		System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Event<T>},
		Messages: pallet_bridge_messages::{Pallet, Call, Event<T>},
		XcmOverBridge: pallet_xcm_bridge_hub::{Pallet, Call, Event<T>},
	}
}

parameter_types! {
	pub const DbWeight: RuntimeDbWeight = RuntimeDbWeight { read: 1, write: 2 };
	pub const ExistentialDeposit: Balance = 1;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for TestRuntime {
	type AccountId = AccountId;
	type AccountData = pallet_balances::AccountData<Balance>;
	type Block = Block;
	type Lookup = IdentityLookup<Self::AccountId>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig as pallet_balances::DefaultConfig)]
impl pallet_balances::Config for TestRuntime {
	type AccountStore = System;
}

impl pallet_bridge_messages::Config for TestRuntime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = TestMessagesWeights;

	type ThisChain = ThisChain;
	type BridgedChain = BridgedChain;
	type BridgedHeaderChain = ();

	type OutboundPayload = Vec<u8>;
	type InboundPayload = Vec<u8>;
	type DeliveryPayments = ();
	type DeliveryConfirmationPayments = ();
	type MessageDispatch = TestMessageDispatch;
	type OnMessagesDelivered = ();
}

pub struct TestMessagesWeights;

impl pallet_bridge_messages::WeightInfo for TestMessagesWeights {
	fn receive_single_message_proof() -> Weight {
		Weight::zero()
	}
	fn receive_n_messages_proof(_n: u32) -> Weight {
		Weight::zero()
	}
	fn receive_single_message_proof_with_outbound_lane_state() -> Weight {
		Weight::zero()
	}
	fn receive_single_n_bytes_message_proof(_n: u32) -> Weight {
		Weight::zero()
	}
	fn receive_delivery_proof_for_single_message() -> Weight {
		Weight::zero()
	}
	fn receive_delivery_proof_for_two_messages_by_single_relayer() -> Weight {
		Weight::zero()
	}
	fn receive_delivery_proof_for_two_messages_by_two_relayers() -> Weight {
		Weight::zero()
	}
	fn receive_single_n_bytes_message_proof_with_dispatch(_n: u32) -> Weight {
		Weight::from_parts(1, 0)
	}
}

impl pallet_bridge_messages::WeightInfoExt for TestMessagesWeights {
	fn expected_extra_storage_proof_size() -> u32 {
		0
	}

	fn receive_messages_proof_overhead_from_runtime() -> Weight {
		Weight::zero()
	}

	fn receive_messages_delivery_proof_overhead_from_runtime() -> Weight {
		Weight::zero()
	}
}

parameter_types! {
	pub const RelayNetwork: NetworkId = NetworkId::Kusama;
	pub const BridgedRelayNetwork: NetworkId = NetworkId::Polkadot;
	pub BridgedRelayNetworkLocation: Location = (Parent, GlobalConsensus(BridgedRelayNetwork::get())).into();
	pub const NonBridgedRelayNetwork: NetworkId = NetworkId::Rococo;
	pub const BridgeReserve: Balance = 100_000;
	pub UniversalLocation: InteriorLocation = [
		GlobalConsensus(RelayNetwork::get()),
		Parachain(THIS_BRIDGE_HUB_ID),
	].into();
	pub const Penalty: Balance = 1_000;
}

/// Type for specifying how a `MultiLocation` can be converted into an `AccountId`. This is used
/// when determining ownership of accounts for asset transacting and when attempting to use XCM
/// `Transact` in order to determine the dispatch Origin.
pub type LocationToAccountId = (
	// The parent (Relay-chain) origin converts to the parent `AccountId`.
	ParentIsPreset<AccountId>,
	// Sibling parachain origins convert to AccountId via the `ParaId::into`.
	SiblingParachainConvertsVia<Sibling, AccountId>,
);

pub struct OpenBridgeOrigin;

impl OpenBridgeOrigin {
	pub fn parent_relay_chain_origin() -> RuntimeOrigin {
		RuntimeOrigin::signed([0u8; 32].into())
	}

	pub fn parent_relay_chain_universal_origin() -> RuntimeOrigin {
		RuntimeOrigin::signed([1u8; 32].into())
	}

	pub fn sibling_parachain_origin() -> RuntimeOrigin {
		let mut account = [0u8; 32];
		account[..4].copy_from_slice(&SIBLING_ASSET_HUB_ID.encode()[..4]);
		RuntimeOrigin::signed(account.into())
	}

	pub fn sibling_parachain_universal_origin() -> RuntimeOrigin {
		RuntimeOrigin::signed([2u8; 32].into())
	}

	pub fn origin_without_sovereign_account() -> RuntimeOrigin {
		RuntimeOrigin::signed([3u8; 32].into())
	}

	pub fn disallowed_origin() -> RuntimeOrigin {
		RuntimeOrigin::signed([42u8; 32].into())
	}
}

impl EnsureOrigin<RuntimeOrigin> for OpenBridgeOrigin {
	type Success = Location;

	fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
		let signer = o.clone().into_signer();
		if signer == Self::parent_relay_chain_origin().into_signer() {
			return Ok(Location { parents: 1, interior: Here })
		} else if signer == Self::parent_relay_chain_universal_origin().into_signer() {
			return Ok(Location {
				parents: 2,
				interior: [GlobalConsensus(RelayNetwork::get())].into(),
			})
		} else if signer == Self::sibling_parachain_universal_origin().into_signer() {
			return Ok(Location {
				parents: 2,
				interior: [GlobalConsensus(RelayNetwork::get()), Parachain(SIBLING_ASSET_HUB_ID)]
					.into(),
			})
		} else if signer == Self::origin_without_sovereign_account().into_signer() {
			return Ok(Location {
				parents: 1,
				interior: [Parachain(SIBLING_ASSET_HUB_ID), OnlyChild].into(),
			})
		}

		let mut sibling_account = [0u8; 32];
		sibling_account[..4].copy_from_slice(&SIBLING_ASSET_HUB_ID.encode()[..4]);
		if signer == Some(sibling_account.into()) {
			return Ok(Location { parents: 1, interior: [Parachain(SIBLING_ASSET_HUB_ID)].into() })
		}

		Err(o)
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
		Ok(Self::parent_relay_chain_origin())
	}
}

impl pallet_xcm_bridge_hub::Config for TestRuntime {
	type RuntimeEvent = RuntimeEvent;

	type UniversalLocation = UniversalLocation;
	type BridgedNetwork = BridgedRelayNetworkLocation;
	type BridgeMessagesPalletInstance = ();

	type OpenBridgeOrigin = OpenBridgeOrigin;
	type BridgeOriginAccountIdConverter = LocationToAccountId;

	type BridgeReserve = BridgeReserve;
	type NativeCurrency = Balances;

	type LocalXcmChannelManager = TestLocalXcmChannelManager;
	type BlobDispatcher = TestBlobDispatcher;
	type MessageExportPrice = ();
	type DestinationVersion = AlwaysLatest;
}

pub struct TestLocalXcmChannelManager;

impl TestLocalXcmChannelManager {
	pub fn make_congested() {
		frame_support::storage::unhashed::put(b"TestLocalXcmChannelManager.Congested", &true);
	}

	pub fn is_bridge_suspened() -> bool {
		frame_support::storage::unhashed::get_or_default(b"TestLocalXcmChannelManager.Suspended")
	}

	pub fn is_bridge_resumed() -> bool {
		frame_support::storage::unhashed::get_or_default(b"TestLocalXcmChannelManager.Resumed")
	}
}

impl LocalXcmChannelManager for TestLocalXcmChannelManager {
	type Error = ();

	fn is_congested(_with: &Location) -> bool {
		frame_support::storage::unhashed::get_or_default(b"TestLocalXcmChannelManager.Congested")
	}

	fn suspend_bridge(_local_origin: &Location, _bridge: BridgeId) -> Result<(), Self::Error> {
		frame_support::storage::unhashed::put(b"TestLocalXcmChannelManager.Suspended", &true);
		Ok(())
	}

	fn resume_bridge(_local_origin: &Location, _bridge: BridgeId) -> Result<(), Self::Error> {
		frame_support::storage::unhashed::put(b"TestLocalXcmChannelManager.Resumed", &true);
		Ok(())
	}
}

pub struct TestBlobDispatcher;

impl TestBlobDispatcher {
	pub fn is_dispatched() -> bool {
		frame_support::storage::unhashed::get_or_default(b"TestBlobDispatcher.Dispatched")
	}
}

impl DispatchBlob for TestBlobDispatcher {
	fn dispatch_blob(_blob: Vec<u8>) -> Result<(), DispatchBlobError> {
		frame_support::storage::unhashed::put(b"TestBlobDispatcher.Dispatched", &true);
		Ok(())
	}
}

pub struct ThisChain;

impl Chain for ThisChain {
	const ID: ChainId = *b"ttch";

	type BlockNumber = u64;
	type Hash = H256;
	type Hasher = BlakeTwo256;
	type Header = SubstrateHeader;
	type AccountId = AccountId;
	type Balance = Balance;
	type Nonce = u64;
	type Signature = sp_runtime::MultiSignature;
	const STATE_VERSION: StateVersion = StateVersion::V1;

	fn max_extrinsic_size() -> u32 {
		u32::MAX
	}

	fn max_extrinsic_weight() -> Weight {
		Weight::MAX
	}
}

impl ChainWithMessages for ThisChain {
	const WITH_CHAIN_MESSAGES_PALLET_NAME: &'static str = "WithThisChainBridgeMessages";
	const MAX_UNREWARDED_RELAYERS_IN_CONFIRMATION_TX: MessageNonce = 16;
	const MAX_UNCONFIRMED_MESSAGES_IN_CONFIRMATION_TX: MessageNonce = 128;
}

pub struct BridgedChain;

pub type BridgedHeaderHash = H256;
pub type BridgedChainHeader = SubstrateHeader;

impl Chain for BridgedChain {
	const ID: ChainId = *b"tbch";

	type BlockNumber = u64;
	type Hash = BridgedHeaderHash;
	type Hasher = BlakeTwo256;
	type Header = BridgedChainHeader;
	type AccountId = AccountId;
	type Balance = Balance;
	type Nonce = u64;
	type Signature = sp_runtime::MultiSignature;
	const STATE_VERSION: StateVersion = StateVersion::V1;

	fn max_extrinsic_size() -> u32 {
		4096
	}

	fn max_extrinsic_weight() -> Weight {
		Weight::MAX
	}
}

impl ChainWithMessages for BridgedChain {
	const WITH_CHAIN_MESSAGES_PALLET_NAME: &'static str = "WithBridgedChainBridgeMessages";
	const MAX_UNREWARDED_RELAYERS_IN_CONFIRMATION_TX: MessageNonce = 16;
	const MAX_UNCONFIRMED_MESSAGES_IN_CONFIRMATION_TX: MessageNonce = 128;
}

/// Test message dispatcher.
pub struct TestMessageDispatch;

impl TestMessageDispatch {
	pub fn deactivate(lane: LaneId) {
		frame_support::storage::unhashed::put(&(b"inactive", lane).encode()[..], &false);
	}
}

impl MessageDispatch for TestMessageDispatch {
	type DispatchPayload = Vec<u8>;
	type DispatchLevelResult = ();

	fn is_active(lane: LaneId) -> bool {
		frame_support::storage::unhashed::take::<bool>(&(b"inactive", lane).encode()[..]) !=
			Some(false)
	}

	fn dispatch_weight(_message: &mut DispatchMessage<Self::DispatchPayload>) -> Weight {
		Weight::zero()
	}

	fn dispatch(
		_: DispatchMessage<Self::DispatchPayload>,
	) -> MessageDispatchResult<Self::DispatchLevelResult> {
		MessageDispatchResult { unspent_weight: Weight::zero(), dispatch_level_result: () }
	}
}

/// Location of bridged asset hub.
pub fn bridged_asset_hub_location() -> InteriorLocation {
	[GlobalConsensus(BridgedRelayNetwork::get()), Parachain(BRIDGED_ASSET_HUB_ID)].into()
}

/// Run pallet test.
pub fn run_test<T>(test: impl FnOnce() -> T) -> T {
	sp_io::TestExternalities::new(
		frame_system::GenesisConfig::<TestRuntime>::default().build_storage().unwrap(),
	)
	.execute_with(test)
}
