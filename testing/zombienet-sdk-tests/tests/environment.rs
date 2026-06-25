// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Shared environment for the Rococo <> Westend bridge zombienet-sdk tests.
//!
//! It:
//!   * spawns two relay-chain networks (Rococo and Westend), each with a Bridge Hub (para 1013 /
//!     1002) and an Asset Hub (para 1000) collator,
//!   * initializes the bridge (HRMP channels, remote XCM versions and bridged foreign assets),
//!   * drives the external `substrate-relay` binary as a set of subprocesses, and
//!   * exposes `subxt` query/extrinsic helpers used by the individual tests.

use anyhow::anyhow;
use codec::Decode;
use std::{future::Future, path::PathBuf, time::Duration};
use subxt::{config::DefaultExtrinsicParamsBuilder, tx::Payload, OnlineClient, PolkadotConfig};
use subxt_signer::sr25519::{dev, Keypair};
use tokio::{
	process::{Child, Command},
	time::{sleep, timeout_at, Instant},
};
use zombienet_sdk::{
	environment::get_spawn_fn, Arg, LocalFileSystem, Network, NetworkConfig, NetworkConfigBuilder,
};

// `1u64 << 60` — the amount the local chain specs endow every well-known account with. The
// finality/parachain relayers (`//Charlie` / `//Dave`) only submit free or mandatory headers, so
// their balance must stay exactly at this value throughout the tests.
pub const ENDOWMENT: u128 = 1u128 << 60;

/// Message lane shared by both Asset Hubs (`0x00000002`).
pub const LANE_ID: [u8; 4] = [0, 0, 0, 2];
/// Target XCM version configured for every remote location.
pub const XCM_VERSION: u32 = 5;

// Bridged chain ids (`*b"bhwd"` / `*b"bhro"`), used as reward keys on the Bridge Hubs.
pub const BRIDGED_CHAIN_ID_BHWD: [u8; 4] = *b"bhwd";
pub const BRIDGED_CHAIN_ID_BHRO: [u8; 4] = *b"bhro";

// `6408de77..` / `e143f238..` — genesis hashes used as `NetworkId::ByGenesis(..)`.
pub const ROCOCO_GENESIS_HASH: [u8; 32] = [
	100, 8, 222, 119, 55, 197, 156, 35, 136, 144, 83, 58, 242, 88, 150, 162, 194, 6, 8, 216, 179,
	128, 187, 1, 2, 154, 203, 57, 39, 129, 6, 62,
];
pub const WESTEND_GENESIS_HASH: [u8; 32] = [
	225, 67, 242, 56, 3, 172, 80, 232, 246, 248, 230, 38, 149, 209, 206, 158, 78, 29, 104, 170, 54,
	193, 205, 44, 253, 21, 52, 2, 19, 243, 66, 62,
];

// Para ids.
pub const ASSET_HUB_PARA_ID: u32 = 1000;
pub const BRIDGE_HUB_ROCOCO_PARA_ID: u32 = 1013;
pub const BRIDGE_HUB_WESTEND_PARA_ID: u32 = 1002;

// Sovereign / reward accounts funded on the Bridge Hubs.
const ASSET_HUB_SOVEREIGN_AT_BRIDGE_HUB: &str = "5Eg2fntNprdN3FgH4sfEaaZhYtddZQSQUqvYJ1f2mLtinVhV";
const BHR_LANE_THIS_CHAIN: &str = "5EHnXaT5GApse1euZWj9hycMbgjKBCNQL9WEwScL8QDx6mhK";
const BHR_LANE_BRIDGED_CHAIN: &str = "5EHnXaT5Tnt4A8aiP9CsuAFRhKPjKZJXRrj4a3mtihFvKpTi";
const BHW_LANE_THIS_CHAIN: &str = "5EHnXaT5GApry9tS6yd1FVusPq8o8bQJGCKyvXTFCoEKk5Z9";
const BHW_LANE_BRIDGED_CHAIN: &str = "5EHnXaT5Tnt3VGpEvc6jSgYwVToDGxLRMuYoZ8coo6GHyWbR";

// The amount funded onto each sovereign/reward account.
const SOVEREIGN_FUNDING: u128 = 100_000_000_000_000;

// ---------------------------------------------------------------------------------------------
// Per-runtime typed operations.
//
// Rococo/Westend (relays) and the two Asset Hubs / Bridge Hubs share the same type layout for the
// calls and storage we touch, but `subxt` generates a distinct module per runtime, so the types
// are nominally different. The macros below generate the (identical) bodies once per runtime.
// ---------------------------------------------------------------------------------------------

macro_rules! relay_ops {
	($name:ident, $relay:ident, $runtime:ident) => {
		pub mod $name {
			use super::sign_submit_wait;
			use crate::$relay::runtime_types::{
				pallet_xcm::pallet::Call as XcmPalletCall,
				polkadot_parachain_primitives::primitives::Id,
				polkadot_runtime_parachains::hrmp::pallet::Call as HrmpCall,
				sp_weights::weight_v2::Weight,
				staging_xcm::v4::{
					junction::Junction, junctions::Junctions, location::Location, Instruction, Xcm,
				},
				xcm::{
					double_encoded::DoubleEncoded,
					v3::{OriginKind, WeightLimit},
					VersionedLocation, VersionedXcm,
				},
				$runtime::RuntimeCall,
			};
			use subxt::{OnlineClient, PolkadotConfig};
			use subxt_signer::sr25519::Keypair;

			/// `sudo(Hrmp::force_open_hrmp_channel(..))`.
			pub async fn open_hrmp_channel(
				client: &OnlineClient<PolkadotConfig>,
				sudo: &Keypair,
				sender: u32,
				recipient: u32,
				max_capacity: u32,
				max_message_size: u32,
			) -> Result<(), anyhow::Error> {
				let call = RuntimeCall::Hrmp(HrmpCall::force_open_hrmp_channel {
					sender: Id(sender),
					recipient: Id(recipient),
					max_capacity,
					max_message_size,
				});
				let tx = crate::$relay::tx().sudo().sudo(call);
				sign_submit_wait(client, &tx, sudo).await
			}

			/// `sudo(XcmPallet::send(..))` carrying an `UnpaidExecution` + `Transact{Superuser}`
			/// message to the given parachain — the governance primitive used to configure the
			/// system parachains.
			pub async fn send_governance_transact(
				client: &OnlineClient<PolkadotConfig>,
				sudo: &Keypair,
				para_id: u32,
				encoded_call: Vec<u8>,
				require_weight_ref_time: u64,
				require_weight_proof_size: u64,
			) -> Result<(), anyhow::Error> {
				let dest = VersionedLocation::V4(Location {
					parents: 0,
					interior: Junctions::X1([Junction::Parachain(para_id)]),
				});
				let message = VersionedXcm::V4(Xcm(vec![
					Instruction::UnpaidExecution {
						weight_limit: WeightLimit::Unlimited,
						check_origin: None,
					},
					Instruction::Transact {
						origin_kind: OriginKind::Superuser,
						require_weight_at_most: Weight {
							ref_time: require_weight_ref_time,
							proof_size: require_weight_proof_size,
						},
						call: DoubleEncoded { encoded: encoded_call },
					},
				]));
				let call = RuntimeCall::XcmPallet(XcmPalletCall::send {
					dest: Box::new(dest),
					message: Box::new(message),
				});
				let tx = crate::$relay::tx().sudo().sudo(call);
				sign_submit_wait(client, &tx, sudo).await
			}
		}
	};
}

