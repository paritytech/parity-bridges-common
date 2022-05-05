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
	AccountId, AllPalletsWithSystem, Balances, BridgeRialtoMessages, Call, Event, Origin, Runtime,
	XcmPallet,
};
use bp_messages::source_chain::MessagesBridge;
use bp_millau::{Balance, WeightToFee};
use bridge_runtime_common::messages::source::FromThisChainMessagePayload;
use codec::Encode;
use frame_support::{
	parameter_types,
	traits::{Everything, Nothing},
	weights::Weight,
};
use sp_runtime::traits::Zero;
use sp_std::{marker::PhantomData, prelude::*};
use xcm::latest::prelude::*;
use xcm_builder::{
	AccountId32Aliases, AllowKnownQueryResponses, AllowTopLevelPaidExecutionFrom,
	CurrencyAdapter as XcmCurrencyAdapter, FixedWeightBounds, IsConcrete,
	SignedAccountId32AsNative, SignedToAccountId32, SovereignSignedViaLocation, TakeWeightCredit,
	UsingComponents,
};

parameter_types! {
	/// The location of the MLAU token, from the context of this chain. Since this token is native to this
	/// chain, we make it synonymous with it and thus it is the `Here` location, which means "equivalent to
	/// the context".
	pub const TokenLocation: MultiLocation = Here.into_location();
	/// The Millau network ID, associated with Kusama.
	pub const ThisNetwork: NetworkId = Kusama;
	/// The Rialto network ID, associated with Polkadot.
	pub const RialtoNetwork: NetworkId = Polkadot;

	/// Our XCM location ancestry - i.e. our location within the Consensus Universe.
	///
	/// Since Kusama is a top-level relay-chain with its own consensus, it's just our network ID.
	pub UniversalLocation: InteriorMultiLocation = ThisNetwork::get().into();
	/// The check account, which holds any native assets that have been teleported out and not back in (yet).
	pub CheckAccount: AccountId = XcmPallet::check_account();

	/// Available bridges.
	pub BridgeTable: Vec<(NetworkId, MultiLocation, Option<MultiAsset>)>
		= vec![(RialtoNetwork::get(), MultiLocation::parent(), None)];

	/// TODO
	pub ToRialtoPrice: MultiAssets = MultiAssets::new();
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
	SovereignSignedViaLocation<SovereignAccountOf, Origin>,
	// The AccountId32 location type can be expressed natively as a `Signed` origin.
	SignedAccountId32AsNative<ThisNetwork, Origin>,
);

parameter_types! {
	/// The amount of weight an XCM operation takes. This is a safe overestimate.
	pub const BaseXcmWeight: Weight = 1_000_000_000;
	/// Maximum number of instructions in a single XCM fragment. A sanity check against weight
	/// calculations getting too crazy.
	pub const MaxInstructions: u32 = 100;
}

/// The XCM router. When we want to send an XCM message, we use this type. It amalgamates all of our
/// individual routers.
pub type XcmRouter = (
	// Only one router so far - use DMP to communicate with Rialto.
	xcm_builder::SovereignPaidRemoteExporter<
		xcm_builder::NetworkExportTable<BridgeTable>,
		ToRialtoBridge<BridgeRialtoMessages>,
		UniversalLocation,
	>,
);

parameter_types! {
	pub const MaxAssetsIntoHolding: u32 = 64;
}

/// The barriers one of which must be passed for an XCM message to be executed.
pub type Barrier = (
	// Weight that is paid for may be consumed.
	TakeWeightCredit,
	// If the message is one that immediately attemps to pay for execution, then allow it.
	AllowTopLevelPaidExecutionFrom<Everything>,
	// Expected responses are OK.
	AllowKnownQueryResponses<XcmPallet>,
);

pub struct XcmConfig;
impl xcm_executor::Config for XcmConfig {
	type Call = Call;
	type XcmSender = XcmRouter;
	type AssetTransactor = LocalAssetTransactor;
	type OriginConverter = LocalOriginConverter;
	type IsReserve = ();
	type IsTeleporter = ();
	type UniversalLocation = UniversalLocation;
	type Barrier = Barrier;
	type Weigher = xcm_builder::FixedWeightBounds<BaseXcmWeight, Call, MaxInstructions>;
	// The weight trader piggybacks on the existing transaction-fee conversion logic.
	type Trader = UsingComponents<WeightToFee, TokenLocation, AccountId, Balances, ()>;
	type ResponseHandler = XcmPallet;
	type AssetTrap = XcmPallet;
	type AssetLocker = ();
	type AssetExchanger = ();
	type AssetClaims = XcmPallet;
	type SubscriptionService = XcmPallet;
	type PalletInstancesInfo = AllPalletsWithSystem;
	type MaxAssetsIntoHolding = MaxAssetsIntoHolding;
	type FeeManager = ();
	type MessageExporter = bridge_runtime_common::messages::HaulBlobExporter<
		ToRialtoBridge<BridgeRialtoMessages>,
		RialtoNetwork,
		ToRialtoPrice,
	>;
	type UniversalAliases = Nothing;
}

/// Type to convert an `Origin` type value into a `MultiLocation` value which represents an interior
/// location of this chain.
pub type LocalOriginToLocation = (
	// Usual Signed origin to be used in XCM as a corresponding AccountId32
	SignedToAccountId32<Origin, AccountId, ThisNetwork>,
);

