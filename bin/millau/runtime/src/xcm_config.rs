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

//! XCM configurations for the Millau runtime.

use super::{
	rialto_messages::{ToRialtoBlobExporter, WithRialtoMessageBridge, XCM_LANE},
	rialto_parachain_messages::{
		ToRialtoParachainBlobExporter, WithRialtoParachainMessageBridge,
		XCM_LANE as XCM_LANE_PARACHAIN,
	},
	AccountId, AllPalletsWithSystem, Balances, Runtime, RuntimeCall, RuntimeEvent, RuntimeOrigin,
	WithRialtoMessagesInstance, WithRialtoParachainMessagesInstance, XcmPallet,
};
use bp_messages::LaneId;
use bp_millau::WeightToFee;
use bp_rialto_parachain::RIALTO_PARACHAIN_ID;
use bridge_runtime_common::CustomNetworkId;
use frame_support::{
	parameter_types,
	traits::{ConstU32, Everything, Nothing},
	weights::Weight,
};
use xcm::latest::prelude::*;
use xcm_builder::{
	AccountId32Aliases, AllowKnownQueryResponses, AllowTopLevelPaidExecutionFrom,
	CurrencyAdapter as XcmCurrencyAdapter, IsConcrete, MintLocation, SignedAccountId32AsNative,
	SignedToAccountId32, SovereignSignedViaLocation, TakeWeightCredit, UsingComponents,
};
use xcm_executor::traits::ExportXcm;

parameter_types! {
	/// The location of the `MLAU` token, from the context of this chain. Since this token is native to this
	/// chain, we make it synonymous with it and thus it is the `Here` location, which means "equivalent to
	/// the context".
	pub const TokenLocation: MultiLocation = Here.into_location();
	/// The Millau network ID.
	pub const ThisNetwork: NetworkId = CustomNetworkId::Millau.as_network_id();
	/// The Rialto network ID.
	pub const RialtoNetwork: NetworkId = CustomNetworkId::Rialto.as_network_id();
	/// The RialtoParachain network ID.
	pub const RialtoParachainNetwork: NetworkId = CustomNetworkId::RialtoParachain.as_network_id();

	/// Our XCM location ancestry - i.e. our location within the Consensus Universe.
	///
	/// Since Kusama is a top-level relay-chain with its own consensus, it's just our network ID.
	pub UniversalLocation: InteriorMultiLocation = ThisNetwork::get().into();
	/// The check account, which holds any native assets that have been teleported out and not back in (yet).
	pub CheckAccount: (AccountId, MintLocation) = (XcmPallet::check_account(), MintLocation::Local);
}

/// The canonical means of converting a `MultiLocation` into an `AccountId`, used when we want to
/// determine the sovereign account controlled by a location.
pub type SovereignAccountOf = (
	// We can directly alias an `AccountId32` into a local account.
	AccountId32Aliases<ThisNetwork, AccountId>,
);

/// Our asset transactor. This is what allows us to interest with the runtime facilities from the
/// point of view of XCM-only concepts like `MultiLocation` and `MultiAsset`.
///
/// Ours is only aware of the Balances pallet, which is mapped to `TokenLocation`.
pub type LocalAssetTransactor = XcmCurrencyAdapter<
	// Use this currency:
	Balances,
	// Use this currency when it is a fungible asset matching the given location or name:
	IsConcrete<TokenLocation>,
	// We can convert the MultiLocations with our converter above:
	SovereignAccountOf,
	// Our chain's account ID type (we can't get away without mentioning it explicitly):
	AccountId,
	// We track our teleports in/out to keep total issuance correct.
	CheckAccount,
>;

/// The means that we convert the XCM message origin location into a local dispatch origin.
type LocalOriginConverter = (
	// A `Signed` origin of the sovereign account that the original location controls.
	SovereignSignedViaLocation<SovereignAccountOf, RuntimeOrigin>,
	// The AccountId32 location type can be expressed natively as a `Signed` origin.
	SignedAccountId32AsNative<ThisNetwork, RuntimeOrigin>,
);

parameter_types! {
	/// The amount of weight an XCM operation takes. This is a safe overestimate.
	pub const BaseXcmWeight: Weight = Weight::from_parts(1_000_000_000, 64 * 1024);
	/// Maximum number of instructions in a single XCM fragment. A sanity check against weight
	/// calculations getting too crazy.
	pub const MaxInstructions: u32 = 100;
}