macro_rules! asset_hub_ops {
	($name:ident, $ah:ident) => {
		pub mod $name {
			use super::{
				free_balance_at, sign_submit_wait_in_block, sign_submit_wait_in_block_nonce,
			};
			use crate::$ah::runtime_types::{
				staging_xcm::v5::{
					asset::{Asset, AssetFilter, AssetId, Assets, Fungibility, WildAsset},
					junction::{Junction, NetworkId},
					junctions::Junctions,
					location::Location,
					Instruction, Xcm,
				},
				xcm::{
					v3::WeightLimit, VersionedAssetId, VersionedAssets, VersionedLocation,
					VersionedXcm,
				},
			};
			// Re-exported so call sites can name the (per-runtime) transfer type.
			pub use crate::$ah::runtime_types::staging_xcm_executor::traits::asset_transfer::TransferType;
			use subxt::{tx::Payload, OnlineClient, PolkadotConfig};
			use subxt_signer::sr25519::Keypair;

			/// The local native asset, `{ parents: 1, interior: Here }`.
			pub fn native_asset() -> Location {
				Location { parents: 1, interior: Junctions::Here }
			}

			/// A bridged native asset, `{ parents: 2, interior: X1(GlobalConsensus(by_genesis)) }`.
			pub fn bridged_asset(by_genesis: [u8; 32]) -> Location {
				Location {
					parents: 2,
					interior: Junctions::X1([Junction::GlobalConsensus(NetworkId::ByGenesis(
						by_genesis,
					))]),
				}
			}

			/// The remote Asset Hub location, `{ parents: 2, X2(GlobalConsensus, Parachain(1000))
			/// }`.
			pub fn remote_asset_hub(by_genesis: [u8; 32]) -> Location {
				Location {
					parents: 2,
					interior: Junctions::X2([
						Junction::GlobalConsensus(NetworkId::ByGenesis(by_genesis)),
						Junction::Parachain(super::ASSET_HUB_PARA_ID),
					]),
				}
			}

			/// `tx.assetConversion.createPool(native, bridged)`.
			// Unused at this runtime revision (the bridged asset is `is_sufficient` and pays its own
			// fees, so no native<>bridged pool is seeded); kept for when pools are needed again.
			#[allow(dead_code)]
			pub async fn create_pool(
				client: &OnlineClient<PolkadotConfig>,
				signer: &Keypair,
				bridged_by_genesis: [u8; 32],
				nonce: u64,
			) -> Result<(), anyhow::Error> {
				let tx = crate::$ah::tx()
					.asset_conversion()
					.create_pool(native_asset(), bridged_asset(bridged_by_genesis));
				sign_submit_wait_in_block_nonce(client, &tx, signer, nonce).await
			}

			/// `tx.assetConversion.addLiquidity(native, bridged, ..)`.
			#[allow(dead_code)]
			pub async fn add_liquidity(
				client: &OnlineClient<PolkadotConfig>,
				signer: &Keypair,
				bridged_by_genesis: [u8; 32],
				native_amount: u128,
				bridged_amount: u128,
				mint_to: subxt::utils::AccountId32,
				nonce: u64,
			) -> Result<(), anyhow::Error> {
				let tx = crate::$ah::tx().asset_conversion().add_liquidity(
					native_asset(),
					bridged_asset(bridged_by_genesis),
					native_amount,
					bridged_amount,
					1,
					1,
					mint_to,
				);
				sign_submit_wait_in_block_nonce(client, &tx, signer, nonce).await
			}

			/// `tx.polkadotXcm.transferAssetsUsingTypeAndThen(..)` from this Asset Hub to the
			/// remote one, sending `amount` of `asset` to `beneficiary` (an `AccountId32`) and
			/// paying the remote fees out of `asset` itself.
			///
			/// The reserve cannot be auto-detected across a consensus boundary — the auto-detecting
			/// `limitedReserveTransferAssets` fails with `InvalidAssetUnknownReserve` — so the
			/// caller passes `transfer_type` explicitly: `LocalReserve` when sending this Asset
			/// Hub's native token out, `DestinationReserve` when sending a bridged token back to
			/// its origin Asset Hub.
			pub async fn transfer_assets(
				client: &OnlineClient<PolkadotConfig>,
				signer: &Keypair,
				remote_by_genesis: [u8; 32],
				beneficiary: [u8; 32],
				asset: impl Fn() -> Location,
				amount: u128,
				transfer_type: TransferType,
			) -> Result<(), anyhow::Error> {
				let dest = VersionedLocation::V5(remote_asset_hub(remote_by_genesis));
				let assets = VersionedAssets::V5(Assets(vec![Asset {
					id: AssetId(asset()),
					fun: Fungibility::Fungible(amount),
				}]));
				// `asset` is rebuilt (the subxt-generated `Location` isn't `Clone`) to also name
				// the asset used to pay the remote fees.
				let remote_fees_id = VersionedAssetId::V5(AssetId(asset()));
				// On the destination: deposit everything that arrives into `beneficiary`.
				let custom_xcm_on_dest = VersionedXcm::V5(Xcm(vec![Instruction::DepositAsset {
					assets: AssetFilter::Wild(WildAsset::AllCounted(1)),
					beneficiary: Location {
						parents: 0,
						interior: Junctions::X1([Junction::AccountId32 {
							network: None,
							id: beneficiary,
						}]),
					},
				}]));
				// Assets and fees use the same transfer type. The subxt-generated `TransferType`
				// isn't `Clone`, so rebuild the second copy (we only use the data-less variants).
				let fees_transfer_type = match &transfer_type {
					TransferType::Teleport => TransferType::Teleport,
					TransferType::LocalReserve => TransferType::LocalReserve,
					TransferType::DestinationReserve => TransferType::DestinationReserve,
					TransferType::RemoteReserve(_) =>
						return Err(anyhow::anyhow!("RemoteReserve transfer type is not supported")),
				};
				let tx = crate::$ah::tx().polkadot_xcm().transfer_assets_using_type_and_then(
					dest,
					assets,
					transfer_type,
					remote_fees_id,
					fees_transfer_type,
					custom_xcm_on_dest,
					WeightLimit::Unlimited,
				);
				sign_submit_wait_in_block(client, &tx, signer).await
			}

			/// SCALE-encoded `PolkadotXcm::force_xcm_version(remote, version)` call, to be wrapped
			/// in a relay-chain governance `Transact`.
			pub async fn force_xcm_version_call(
				client: &OnlineClient<PolkadotConfig>,
				remote: Location,
				version: u32,
			) -> Result<Vec<u8>, anyhow::Error> {
				let call = crate::$ah::tx().polkadot_xcm().force_xcm_version(remote, version);
				Ok(call.encode_call_data(&client.metadata())?)
			}

			/// SCALE-encoded `ForeignAssets::force_create(bridged_asset, owner, is_sufficient,
			/// min_balance)` call, wrapped in a relay-chain governance `Transact` (root). At this runtime
			/// revision the bridged asset is not pre-registered at genesis, so it is created here before
			/// the bridge can mint it; reserve trust is static in the runtime's XCM config, so no
			/// per-asset reserve registration is needed.
			pub async fn force_create_foreign_asset_call(
				client: &OnlineClient<PolkadotConfig>,
				bridged_by_genesis: [u8; 32],
				owner: subxt::utils::AccountId32,
				min_balance: u128,
			) -> Result<Vec<u8>, anyhow::Error> {
				let who = subxt::utils::MultiAddress::Id(owner);
				let call = crate::$ah::tx().foreign_assets().force_create(
					bridged_asset(bridged_by_genesis),
					who,
					true,
					min_balance,
				);
				Ok(call.encode_call_data(&client.metadata())?)
			}

			/// Free balance of `account` (native asset) via `system.account`.
			pub async fn free_balance(
				client: &OnlineClient<PolkadotConfig>,
				account: [u8; 32],
			) -> Result<u128, anyhow::Error> {
				free_balance_at(client, account).await
			}

			/// Balance of a bridged (foreign) asset held by `account`, or `None` if the account
			/// has no entry for that asset yet.
			pub async fn foreign_asset_balance(
				client: &OnlineClient<PolkadotConfig>,
				asset: Location,
				account: subxt::utils::AccountId32,
			) -> Result<Option<u128>, anyhow::Error> {
				let addr = crate::$ah::storage().foreign_assets().account(asset, account);
				let maybe = client.storage().at_latest().await?.fetch(&addr).await?;
				Ok(maybe.map(|a| a.balance))
			}

						/// Whether the bridged foreign asset is owned by `account` on this Asset Hub.
			pub async fn bridged_asset_owner_is(
				client: &OnlineClient<PolkadotConfig>,
				bridged_by_genesis: [u8; 32],
				account: &subxt::utils::AccountId32,
			) -> Result<bool, anyhow::Error> {
				let addr = crate::$ah::storage().foreign_assets().asset(bridged_asset(bridged_by_genesis));
				match client.storage().at_latest().await?.fetch(&addr).await? {
					Some(details) => Ok(&details.owner == account),
					None => Ok(false),
				}
			}

			/// Whether the HRMP egress channel towards `sibling` is open.
			pub async fn hrmp_egress_open(
				client: &OnlineClient<PolkadotConfig>,
				sibling: u32,
			) -> Result<bool, anyhow::Error> {
				let addr = crate::$ah::storage().parachain_system().relevant_messaging_state();
				let Some(state) = client.storage().at_latest().await?.fetch(&addr).await? else {
					return Ok(false);
				};
				Ok(state.egress_channels.iter().any(|(id, _)| id.0 == sibling))
			}
		}
	};
}