impl pallet_xcm::Config for Runtime {
	type Event = Event;
	// We don't allow any messages to be sent via the transaction yet. This is basically safe to
	// enable, (safe the possibility of someone spamming the parachain if they're willing to pay
	// the DOT to send from the Relay-chain). But it's useless until we bring in XCM v3 which will
	// make `DescendOrigin` a bit more useful.
	type SendXcmOrigin = xcm_builder::EnsureXcmOrigin<Origin, LocalOriginToLocation>;
	type XcmRouter = XcmRouter;
	// Anyone can execute XCM messages locally.
	type ExecuteXcmOrigin = xcm_builder::EnsureXcmOrigin<Origin, LocalOriginToLocation>;
	type XcmExecuteFilter = Everything;
	type XcmExecutor = xcm_executor::XcmExecutor<XcmConfig>;
	// Anyone is able to use teleportation regardless of who they are and what they want to
	// teleport.
	type XcmTeleportFilter = Everything;
	// Anyone is able to use reserve transfers regardless of who they are and what they want to
	// transfer.
	type XcmReserveTransferFilter = Everything;
	type Weigher = FixedWeightBounds<BaseXcmWeight, Call, MaxInstructions>;
	type UniversalLocation = UniversalLocation;
	type Origin = Origin;
	type Call = Call;
	const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
	type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
	type Currency = Balances;
	type CurrencyMatcher = ();
	type TrustedLockers = ();
	type SovereignAccountOf = SovereignAccountOf;
	type MaxLockers = frame_support::traits::ConstU32<8>;
}

/// With-rialto bridge.
pub struct ToRialtoBridge<MB>(PhantomData<MB>);

impl<MB: MessagesBridge<Origin, AccountId, Balance, FromThisChainMessagePayload>> SendXcm
	for ToRialtoBridge<MB>
{
	type Ticket = (MultiLocation, Xcm<()>);

	fn validate(
		dest: &mut Option<MultiLocation>,
		msg: &mut Option<Xcm<()>>,
	) -> SendResult<(MultiLocation, Xcm<()>)> {
		let dest: InteriorMultiLocation = RialtoNetwork::get().into();
		let here = UniversalLocation::get();
		let route = dest.relative_to(&here);
		let pair = (route, msg.take().unwrap());
		Ok((pair, MultiAssets::new()))
	}

	fn deliver(pair: (MultiLocation, Xcm<()>)) -> Result<XcmHash, SendError> {
		let result = MB::send_message(
			pallet_xcm::Origin::from(MultiLocation::from(UniversalLocation::get())).into(),
			[0, 0, 0, 0],
			pair.encode(),
			Zero::zero(),
		);
		log::info!(target: "runtime::bridge", "Trying to send XCM message (SendXcm) to Millau: {:?}", result);
		result
			.map(|_artifacts| XcmHash::default()) // TODO: what's hash here? (lane, nonce).encode().hash() or something else
			.map_err(|_e| SendError::Transport("Bridge has rejected the message"))
	}
}

impl<MB: MessagesBridge<Origin, AccountId, Balance, FromThisChainMessagePayload>> bridge_runtime_common::messages::HaulBlob for ToRialtoBridge<MB> {
	fn haul_blob(blob: Vec<u8>) {
		let result = MB::send_message(
			pallet_xcm::Origin::from(MultiLocation::from(UniversalLocation::get())).into(),
			[0, 0, 0, 0],
			blob,
			Zero::zero(),
		);
		log::info!(target: "runtime::bridge", "Trying to send XCM message (HaulBlob) to Millau: {:?}", result);
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn new_test_ext() -> sp_io::TestExternalities {
		sp_io::TestExternalities::new(frame_system::GenesisConfig::default().build_storage::<Runtime>().unwrap())
	}

	#[test]
	fn messages_to_rialto_are_sent() {
		let outcome = new_test_ext().execute_with(|| {
			let dest = (Parent, X1(GlobalConsensus(RialtoNetwork::get())));
			let xcm: Xcm<()> = vec![Instruction::Trap(42)].into();

			let (ticket, price) = validate_send::<XcmRouter>(dest.into(), xcm)?;
			println!("=== ticket: {:?}", ticket);
			println!("=== price: {:?}", price);

			XcmRouter::deliver(ticket)
		});

		println!("=== {:?}", outcome);
	}

	#[test]
	fn messages_from_rialto_are_dispatched() {
		type XcmExecutor = xcm_executor::XcmExecutor<XcmConfig>;

		let _ = env_logger::try_init();

		let outcome = new_test_ext().execute_with(|| {
			let location = (Parent, X1(GlobalConsensus(RialtoNetwork::get())));
			// simple Trap(42) is converted to this XCM by SovereignPaidRemoteExporter
			let xcm: Xcm<Call> = vec![
				ExportMessage {
					network: RialtoNetwork::get(),
					destination: Here,
					xcm: vec![
						Instruction::UniversalOrigin(GlobalConsensus(RialtoNetwork::get())),
						Instruction::Trap(42)
					].into(),
				},
			].into();

			let hash = xcm.using_encoded(sp_io::hashing::blake2_256);
			let max_weight = 1000000000;
			let weight_credit = 1000000000;

			XcmExecutor::execute_xcm_in_credit(location, xcm, hash, max_weight, weight_credit)
		});

		println!("=== {:?}", outcome);
	}
}