/// The XCM router. We are not sending messages to sibling/parent/child chains here.
pub type XcmRouter = ();

/// The barriers one of which must be passed for an XCM message to be executed.
pub type Barrier = (
	// Weight that is paid for may be consumed.
	TakeWeightCredit,
	// If the message is one that immediately attemps to pay for execution, then allow it.
	AllowTopLevelPaidExecutionFrom<Everything>,
	// Expected responses are OK.
	AllowKnownQueryResponses<XcmPallet>,
);

/// Dispatches received XCM messages from other chain.
pub type OnMillauBlobDispatcher = xcm_builder::BridgeBlobDispatcher<
	crate::xcm_config::XcmRouter,
	crate::xcm_config::UniversalLocation,
>;

/// XCM weigher type.
pub type XcmWeigher = xcm_builder::FixedWeightBounds<BaseXcmWeight, RuntimeCall, MaxInstructions>;

pub struct XcmConfig;
impl xcm_executor::Config for XcmConfig {
	type RuntimeCall = RuntimeCall;
	type XcmSender = ();
	type AssetTransactor = LocalAssetTransactor;
	type OriginConverter = LocalOriginConverter;
	type IsReserve = ();
	type IsTeleporter = ();
	type UniversalLocation = UniversalLocation;
	type Barrier = Barrier;
	type Weigher = XcmWeigher;
	// The weight trader piggybacks on the existing transaction-fee conversion logic.
	type Trader = UsingComponents<WeightToFee, TokenLocation, AccountId, Balances, ()>;
	type ResponseHandler = XcmPallet;
	type AssetTrap = XcmPallet;
	type AssetLocker = ();
	type AssetExchanger = ();
	type AssetClaims = XcmPallet;
	type SubscriptionService = XcmPallet;
	type PalletInstancesInfo = AllPalletsWithSystem;
	type MaxAssetsIntoHolding = ConstU32<64>;
	type FeeManager = ();
	type MessageExporter = ToRialtoOrRialtoParachainSwitchExporter;
	type UniversalAliases = Nothing;
	type CallDispatcher = RuntimeCall;
	type SafeCallFilter = Everything;
}

/// Type to convert an `Origin` type value into a `MultiLocation` value which represents an interior
/// location of this chain.
pub type LocalOriginToLocation = (
	// Usual Signed origin to be used in XCM as a corresponding AccountId32
	SignedToAccountId32<RuntimeOrigin, AccountId, ThisNetwork>,
);

#[cfg(feature = "runtime-benchmarks")]
parameter_types! {
	pub ReachableDest: Option<MultiLocation> = todo!("We dont use benchmarks for pallet_xcm, so if you hit this message, you need to remove this and define value instead");
}

impl pallet_xcm::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	// We don't allow any messages to be sent via the transaction yet. This is basically safe to
	// enable, (safe the possibility of someone spamming the parachain if they're willing to pay
	// the DOT to send from the Relay-chain). But it's useless until we bring in XCM v3 which will
	// make `DescendOrigin` a bit more useful.
	type SendXcmOrigin = xcm_builder::EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
	type XcmRouter = ();
	// Anyone can execute XCM messages locally.
	type ExecuteXcmOrigin = xcm_builder::EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
	type XcmExecuteFilter = Everything;
	type XcmExecutor = xcm_executor::XcmExecutor<XcmConfig>;
	// Anyone is able to use teleportation regardless of who they are and what they want to
	// teleport.
	type XcmTeleportFilter = Everything;
	// Anyone is able to use reserve transfers regardless of who they are and what they want to
	// transfer.
	type XcmReserveTransferFilter = Everything;
	type Weigher = XcmWeigher;
	type UniversalLocation = UniversalLocation;
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
	type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
	type Currency = Balances;
	type CurrencyMatcher = ();
	type TrustedLockers = ();
	type SovereignAccountOf = SovereignAccountOf;
	type MaxLockers = frame_support::traits::ConstU32<8>;
	type WeightInfo = pallet_xcm::TestWeightInfo;
	#[cfg(feature = "runtime-benchmarks")]
	type ReachableDest = ReachableDest;
}

pub struct ToRialtoOrRialtoParachainSwitchExporter;

impl ExportXcm for ToRialtoOrRialtoParachainSwitchExporter {
	type Ticket = (NetworkId, (sp_std::prelude::Vec<u8>, XcmHash));