macro_rules! bridge_hub_ops {
	($name:ident, $bh:ident) => {
		pub mod $name {
			use super::{free_balance_at, sign_submit_wait, XCM_VERSION};
			use crate::$bh::runtime_types::staging_xcm::v5::{
				junction::{Junction, NetworkId},
				junctions::Junctions,
				location::Location,
			};
			use subxt::{tx::Payload, OnlineClient, PolkadotConfig};
			use subxt_signer::sr25519::Keypair;

			/// The remote Bridge Hub location, `{ parents: 2, X2(GlobalConsensus, Parachain) }`.
			pub fn remote_bridge_hub(by_genesis: [u8; 32], para: u32) -> Location {
				Location {
					parents: 2,
					interior: Junctions::X2([
						Junction::GlobalConsensus(NetworkId::ByGenesis(by_genesis)),
						Junction::Parachain(para),
					]),
				}
			}

			/// `tx.balances.transferAllowDeath(target, amount)`.
			// Bridge sovereign/reward accounts are pre-funded at genesis, so this is unused; kept
			// as a helper for ad-hoc transfers.
			#[allow(dead_code)]
			pub async fn transfer_balance(
				client: &OnlineClient<PolkadotConfig>,
				signer: &Keypair,
				target: subxt::utils::AccountId32,
				amount: u128,
			) -> Result<(), anyhow::Error> {
				let tx = crate::$bh::tx()
					.balances()
					.transfer_allow_death(subxt::utils::MultiAddress::Id(target), amount);
				sign_submit_wait(client, &tx, signer).await
			}

			/// SCALE-encoded `PolkadotXcm::force_xcm_version(remote, XCM_VERSION)` call.
			pub async fn force_xcm_version_call(
				client: &OnlineClient<PolkadotConfig>,
				remote: Location,
			) -> Result<Vec<u8>, anyhow::Error> {
				let call = crate::$bh::tx().polkadot_xcm().force_xcm_version(remote, XCM_VERSION);
				Ok(call.encode_call_data(&client.metadata())?)
			}

			/// Free balance of `account` via `system.account`.
			pub async fn free_balance(
				client: &OnlineClient<PolkadotConfig>,
				account: [u8; 32],
			) -> Result<u128, anyhow::Error> {
				free_balance_at(client, account).await
			}
		}
	};
}

relay_ops!(relay_rococo, rococo, rococo_runtime);
relay_ops!(relay_westend, westend, westend_runtime);
asset_hub_ops!(asset_hub_rococo, asset_hub_rococo);
asset_hub_ops!(asset_hub_westend, asset_hub_westend);
bridge_hub_ops!(bridge_hub_rococo, bridge_hub_rococo);
bridge_hub_ops!(bridge_hub_westend, bridge_hub_westend);

/// Reads `bridgeRelayers.relayerRewards(relayer, RewardsAccountParams)` on Bridge Hub Rococo.
pub async fn bridge_hub_rococo_relayer_reward(
	client: &OnlineClient<PolkadotConfig>,
	relayer: subxt::utils::AccountId32,
) -> Result<Option<u128>, anyhow::Error> {
	use crate::bridge_hub_rococo::runtime_types::{
		bp_messages::lane::LegacyLaneId,
		bp_relayers::{RewardsAccountOwner, RewardsAccountParams},
	};
	let reward = RewardsAccountParams {
		owner: RewardsAccountOwner::ThisChain,
		bridged_chain_id: BRIDGED_CHAIN_ID_BHWD,
		lane_id: LegacyLaneId(LANE_ID),
	};
	let addr = crate::bridge_hub_rococo::storage()
		.bridge_relayers()
		.relayer_rewards(relayer, reward);
	Ok(client.storage().at_latest().await?.fetch(&addr).await?)
}

/// Reads `bridgeRelayers.relayerRewards(relayer, BridgeReward::RococoWestend(..))` on Bridge Hub
/// Westend (the reward kind is wrapped in the runtime's `BridgeReward` enum there).
pub async fn bridge_hub_westend_relayer_reward(
	client: &OnlineClient<PolkadotConfig>,
	relayer: subxt::utils::AccountId32,
) -> Result<Option<u128>, anyhow::Error> {
	use crate::bridge_hub_westend::runtime_types::{
		bp_messages::lane::LegacyLaneId,
		bp_relayers::{RewardsAccountOwner, RewardsAccountParams},
		bridge_hub_westend_runtime::bridge_common_config::BridgeReward,
	};
	let reward = BridgeReward::RococoWestend(RewardsAccountParams {
		owner: RewardsAccountOwner::ThisChain,
		bridged_chain_id: BRIDGED_CHAIN_ID_BHRO,
		lane_id: LegacyLaneId(LANE_ID),
	});
	let addr = crate::bridge_hub_westend::storage()
		.bridge_relayers()
		.relayer_rewards(relayer, reward);
	Ok(client.storage().at_latest().await?.fetch(&addr).await?)
}

// ---------------------------------------------------------------------------------------------
// Generic subxt helpers (runtime-agnostic).
// ---------------------------------------------------------------------------------------------

/// Whether `e` is a transient failure caused by the reorgy asset/bridge hubs, safe to retry by
/// rebuilding/resubmitting (or re-watching) the transaction: nothing durable was committed.
///
/// subxt surfaces these as distinct error strings, so we match on their `Display` text:
///   * `discarded` / `unknown Block` — the best block read while building the tx (or the block a
///     status referenced) was pruned;
///   * `no longer be found` / `non-finalized fork` — subxt's `TransactionError::BlockNotFound`: the
///     block that had just reported the tx in-block was reorged away;
///   * `Invalid Transaction` — a just-submitted predecessor from the same signer is not yet
///     reflected in the queried nonce.
fn is_transient_reorg_error(e: &subxt::Error) -> bool {
	let s = e.to_string();
	s.contains("discarded") ||
		s.contains("unknown Block") ||
		s.contains("no longer be found") ||
		s.contains("non-finalized fork") ||
		s.contains("Invalid Transaction")
}

/// Signs `call` with `signer`, submits it and waits for finalized success.
///
/// `wait_for_finalized_success` is reorg-tolerant: it watches the transaction through best-block
/// retractions until it lands in a finalized block. The only racy step is building the transaction
/// (subxt reads the nonce/runtime at the best block, which the fast asset-hub collator can prune
/// before the call returns — surfacing as `unknown Block`/`State already discarded`); since nothing
/// is submitted when that happens, we simply retry building it against fresh state.
pub async fn sign_submit_wait<C: Payload>(
	client: &OnlineClient<PolkadotConfig>,
	call: &C,
	signer: &Keypair,
) -> Result<(), anyhow::Error> {
	const ATTEMPTS: usize = 12;
	for attempt in 1..=ATTEMPTS {
		let params = DefaultExtrinsicParamsBuilder::new().immortal().build();
		// Both the submit and the finalized-success wait can fail transiently on the reorgy hubs
		// (pre-pool rejection while building, or the finalized-success block reorged away). Neither
		// commits anything durable, so a transient error is retried with a fresh build.
		let result = match client.tx().sign_and_submit_then_watch(call, signer, params).await {
			Ok(progress) => progress.wait_for_finalized_success().await.map(|_| ()),
			Err(e) => Err(e),
		};
		match result {
			Ok(()) => return Ok(()),
			Err(e) if attempt < ATTEMPTS && is_transient_reorg_error(&e) => {
				sleep(Duration::from_secs(3)).await;
				continue;
			},
			Err(e) => return Err(e.into()),
		}
	}
	unreachable!("loop returns or errors on the final attempt")
}

/// Signs `call` with `signer`, submits it and waits only for **in-block** success (not finality).
///
/// Used for the asset-hub asset transfers; the test verifies the cross-chain outcomes via
/// [`retry_until`], so inclusion is sufficient. Tolerant of the asset hubs' reorgs: a pruned best
/// block while building the tx (`unknown Block`) is retried (nothing was submitted), a retracted
/// in-block report is handled by re-watching the same tx, and a tx that a reorg re-validates as
/// `Invalid`/`Dropped` (so it never reached the canonical chain) is rebuilt and resubmitted.
pub async fn sign_submit_wait_in_block<C: Payload>(
	client: &OnlineClient<PolkadotConfig>,
	call: &C,
	signer: &Keypair,
) -> Result<(), anyhow::Error> {
	use subxt::tx::TxStatus;
	const ATTEMPTS: usize = 12;
	'attempts: for attempt in 1..=ATTEMPTS {
		let params = DefaultExtrinsicParamsBuilder::new().immortal().build();
		let mut progress = match client.tx().sign_and_submit_then_watch(call, signer, params).await
		{
			Ok(p) => p,
			// Transient pre-pool failures (nothing submitted): the best block read while building
			// the tx can be pruned (`unknown Block`/`discarded`), or a just-submitted
			// predecessor from the same signer may not be reflected in the queried nonce yet
			// (`Invalid Transaction`). A short wait lets the node catch up; a fresh nonce is
			// queried on the next attempt.
			Err(e) if attempt < ATTEMPTS && is_transient_reorg_error(&e) => {
				sleep(Duration::from_secs(3)).await;
				continue 'attempts;
			},
			Err(e) => return Err(e.into()),
		};
		loop {
			let status = match progress.next().await {
				Some(Ok(status)) => status,
				// The status subscription itself can fail transiently when the referenced block is
				// reorged away (`BlockNotFound` etc.); nothing is committed, so rebuild + resubmit.
				Some(Err(e)) if attempt < ATTEMPTS && is_transient_reorg_error(&e) => {
					sleep(Duration::from_secs(3)).await;
					continue 'attempts;
				},
				Some(Err(e)) => return Err(e.into()),
				None => break,
			};
			match status {
				TxStatus::InBestBlock(in_block) | TxStatus::InFinalizedBlock(in_block) => {
					match in_block.wait_for_success().await {
						Ok(_) => return Ok(()),
						// in-block report retracted by a reorg (`discarded`/`unknown Block`/
						// `BlockNotFound`); keep watching the same tx for re-inclusion.
						Err(e) if is_transient_reorg_error(&e) => continue,
						Err(e) => return Err(e.into()),
					}
				},
				// On the reorgy asset hubs the node re-validates pending txs against a new best
				// chain and can report this tx `Invalid`/`Dropped`/`Error` (e.g. the fork it
				// sat in was pruned). Such a tx is not on the canonical chain, so its nonce is
				// unconsumed and it is safe to rebuild against fresh state and resubmit rather
				// than failing the test.
				TxStatus::Error { .. } | TxStatus::Invalid { .. } | TxStatus::Dropped { .. } => {
					if attempt < ATTEMPTS {
						sleep(Duration::from_secs(3)).await;
						continue 'attempts;
					}
					return Err(anyhow!(
						"transaction kept being invalidated by reorgs after {ATTEMPTS} attempts"
					));
				},
				_ => continue,
			}
		}
		// Status stream ended without an inclusion verdict; resubmit.
		if attempt < ATTEMPTS {
			sleep(Duration::from_secs(3)).await;
			continue 'attempts;
		}
		return Err(anyhow!("transaction status stream ended before inclusion"));
	}
	unreachable!("loop returns or continues on the final attempt")
}

/// Like [`sign_submit_wait_in_block`] but with an explicit `nonce`, for submitting a rapid sequence
/// of transactions from the same signer (e.g. create_pool then add_liquidity) on the reorgy asset
/// hubs: explicit sequential nonces queue as `Future` instead of racing the auto-queried nonce.
#[allow(dead_code)]
pub async fn sign_submit_wait_in_block_nonce<C: Payload>(
	client: &OnlineClient<PolkadotConfig>,
	call: &C,
	signer: &Keypair,
	nonce: u64,
) -> Result<(), anyhow::Error> {
	use subxt::tx::TxStatus;
	const ATTEMPTS: usize = 12;
	let mut progress = None;
	for attempt in 1..=ATTEMPTS {
		let params = DefaultExtrinsicParamsBuilder::new().nonce(nonce).build();
		match client.tx().sign_and_submit_then_watch(call, signer, params).await {
			Ok(p) => {
				progress = Some(p);
				break;
			},
			Err(e) if attempt < ATTEMPTS && is_transient_reorg_error(&e) => {
				sleep(Duration::from_secs(3)).await;
				continue;
			},
			Err(e) => return Err(e.into()),
		}
	}
	let mut progress = progress.ok_or_else(|| anyhow!("could not submit transaction"))?;
	while let Some(status) = progress.next().await.transpose()? {
		match status {
			TxStatus::InBestBlock(in_block) | TxStatus::InFinalizedBlock(in_block) =>
				match in_block.wait_for_success().await {
					Ok(_) => return Ok(()),
					Err(e) if is_transient_reorg_error(&e) => continue,
					Err(e) => return Err(e.into()),
				},
			TxStatus::Error { message } |
			TxStatus::Invalid { message } |
			TxStatus::Dropped { message } =>
				return Err(anyhow!("transaction failed before inclusion: {message}")),
			_ => continue,
		}
	}
	Err(anyhow!("transaction status stream ended before inclusion"))
}
/// Free balance of `account` via dynamic `System::Account` storage (works for any runtime).
pub async fn free_balance_at(
	client: &OnlineClient<PolkadotConfig>,
	account: [u8; 32],
) -> Result<u128, anyhow::Error> {
	use subxt::ext::scale_value::{At, Value};
	let addr = subxt::dynamic::storage("System", "Account", vec![Value::from_bytes(account)]);
	let Some(value) = client.storage().at_latest().await?.fetch(&addr).await? else {
		return Ok(0);
	};
	let value = value.to_value()?;
	value
		.at("data")
		.and_then(|data| data.at("free"))
		.and_then(|free| free.as_u128())
		.ok_or_else(|| anyhow!("unexpected System::Account layout"))
}

/// Calls the `<Chain>FinalityApi_best_finalized` runtime API and returns the best finalized
/// bridged header number.
pub async fn best_finalized_bridged_header(
	client: &OnlineClient<PolkadotConfig>,
	finality_api: &str,
) -> Result<Option<u32>, anyhow::Error> {
	let method = format!("{finality_api}_best_finalized");
	let encoded = client.runtime_api().at_latest().await?.call_raw(method.as_str(), None).await?;
	// `Option<HeaderId<Hash, Number>>` where `HeaderId(Number, Hash)` — we only need the number.
	let decoded: Option<(u32, [u8; 32])> = Decode::decode(&mut &encoded[..])?;
	Ok(decoded.map(|(number, _hash)| number))
}

/// Waits until `client` reports a best block of at least `height`, or `timeout` elapses.
///
/// Uses best (not finalized) blocks: confirms block production without depending on the finality
/// chain.
pub async fn wait_for_block_height(
	client: &OnlineClient<PolkadotConfig>,
	height: u32,
	timeout: Duration,
) -> Result<(), anyhow::Error> {
	let mut sub = client.blocks().subscribe_best().await?;
	let deadline = Instant::now() + timeout;
	while let Ok(Some(block)) = timeout_at(deadline, sub.next()).await {
		if block?.number() >= height {
			return Ok(());
		}
	}
	Err(anyhow!("timeout waiting for block height {height}"))
}

/// Waits until `client` reports a *finalized* block of at least `height`, or `timeout` elapses.
///
/// Gates `init-bridge`: a freshly-started bridge hub reorgs heavily at the tip, so init waits for
/// steady finalization first, otherwise the init tx can be orphaned and never re-included.
pub async fn wait_for_finalized_height(
	client: &OnlineClient<PolkadotConfig>,
	height: u32,
	timeout: Duration,
) -> Result<(), anyhow::Error> {
	let mut sub = client.blocks().subscribe_finalized().await?;
	let deadline = Instant::now() + timeout;
	while let Ok(Some(block)) = timeout_at(deadline, sub.next()).await {
		if block?.number() >= height {
			return Ok(());
		}
	}
	Err(anyhow!("timeout waiting for finalized block height {height}"))
}

/// Subscribes to best blocks of `client` for `duration` and counts the GRANDPA
/// (`UpdatedBestFinalizedHeader`) and parachain (`UpdatedParachainHead`) header-import events
/// emitted by the given bridge pallets.
pub async fn count_synced_headers(
	client: &OnlineClient<PolkadotConfig>,
	grandpa_pallet: &str,
	parachains_pallet: &str,
	duration: Duration,
) -> Result<(u32, u32), anyhow::Error> {
	let mut sub = client.blocks().subscribe_best().await?;
	let deadline = Instant::now() + duration;
	let (mut grandpa_headers, mut parachain_headers) = (0u32, 0u32);
	while let Ok(Some(block)) = timeout_at(deadline, sub.next()).await {
		let block = block?;
		for event in block.events().await?.iter() {
			let event = event?;
			match (event.pallet_name(), event.variant_name()) {
				(p, "UpdatedBestFinalizedHeader") if p == grandpa_pallet => grandpa_headers += 1,
				(p, "UpdatedParachainHead") if p == parachains_pallet => parachain_headers += 1,
				_ => {},
			}
		}
	}
	Ok((grandpa_headers, parachain_headers))
}

/// Polls `f` every 6s until it yields `Some`, or `timeout` elapses.
pub async fn retry_until<F, Fut, T>(timeout: Duration, mut f: F) -> Result<T, anyhow::Error>
where
	F: FnMut() -> Fut,
	Fut: Future<Output = Result<Option<T>, anyhow::Error>>,
{
	let deadline = Instant::now() + timeout;
	loop {
		if let Some(value) = f().await? {
			return Ok(value);
		}
		if Instant::now() >= deadline {
			return Err(anyhow!("timeout in retry_until"));
		}
		sleep(Duration::from_secs(6)).await;
	}
}

// ---------------------------------------------------------------------------------------------
// `substrate-relay` subprocess driver.
// ---------------------------------------------------------------------------------------------

const RELAYER_RUST_LOG: &str = "runtime=trace,rpc=trace,bridge=trace";