	fn validate(
		network: NetworkId,
		channel: u32,
		universal_source: &mut Option<InteriorMultiLocation>,
		destination: &mut Option<InteriorMultiLocation>,
		message: &mut Option<Xcm<()>>,
	) -> SendResult<Self::Ticket> {
		if network == RialtoNetwork::get() {
			ToRialtoBlobExporter::validate(network, channel, universal_source, destination, message)
				.map(|result| ((RialtoNetwork::get(), result.0), result.1))
		} else if network == RialtoParachainNetwork::get() {
			ToRialtoParachainBlobExporter::validate(
				network,
				channel,
				universal_source,
				destination,
				message,
			)
			.map(|result| ((RialtoParachainNetwork::get(), result.0), result.1))
		} else {
			Err(SendError::Unroutable)
		}
	}

	fn deliver(ticket: Self::Ticket) -> Result<XcmHash, SendError> {
		let (network, ticket) = ticket;
		if network == RialtoNetwork::get() {
			ToRialtoBlobExporter::deliver(ticket)
		} else if network == RialtoParachainNetwork::get() {
			ToRialtoParachainBlobExporter::deliver(ticket)
		} else {
			Err(SendError::Unroutable)
		}
	}
}

/*
#[cfg(test)]
mod tests {
	use super::*;
	use crate::rialto_messages::WeightCredit;
	use bp_messages::{
		target_chain::{DispatchMessage, DispatchMessageData, MessageDispatch},
		MessageKey,
	};
	use bp_runtime::messages::MessageDispatchResult;
	use bridge_runtime_common::messages::target::FromBridgedChainMessageDispatch;
	use codec::Encode;

	fn new_test_ext() -> sp_io::TestExternalities {
		sp_io::TestExternalities::new(
			frame_system::GenesisConfig::default().build_storage::<Runtime>().unwrap(),
		)
	}

	#[test]
	fn xcm_messages_are_sent_using_bridge_router() {
		new_test_ext().execute_with(|| {
			let xcm: Xcm<()> = vec![Instruction::Trap(42)].into();
			let expected_fee = MultiAssets::from((Here, 1_000_000_u128));
			let expected_hash =
				([0u8, 0u8, 0u8, 0u8], 1u64).using_encoded(sp_io::hashing::blake2_256);

			// message 1 to Rialto
			let dest = (Parent, X1(GlobalConsensus(RialtoNetwork::get())));
			let send_result = send_xcm::<XcmRouter>(dest.into(), xcm.clone());
			assert_eq!(send_result, Ok((expected_hash, expected_fee.clone())));

			// message 2 to RialtoParachain (expected hash is the same, since other lane is used)
			let dest =
				(Parent, X2(GlobalConsensus(RialtoNetwork::get()), Parachain(RIALTO_PARACHAIN_ID)));
			let send_result = send_xcm::<XcmRouter>(dest.into(), xcm);
			assert_eq!(send_result, Ok((expected_hash, expected_fee)));
		})
	}

	#[test]
	fn xcm_messages_from_rialto_are_dispatched() {
		type XcmExecutor = xcm_executor::XcmExecutor<XcmConfig>;
		type MessageDispatcher = FromBridgedChainMessageDispatch<
			WithRialtoMessageBridge,
			XcmExecutor,
			XcmWeigher,
			WeightCredit,
		>;

		new_test_ext().execute_with(|| {
			let location: MultiLocation =
				(Parent, X1(GlobalConsensus(RialtoNetwork::get()))).into();
			let xcm: Xcm<RuntimeCall> = vec![Instruction::Trap(42)].into();

			let mut incoming_message = DispatchMessage {
				key: MessageKey { lane_id: LaneId([0, 0, 0, 0]), nonce: 1 },
				data: DispatchMessageData { payload: Ok((location, xcm).into()) },
			};

			let dispatch_weight = MessageDispatcher::dispatch_weight(&mut incoming_message);
			assert_eq!(dispatch_weight, BaseXcmWeight::get());

			let dispatch_result =
				MessageDispatcher::dispatch(&AccountId::from([0u8; 32]), incoming_message);
			assert_eq!(
				dispatch_result,
				MessageDispatchResult {
					unspent_weight: frame_support::weights::Weight::zero(),
					dispatch_level_result: (),
				}
			);
		})
	}
}
*/