fn relayer_binary() -> PathBuf {
	if let Ok(path) = std::env::var("SUBSTRATE_RELAY_BINARY") {
		return PathBuf::from(path);
	}
	let home = std::env::var("HOME").unwrap_or_default();
	PathBuf::from(home).join("local_bridge_testing/bin/substrate-relay")
}

/// A spawned long-running `substrate-relay` process. Killed when dropped.
pub struct Relayer(Child);

impl Drop for Relayer {
	fn drop(&mut self) {
		let _ = self.0.start_kill();
	}
}

fn spawn_relayer(args: &[&str]) -> Result<Relayer, anyhow::Error> {
	log::info!("Spawning substrate-relay {}", args.join(" "));
	let child = Command::new(relayer_binary())
		.args(args)
		.env("RUST_LOG", RELAYER_RUST_LOG)
		.kill_on_drop(true)
		.spawn()?;
	Ok(Relayer(child))
}

#[allow(dead_code)]
async fn run_relayer_to_completion(args: &[&str]) -> Result<(), anyhow::Error> {
	log::info!("Running substrate-relay {}", args.join(" "));
	let status = Command::new(relayer_binary())
		.args(args)
		.env("RUST_LOG", RELAYER_RUST_LOG)
		.status()
		.await?;
	if !status.success() {
		return Err(anyhow!("substrate-relay {:?} exited with {status}", args));
	}
	Ok(())
}

/// Returns `true` if the bridge GRANDPA pallet `grandpa_pallet` on `client`'s chain reports
/// `operating_mode == Normal` at the latest **finalized** block.
///
/// Read dynamically (from the node's runtime metadata) at a finalized block on purpose:
/// `BasicOperatingMode` SCALE-encodes to a single byte (`0` = Normal, `1` = Halted), and reading
/// at finalized (not best) means a reorg-victim block can't give a false answer.
async fn bridge_operating_mode_normal_at_finalized(
	client: &OnlineClient<PolkadotConfig>,
	grandpa_pallet: &str,
) -> Result<bool, anyhow::Error> {
	use subxt::ext::scale_value::Value;
	let addr = subxt::dynamic::storage(grandpa_pallet, "PalletOperatingMode", Vec::<Value>::new());
	let mut sub = client.blocks().subscribe_finalized().await?;
	let block = match sub.next().await {
		Some(block) => block?,
		None => return Ok(false),
	};
	match block.storage().fetch(&addr).await? {
		Some(v) => Ok(v.encoded().first() == Some(&0u8)),
		None => Ok(false),
	}
}

/// Runs the idempotent `init-bridge` command until the bridge is confirmed operational.
///
/// The relay's `init-bridge` decides it is done by reading `is_initialized` at the **best**
/// (non-finalized) header. On a loaded host the bridge hub reorgs ~tens of blocks deep, so the
/// init tx can land on a branch that is briefly best — fooling that check into reporting success —
/// and is then retracted, leaving the canonical chain `Halted`. We therefore drive init from the
/// **finalized** state: before each attempt we check `operating_mode == Normal` at a finalized
/// block, and only stop once that holds. Each attempt re-submits (the relay no-ops fast when it
/// believes it is already initialized), so once a submission finalizes the bridge is operational.
async fn init_bridge_confirmed(
	args: &[&str],
	target_client: &OnlineClient<PolkadotConfig>,
	grandpa_pallet: &str,
) -> Result<(), anyhow::Error> {
	const ATTEMPTS: usize = 15;
	const PER_ATTEMPT: Duration = Duration::from_secs(120);
	// Pause between attempts so finality can advance and to avoid hammering the target node with
	// rapid connect/disconnect cycles (a transient client/relay error must not spin the loop).
	const BETWEEN_ATTEMPTS: Duration = Duration::from_secs(12);
	for attempt in 1..=ATTEMPTS {
		if bridge_operating_mode_normal_at_finalized(target_client, grandpa_pallet)
			.await
			.unwrap_or(false)
		{
			log::info!("bridge {grandpa_pallet} confirmed operational (Normal) at finalized");
			return Ok(());
		}
		log::info!("init-bridge attempt {attempt}/{ATTEMPTS}: substrate-relay {}", args.join(" "));
		let mut child = Command::new(relayer_binary())
			.args(args)
			.env("RUST_LOG", RELAYER_RUST_LOG)
			.kill_on_drop(true)
			.spawn()?;
		if tokio::time::timeout(PER_ATTEMPT, child.wait()).await.is_err() {
			log::warn!(
				"init-bridge attempt {attempt} for {grandpa_pallet} not finalized within \
				 {PER_ATTEMPT:?}; re-checking finalized state and retrying"
			);
		}
		let _ = child.start_kill();
		let _ = child.wait().await;
		sleep(BETWEEN_ATTEMPTS).await;
	}
	if bridge_operating_mode_normal_at_finalized(target_client, grandpa_pallet)
		.await
		.unwrap_or(false)
	{
		return Ok(());
	}
	Err(anyhow!("bridge {grandpa_pallet} did not become operational after {ATTEMPTS} attempts"))
}

// ---------------------------------------------------------------------------------------------
// Network spawning.
// ---------------------------------------------------------------------------------------------

/// The full Rococo <> Westend bridge environment: both networks plus any running relayer
/// processes (kept alive for the lifetime of the value).
pub struct BridgeTestEnv {
	pub rococo: Network<LocalFileSystem>,
	pub westend: Network<LocalFileSystem>,
	_relayers: Vec<Relayer>,
}

/// Genesis `balances` override for a bridge hub.
///
/// `with_genesis_overrides` *replaces* the `balances.balances` array (it doesn't append), so we
/// must re-list the well-known dev accounts that the network relies on — the collators (Alice/Bob)
/// and the relayer signers (`//Bob`, `//Charlie`, `//Dave`, `//Eve`, `//Ferdie`) all need balance
/// to pay fees — and then add the bridge sovereign/reward accounts. Account ids are derived from
/// the dev keys (no hard-coded SS58), amounts are well within `u64` so they serialize cleanly.
fn bridge_hub_balances_override(sovereign_accounts: &[&str]) -> serde_json::Value {
	const DEV_FUNDING: u64 = 1 << 60;
	let mut balances: Vec<serde_json::Value> =
		[dev::alice(), dev::bob(), dev::charlie(), dev::dave(), dev::eve(), dev::ferdie()]
			.iter()
			.map(|k| serde_json::json!([dev_account(k).to_string(), DEV_FUNDING]))
			.collect();
	for account in sovereign_accounts {
		balances.push(serde_json::json!([*account, SOVEREIGN_FUNDING as u64]));
	}
	serde_json::json!({ "balances": { "balances": balances } })
}

/// Relay-chain genesis override allowing a non-zero relay-parent ancestry. asset-hub-westend sets
/// `RELAY_PARENT_OFFSET = 1` (authors on a relay parent one block behind best), so the relay must
/// accept candidates whose relay parent is an ancestor of its best block; the local presets ship
/// `allowed_ancestry_len: 0` (latest only), which would reject them.
fn relay_async_backing_override() -> serde_json::Value {
	serde_json::json!({
		"configuration": {
			"config": {
				// `max_candidate_depth: 1` keeps the bridge-hub unincluded segment shallow so a
				// fast-runtime relay reorg strands at most ~1 already-authored parablock rather than
				// a deep pipelined segment. With depth 3 the collator reached #170 while the relay
				// had only backed #164/included #163, then reorged #170->#163, discarding the
				// relayer's in-flight finality/parachain-head/message proof txs (=> ProofSubmissionTxLost).
				// Empirically depth 1 gave the fewest such losses (~8, vs ~20 at depth 3 and ~10 at
				// depth 0) — the best-performing config found for these bridge zombienet-sdk tests
				// under fast-runtime relay churn (both bridge directions deliver; the test still times
				// out on confirmation/round-trip legs due to relay-chain reorg churn beyond parachain
				// config's reach). `allowed_ancestry_len` stays 2 — asset-hub-westend authors with
				// RELAY_PARENT_OFFSET=1 and needs >=1.
				"async_backing_params": { "max_candidate_depth": 1, "allowed_ancestry_len": 2 }
			}
		}
	})
}

// Default node images for the `docker` provider, overridable via `POLKADOT_IMAGE` /
// `CUMULUS_IMAGE`. Tagged with the polkadot-sdk revision pinned in `Cargo.lock` (see `build.rs`).
const DEFAULT_POLKADOT_IMAGE: &str =
	concat!("docker.io/paritypr/polkadot-debug:", env!("POLKADOT_SDK_SHORT_HASH"));
const DEFAULT_CUMULUS_IMAGE: &str =
	concat!("docker.io/paritypr/polkadot-parachain-debug:", env!("POLKADOT_SDK_SHORT_HASH"));

struct NodeImages {
	polkadot: String,
	cumulus: String,
}

fn node_images() -> NodeImages {
	NodeImages {
		polkadot: std::env::var("POLKADOT_IMAGE")
			.unwrap_or_else(|_| DEFAULT_POLKADOT_IMAGE.to_string()),
		cumulus: std::env::var("CUMULUS_IMAGE")
			.unwrap_or_else(|_| DEFAULT_CUMULUS_IMAGE.to_string()),
	}
}

fn rococo_network_config() -> Result<NetworkConfig, anyhow::Error> {
	let images = node_images();
	// Bridge hubs use the default (fork-aware) transaction pool, which re-validates pending txs
	// across reorgs so the relayer's proof transactions survive the bridge hubs' relay-parent
	// reorgs.
	let bh_args: Vec<Arg> = vec![
		"-lparachain=info,runtime::bridge=trace,xcm=debug,txpool=debug".into(),
		// Slot-based authoring paces bridge-hub block production to the relay chain's backing
		// instead of over-producing an unincluded segment (we saw the collator reach #170 while
		// the relay had only backed #164/included #163, forcing a ~7-block reorg back to #163
		// whenever the fast-runtime relay chain reorged — which discards the relay's in-flight
		// finality/parachain-head/message proof txs => ProofSubmissionTxLost, reverse leg stuck).
		// Matches what asset-hub-westend already requires.
		"--authoring".into(),
		"slot-based".into(),
	];
	let ah_args: Vec<Arg> =
		vec!["-lparachain=info,xcm=debug,runtime::bridge=trace,txpool=debug".into()];
	NetworkConfigBuilder::new()
		.with_relaychain(|r| {
			r.with_chain("rococo-local")
				.with_default_command("polkadot")
				.with_default_image(images.polkadot.as_str())
				.with_default_args(vec!["-lparachain=info,xcm=debug".into()])
				.with_genesis_overrides(relay_async_backing_override())
				.with_validator(|n| {
					n.with_name("alice-rococo-validator").with_initial_balance(2_000_000_000_000)
				})
				.with_validator(|n| {
					n.with_name("bob-rococo-validator").with_initial_balance(2_000_000_000_000)
				})
				.with_validator(|n| {
					n.with_name("charlie-rococo-validator").with_initial_balance(2_000_000_000_000)
				})
		})
		.with_parachain(|p| {
			p.with_id(BRIDGE_HUB_ROCOCO_PARA_ID)
				.with_chain("bridge-hub-rococo-local")
				.cumulus_based(true)
				.with_default_command("polkadot-parachain")
				.with_default_image(images.cumulus.as_str())
				// Pre-fund the bridge sovereign/reward accounts at genesis instead of via
				// post-spawn transactions: a freshly launched bridge hub reorgs at the tip, which
				// invalidates submitted funding txs (and the bridge-hub tx path is fragile under
				// the relayer reward/obsolete extensions). Genesis state is the idiomatic
				// zombienet-sdk way and sidesteps that entirely.
				.with_genesis_overrides(bridge_hub_balances_override(&[
					ASSET_HUB_SOVEREIGN_AT_BRIDGE_HUB,
					BHR_LANE_THIS_CHAIN,
					BHR_LANE_BRIDGED_CHAIN,
				]))
				// Single collator on the bridge hub. A second collator caused competing block
				// production (fork war): the bridge hub's best block oscillated between forks and
				// the block carrying the relayer's bridged-finality / parachain-head update kept
				// getting retracted before finalization, so the relayer's proof tx was repeatedly
				// invalidated and the bridged side never made durable progress. One collator builds
				// linearly with no fork competition, so those updates land and finalize.
				.with_collator(|n| {
					n.with_name("bridge-hub-rococo-collator1").with_args(bh_args.clone())
				})
		})
		.with_parachain(|p| {
			p.with_id(ASSET_HUB_PARA_ID)
				.with_chain("asset-hub-rococo-local")
				.cumulus_based(true)
				.with_default_command("polkadot-parachain")
				.with_default_image(images.cumulus.as_str())
				// Two asset-hub collators.
				.with_collator(|n| {
					n.with_name("asset-hub-rococo-collator1").with_args(ah_args.clone())
				})
				.with_collator(|n| {
					n.with_name("asset-hub-rococo-collator2").with_args(ah_args.clone())
				})
		})
		.with_global_settings(global_settings)
		.build()
		.map_err(config_errs)
}

fn westend_network_config() -> Result<NetworkConfig, anyhow::Error> {
	let images = node_images();
	// See `rococo_network_config`: bridge hubs use the default fork-aware pool so the relayer's
	// proof txs survive reorgs.
	let bh_args: Vec<Arg> = vec![
		"-lparachain=info,runtime::bridge=trace,xcm=debug,txpool=debug".into(),
		// Slot-based authoring paces bridge-hub block production to the relay chain's backing
		// instead of over-producing an unincluded segment (we saw the collator reach #170 while
		// the relay had only backed #164/included #163, forcing a ~7-block reorg back to #163
		// whenever the fast-runtime relay chain reorged — which discards the relay's in-flight
		// finality/parachain-head/message proof txs => ProofSubmissionTxLost, reverse leg stuck).
		// Matches what asset-hub-westend already requires.
		"--authoring".into(),
		"slot-based".into(),
	];
	// The asset-hub-westend runtime authors via the slot-based collator (it panics under
	// default/lookahead authoring — the block lacks the expected relay-parent descendants).
	let ah_args: Vec<Arg> = vec![
		"-lparachain=info,xcm=debug,runtime::bridge=trace,txpool=debug".into(),
		"--authoring".into(),
		"slot-based".into(),
	];
	NetworkConfigBuilder::new()
		.with_relaychain(|r| {
			r.with_chain("westend-local")
				.with_default_command("polkadot")
				.with_default_image(images.polkadot.as_str())
				.with_default_args(vec!["-lparachain=info,xcm=debug".into()])
				.with_genesis_overrides(relay_async_backing_override())
				.with_validator(|n| {
					n.with_name("alice-westend-validator").with_initial_balance(2_000_000_000_000)
				})
				.with_validator(|n| {
					n.with_name("bob-westend-validator").with_initial_balance(2_000_000_000_000)
				})
				.with_validator(|n| {
					n.with_name("charlie-westend-validator").with_initial_balance(2_000_000_000_000)
				})
		})
		.with_parachain(|p| {
			p.with_id(BRIDGE_HUB_WESTEND_PARA_ID)
				.with_chain("bridge-hub-westend-local")
				.cumulus_based(true)
				.with_default_command("polkadot-parachain")
				.with_default_image(images.cumulus.as_str())
				// Pre-fund the bridge sovereign/reward accounts at genesis (see
				// `rococo_network_config`).
				.with_genesis_overrides(bridge_hub_balances_override(&[
					ASSET_HUB_SOVEREIGN_AT_BRIDGE_HUB,
					BHW_LANE_THIS_CHAIN,
					BHW_LANE_BRIDGED_CHAIN,
				]))
				// Single collator on the bridge hub (see `rococo_network_config`): a second
				// collator fork-wars and retracts the block carrying the relayer's
				// bridged-finality / parachain-head update before finalization. One collator
				// builds linearly so those updates land and finalize.
				.with_collator(|n| {
					n.with_name("bridge-hub-westend-collator1").with_args(bh_args.clone())
				})
		})
		.with_parachain(|p| {
			p.with_id(ASSET_HUB_PARA_ID)
				.with_chain("asset-hub-westend-local")
				.cumulus_based(true)
				.with_default_command("polkadot-parachain")
				.with_default_image(images.cumulus.as_str())
				// Two slot-based collators.
				.with_collator(|n| {
					n.with_name("asset-hub-westend-collator1").with_args(ah_args.clone())
				})
				.with_collator(|n| {
					n.with_name("asset-hub-westend-collator2").with_args(ah_args.clone())
				})
		})
		.with_global_settings(global_settings)
		.build()
		.map_err(config_errs)
}

/// Shared global settings for both networks. We disable `tear_down_on_failure` so the background
/// node-monitoring task (which declares a node "crashed" if its metrics endpoint does not respond
/// within ~5s) does not tear the network down on a transient, load-induced timeout — the same
/// approach the polkadot zombienet-sdk tests use on busy CI runners.
fn global_settings(
	settings: zombienet_sdk::GlobalSettingsBuilder,
) -> zombienet_sdk::GlobalSettingsBuilder {
	// `with_node_spawn_timeout` takes seconds (zombienet's own `Duration` alias, i.e. `u32`).
	settings.with_tear_down_on_failure(false).with_node_spawn_timeout(600)
}

fn config_errs(errs: Vec<anyhow::Error>) -> anyhow::Error {
	anyhow!(
		"network config errors: {}",
		errs.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; ")
	)
}

impl BridgeTestEnv {
	/// Spawns both networks and, depending on the flags, initializes the bridge and starts the
	/// relayer.
	pub async fn spawn(init: bool, start_relayer: bool) -> Result<Self, anyhow::Error> {
		let _ = env_logger::try_init_from_env(
			env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
		);

		let spawn_fn = get_spawn_fn();
        // Rococo and Westend are independent networks; spawn them concurrently instead of serially
        // (saves ~one full network spawn — ~35s in local runs).
        log::info!("Spawning Rococo and Westend networks concurrently");
        let (rococo, westend) = tokio::try_join!(
                async { spawn_fn(rococo_network_config()?).await.map_err(|e| anyhow!("Rococo spawn failed: {e}")) },
                async { spawn_fn(westend_network_config()?).await.map_err(|e| anyhow!("Westend spawn failed: {e}")) },
        )?;

		let mut env = BridgeTestEnv { rococo, westend, _relayers: Vec::new() };

		if init {
			env.init_bridge().await?;
		}
		if start_relayer {
			env.start_relayer().await?;
		}
		Ok(env)
	}

	async fn client_of(
		network: &Network<LocalFileSystem>,
		node: &str,
	) -> Result<OnlineClient<PolkadotConfig>, anyhow::Error> {
		let node = network.get_node(node)?;
		let client: OnlineClient<PolkadotConfig> = node.wait_client().await?;
		Ok(client)
	}

	pub async fn rococo_relay_client(&self) -> Result<OnlineClient<PolkadotConfig>, anyhow::Error> {
		Self::client_of(&self.rococo, "alice-rococo-validator").await
	}
	pub async fn westend_relay_client(
		&self,
	) -> Result<OnlineClient<PolkadotConfig>, anyhow::Error> {
		Self::client_of(&self.westend, "alice-westend-validator").await
	}
	pub async fn asset_hub_rococo_client(
		&self,
	) -> Result<OnlineClient<PolkadotConfig>, anyhow::Error> {
		Self::client_of(&self.rococo, "asset-hub-rococo-collator1").await
	}
	pub async fn asset_hub_westend_client(
		&self,
	) -> Result<OnlineClient<PolkadotConfig>, anyhow::Error> {
		Self::client_of(&self.westend, "asset-hub-westend-collator1").await
	}
	pub async fn bridge_hub_rococo_client(
		&self,
	) -> Result<OnlineClient<PolkadotConfig>, anyhow::Error> {
		Self::client_of(&self.rococo, "bridge-hub-rococo-collator1").await
	}
	pub async fn bridge_hub_westend_client(
		&self,
	) -> Result<OnlineClient<PolkadotConfig>, anyhow::Error> {
		Self::client_of(&self.westend, "bridge-hub-westend-collator1").await
	}

	/// Initializes both sides of the bridge: waits for block production, opens HRMP channels, sets
	/// remote XCM versions and creates the bridged foreign assets.
	async fn init_bridge(&self) -> Result<(), anyhow::Error> {
		let rococo_relay = self.rococo_relay_client().await?;
		let westend_relay = self.westend_relay_client().await?;
		let ahr = self.asset_hub_rococo_client().await?;
		let ahw = self.asset_hub_westend_client().await?;
		let bhr = self.bridge_hub_rococo_client().await?;
		let bhw = self.bridge_hub_westend_client().await?;

		let alice = dev::alice();

		log::info!("Waiting for parachains to start producing blocks");
		wait_for_block_height(&ahr, 10, Duration::from_secs(300)).await?;
		wait_for_block_height(&bhr, 10, Duration::from_secs(300)).await?;
		wait_for_block_height(&ahw, 10, Duration::from_secs(300)).await?;
		wait_for_block_height(&bhw, 10, Duration::from_secs(300)).await?;

		log::info!("Opening HRMP channels and setting remote XCM versions");
		relay_rococo::open_hrmp_channel(
			&rococo_relay,
			&alice,
			ASSET_HUB_PARA_ID,
			BRIDGE_HUB_ROCOCO_PARA_ID,
			4,
			524288,
		)
		.await?;
		relay_rococo::open_hrmp_channel(
			&rococo_relay,
			&alice,
			BRIDGE_HUB_ROCOCO_PARA_ID,
			ASSET_HUB_PARA_ID,
			4,
			524288,
		)
		.await?;
		relay_westend::open_hrmp_channel(
			&westend_relay,
			&alice,
			ASSET_HUB_PARA_ID,
			BRIDGE_HUB_WESTEND_PARA_ID,
			4,
			524288,
		)
		.await?;
		relay_westend::open_hrmp_channel(
			&westend_relay,
			&alice,
			BRIDGE_HUB_WESTEND_PARA_ID,
			ASSET_HUB_PARA_ID,
			4,
			524288,
		)
		.await?;

		// Remote XCM versions (Asset Hub <-> Asset Hub, Bridge Hub <-> Bridge Hub).
		let ahw_on_ahr = asset_hub_rococo::remote_asset_hub(WESTEND_GENESIS_HASH);
		let force_ahw =
			asset_hub_rococo::force_xcm_version_call(&ahr, ahw_on_ahr, XCM_VERSION).await?;
		relay_rococo::send_governance_transact(
			&rococo_relay,
			&alice,
			ASSET_HUB_PARA_ID,
			force_ahw,
			200_000_000,
			12_000,
		)
		.await?;

		let bhw_on_bhr =
			bridge_hub_rococo::remote_bridge_hub(WESTEND_GENESIS_HASH, BRIDGE_HUB_WESTEND_PARA_ID);
		let force_bhw = bridge_hub_rococo::force_xcm_version_call(&bhr, bhw_on_bhr).await?;
		relay_rococo::send_governance_transact(
			&rococo_relay,
			&alice,
			BRIDGE_HUB_ROCOCO_PARA_ID,
			force_bhw,
			200_000_000,
			12_000,
		)
		.await?;

		let ahr_on_ahw = asset_hub_westend::remote_asset_hub(ROCOCO_GENESIS_HASH);
		let force_ahr =
			asset_hub_westend::force_xcm_version_call(&ahw, ahr_on_ahw, XCM_VERSION).await?;
		relay_westend::send_governance_transact(
			&westend_relay,
			&alice,
			ASSET_HUB_PARA_ID,
			force_ahr,
			200_000_000,
			12_000,
		)
		.await?;

		let bhr_on_bhw =
			bridge_hub_westend::remote_bridge_hub(ROCOCO_GENESIS_HASH, BRIDGE_HUB_ROCOCO_PARA_ID);
		let force_bhr = bridge_hub_westend::force_xcm_version_call(&bhw, bhr_on_bhw).await?;
		relay_westend::send_governance_transact(
			&westend_relay,
			&alice,
			BRIDGE_HUB_WESTEND_PARA_ID,
			force_bhr,
			200_000_000,
			12_000,
		)
		.await?;

		log::info!("Waiting for HRMP channels to open");
		retry_until(Duration::from_secs(600), || {
			let ahr = ahr.clone();
			async move {
				Ok(asset_hub_rococo::hrmp_egress_open(&ahr, BRIDGE_HUB_ROCOCO_PARA_ID)
					.await?
					.then_some(()))
			}
		})
		.await?;
		retry_until(Duration::from_secs(600), || {
			let ahw = ahw.clone();
			async move {
				Ok(asset_hub_westend::hrmp_egress_open(&ahw, BRIDGE_HUB_WESTEND_PARA_ID)
					.await?
					.then_some(()))
			}
		})
		.await?;

		// At this runtime revision the bridged foreign asset is not pre-registered at genesis, and
		// the asset hub's reserve trust is static in its XCM config, so we create the (sufficient)
		// asset via a governance (root) `ForeignAssets::force_create` and wait for it to exist
		// before transferring.
		log::info!("Creating bridged foreign assets on both Asset Hubs");
		let owner_acc = dev_account(&alice);
		let create_wwnd = asset_hub_rococo::force_create_foreign_asset_call(
			&ahr,
			WESTEND_GENESIS_HASH,
			owner_acc.clone(),
			10_000_000_000,
		)
		.await?;
		relay_rococo::send_governance_transact(
			&rococo_relay,
			&alice,
			ASSET_HUB_PARA_ID,
			create_wwnd,
			5_000_000_000,
			100_000,
		)
		.await?;
		let create_wroc = asset_hub_westend::force_create_foreign_asset_call(
			&ahw,
			ROCOCO_GENESIS_HASH,
			owner_acc.clone(),
			10_000_000_000,
		)
		.await?;
		relay_westend::send_governance_transact(
			&westend_relay,
			&alice,
			ASSET_HUB_PARA_ID,
			create_wroc,
			5_000_000_000,
			100_000,
		)
		.await?;
		log::info!("force_create governance transacts submitted; waiting for the assets to exist");
		retry_until(Duration::from_secs(300), || {
			let ahr = ahr.clone();
			let owner_acc = owner_acc.clone();
			async move {
				Ok(asset_hub_rococo::bridged_asset_owner_is(&ahr, WESTEND_GENESIS_HASH, &owner_acc)
					.await?
					.then_some(()))
			}
		})
		.await?;
		log::info!("Rococo AH: bridged foreign asset created");
		retry_until(Duration::from_secs(300), || {
			let ahw = ahw.clone();
			let owner_acc = owner_acc.clone();
			async move {
				Ok(asset_hub_westend::bridged_asset_owner_is(&ahw, ROCOCO_GENESIS_HASH, &owner_acc)
					.await?
					.then_some(()))
			}
		})
		.await?;
		log::info!("Westend AH: bridged foreign asset created");

		// No asset-conversion pool / liquidity setup here. The bridged foreign asset is created as
		// `is_sufficient = true` (see `force_create_foreign_asset_call`), so at this runtime
		// revision it can pay its own XCM execution fees directly — there is no need to seed a
		// native<>bridged pool (and we couldn't anyway: nothing mints the bridged asset to a
		// local account before the first bridge transfer).

		// The bridge sovereign / reward accounts are pre-funded at genesis (see
		// `bridge_hub_balances_override`), so there is no post-spawn funding step here.

		log::info!("Bridge initialization complete");
		Ok(())
	}

	/// Initializes the GRANDPA bridge pallets and starts the finality, parachains and messages
	/// relayers.
	pub async fn start_relayer(&mut self) -> Result<(), anyhow::Error> {
		// Resolve the actual node WS endpoints. Ports are assigned dynamically by zombienet, so we
		// read each node's real URI and pass it to `substrate-relay`.
		let rococo_relay = self.rococo.get_node("alice-rococo-validator")?.ws_uri().to_string();
		let westend_relay = self.westend.get_node("alice-westend-validator")?.ws_uri().to_string();
		let bh_rococo = self.rococo.get_node("bridge-hub-rococo-collator1")?.ws_uri().to_string();
		let bh_westend =
			self.westend.get_node("bridge-hub-westend-collator1")?.ws_uri().to_string();

		// A freshly launched bridge hub produces best blocks quickly but reorgs heavily for the
		// first few dozen blocks (collators racing, finality lagging). The one-shot `init-bridge`
		// tx submitted during that window can land on a branch that is later orphaned and never
		// re-included/finalized, so the relayer waits out its long finality timeout and reports the
		// tx as `Lost`. Wait until each bridge hub is steadily finalizing (past the early-fork
		// window) before initializing the bridge.
		let bhr_client = Self::client_of(&self.rococo, "bridge-hub-rococo-collator1").await?;
		let bhw_client = Self::client_of(&self.westend, "bridge-hub-westend-collator1").await?;
		log::info!("Waiting for bridge hubs to finalize past the early-fork window before init");
		wait_for_finalized_height(&bhr_client, 40, Duration::from_secs(600)).await?;
		wait_for_finalized_height(&bhw_client, 40, Duration::from_secs(600)).await?;

		// init-bridge (idempotent), driven from finalized state until the GRANDPA pallet is
		// confirmed `Normal` at a finalized block (best-state can be a reorg victim).
		init_bridge_confirmed(
			&[
				"init-bridge",
				"westend-to-bridge-hub-rococo",
				"--source-uri",
				westend_relay.as_str(),
				"--source-version-mode",
				"Auto",
				"--target-uri",
				bh_rococo.as_str(),
				"--target-version-mode",
				"Auto",
				"--target-signer",
				"//Bob",
			],
			&bhr_client,
			"BridgeWestendGrandpa",
		)
		.await?;
		init_bridge_confirmed(
			&[
				"init-bridge",
				"rococo-to-bridge-hub-westend",
				"--source-uri",
				rococo_relay.as_str(),
				"--source-version-mode",
				"Auto",
				"--target-uri",
				bh_westend.as_str(),
				"--target-version-mode",
				"Auto",
				"--target-signer",
				"//Bob",
			],
			&bhw_client,
			"BridgeRococoGrandpa",
		)
		.await?;

		// Finality relayers (free relay-chain headers, signed by //Charlie).
		self._relayers.push(spawn_relayer(&[
			"relay-headers",
			"rococo-to-bridge-hub-westend",
			"--only-free-headers",
			"--source-uri",
			rococo_relay.as_str(),
			"--source-version-mode",
			"Auto",
			"--target-uri",
			bh_westend.as_str(),
			"--target-version-mode",
			"Auto",
			"--target-signer",
			"//Charlie",
			"--target-transactions-mortality",
			"1024",
		])?);
		self._relayers.push(spawn_relayer(&[
			"relay-headers",
			"westend-to-bridge-hub-rococo",
			"--only-free-headers",
			"--source-uri",
			westend_relay.as_str(),
			"--source-version-mode",
			"Auto",
			"--target-uri",
			bh_rococo.as_str(),
			"--target-version-mode",
			"Auto",
			"--target-signer",
			"//Charlie",
			"--target-transactions-mortality",
			"1024",
		])?);

		// Parachains relayers (free parachain headers, signed by //Dave). The `relay-parachains`
		// subcommand identifies the bridge by its Bridge Hub pair (`substrate-relay` >= v1.8.10).
		self._relayers.push(spawn_relayer(&[
			"relay-parachains",
			"bridge-hub-rococo-to-bridge-hub-westend",
			"--only-free-headers",
			"--source-uri",
			rococo_relay.as_str(),
			"--source-version-mode",
			"Auto",
			"--target-uri",
			bh_westend.as_str(),
			"--target-version-mode",
			"Auto",
			"--target-signer",
			"//Dave",
			"--target-transactions-mortality",
			"1024",
		])?);
		self._relayers.push(spawn_relayer(&[
			"relay-parachains",
			"bridge-hub-westend-to-bridge-hub-rococo",
			"--only-free-headers",
			"--source-uri",
			westend_relay.as_str(),
			"--source-version-mode",
			"Auto",
			"--target-uri",
			bh_rococo.as_str(),
			"--target-version-mode",
			"Auto",
			"--target-signer",
			"//Dave",
			"--target-transactions-mortality",
			"1024",
		])?);

		// Messages relayers (lane 0x00000002; //Eve for ro->wnd, //Ferdie for wnd->ro).
		self._relayers.push(spawn_relayer(&[
			"relay-messages",
			"bridge-hub-rococo-to-bridge-hub-westend",
			"--source-uri",
			bh_rococo.as_str(),
			"--source-version-mode",
			"Auto",
			"--source-signer",
			"//Eve",
			"--source-transactions-mortality",
			"1024",
			"--target-uri",
			bh_westend.as_str(),
			"--target-version-mode",
			"Auto",
			"--target-signer",
			"//Eve",
			"--target-transactions-mortality",
			"1024",
			"--lane",
			"00000002",
		])?);
		self._relayers.push(spawn_relayer(&[
			"relay-messages",
			"bridge-hub-westend-to-bridge-hub-rococo",
			"--source-uri",
			bh_westend.as_str(),
			"--source-version-mode",
			"Auto",
			"--source-signer",
			"//Ferdie",
			"--source-transactions-mortality",
			"1024",
			"--target-uri",
			bh_rococo.as_str(),
			"--target-version-mode",
			"Auto",
			"--target-signer",
			"//Ferdie",
			"--target-transactions-mortality",
			"1024",
			"--lane",
			"00000002",
		])?);

		log::info!("Waiting for the GRANDPA bridge pallets to be initialized");
		let bhr = self.bridge_hub_rococo_client().await?;
		let bhw = self.bridge_hub_westend_client().await?;
		retry_until(Duration::from_secs(400), || {
			let bhr = bhr.clone();
			async move {
				Ok(best_finalized_bridged_header(&bhr, "WestendFinalityApi")
					.await?
					.filter(|n| *n > 0)
					.map(|_| ()))
			}
		})
		.await?;
		retry_until(Duration::from_secs(400), || {
			let bhw = bhw.clone();
			async move {
				Ok(best_finalized_bridged_header(&bhw, "RococoFinalityApi")
					.await?
					.filter(|n| *n > 0)
					.map(|_| ()))
			}
		})
		.await?;
		log::info!("Relayer started and GRANDPA bridge pallets initialized");
		Ok(())
	}
}

/// Account id of a dev signer, as a `subxt` `AccountId32`.
pub fn dev_account(keypair: &Keypair) -> subxt::utils::AccountId32 {
	subxt::utils::AccountId32(keypair.public_key().0)
}

/// 32-byte public key of a dev signer.
pub fn dev_public(keypair: &Keypair) -> [u8; 32] {
	keypair.public_key().0
}

#[allow(dead_code)]
fn parse_account(ss58: &str) -> Result<subxt::utils::AccountId32, anyhow::Error> {
	ss58.parse::<subxt::utils::AccountId32>()
		.map_err(|e| anyhow!("invalid SS58 account {ss58}: {e:?}"))
}
