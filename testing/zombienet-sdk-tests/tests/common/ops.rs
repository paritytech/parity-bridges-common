// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Shared, per-runtime typed-operation macros used by every bridge pair.
//!
//! `subxt` generates a distinct module per runtime, so even where the calls/storage we touch share
//! a layout the types are nominally different. The macros below emit the identical bodies once per
//! runtime. They are `#[macro_use]`-exported from [`crate::common`], so a bridge pair invokes them
//! from its own `mod.rs`; inside the generated module `super::` therefore resolves to that pair's
//! module (which must define `ASSET_HUB_PARA_ID`).
//!
//! The bridged/remote consensus network differs per pair — Rococo/Westend identify each other by
//! genesis hash (`NetworkId::ByGenesis(..)`), Polkadot/Kusama by the named `NetworkId::Polkadot` /
//! `NetworkId::Kusama` variants. Each `asset_hub_ops!` / `bridge_hub_ops!` invocation therefore
//! passes the remote network as an expression (`$remote_network`), evaluated inside the generated
//! module where the per-runtime `NetworkId` is in scope.

macro_rules! asset_hub_ops {
	($name:ident, $ah:ident, $remote_network:expr) => {
		pub mod $name {
			use crate::common::utils::{
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

			/// The bridged/remote consensus network this Asset Hub bridges to.
			pub fn remote_network() -> NetworkId {
				$remote_network
			}

			/// The local native asset, `{ parents: 1, interior: Here }`.
			pub fn native_asset() -> Location {
				Location { parents: 1, interior: Junctions::Here }
			}

			/// The bridged native asset, `{ parents: 2, interior: X1(GlobalConsensus(remote)) }`.
			pub fn bridged_asset() -> Location {
				Location {
					parents: 2,
					interior: Junctions::X1([Junction::GlobalConsensus(remote_network())]),
				}
			}

			/// The remote Asset Hub location, `{ parents: 2, X2(GlobalConsensus, Parachain(1000))
			/// }`.
			pub fn remote_asset_hub() -> Location {
				Location {
					parents: 2,
					interior: Junctions::X2([
						Junction::GlobalConsensus(remote_network()),
						Junction::Parachain(super::ASSET_HUB_PARA_ID),
					]),
				}
			}

			/// `tx.assetConversion.createPool(native, bridged)`. The bridged asset is registered as
			/// `is_sufficient: false`, so it can only pay XCM fees by being swapped to the native
			/// token through an asset-conversion pool (the runtime's `SwapFirstAssetTrader`);
			/// `init_bridge` seeds one per Asset Hub. This is a regular signed call (no sudo).
			pub async fn create_pool(
				client: &OnlineClient<PolkadotConfig>,
				signer: &Keypair,
				nonce: u64,
			) -> Result<(), anyhow::Error> {
				let tx =
					crate::$ah::tx().asset_conversion().create_pool(native_asset(), bridged_asset());
				sign_submit_wait_in_block_nonce(client, &tx, signer, nonce).await
			}

			/// `tx.assetConversion.addLiquidity(native, bridged, ..)`. Seeds the pool created by
			/// [`create_pool`] with liquidity so the bridged asset's XCM fees can be swapped to
			/// native.
			pub async fn add_liquidity(
				client: &OnlineClient<PolkadotConfig>,
				signer: &Keypair,
				native_amount: u128,
				bridged_amount: u128,
				mint_to: subxt::utils::AccountId32,
				nonce: u64,
			) -> Result<(), anyhow::Error> {
				let tx = crate::$ah::tx().asset_conversion().add_liquidity(
					native_asset(),
					bridged_asset(),
					native_amount,
					bridged_amount,
					1,
					1,
					mint_to,
				);
				sign_submit_wait_in_block_nonce(client, &tx, signer, nonce).await
			}

			/// `transferAssetsUsingTypeAndThen` from this Asset Hub to the remote one: sends `amount`
			/// of `asset` to `beneficiary`, paying remote fees out of `asset`. The reserve can't be
			/// auto-detected across a consensus boundary, so the caller passes `transfer_type`
			/// (`LocalReserve` to send this Asset Hub's native token out, `DestinationReserve` to
			/// send a bridged token back to its origin).
			pub async fn transfer_assets(
				client: &OnlineClient<PolkadotConfig>,
				signer: &Keypair,
				beneficiary: [u8; 32],
				asset: impl Fn() -> Location,
				amount: u128,
				transfer_type: TransferType,
			) -> Result<(), anyhow::Error> {
				let dest = VersionedLocation::V5(remote_asset_hub());
				let assets = VersionedAssets::V5(Assets(vec![Asset {
					id: AssetId(asset()),
					fun: Fungibility::Fungible(amount),
				}]));
				// Rebuilt (subxt's `Location` isn't `Clone`) to also pay the remote fees in `asset`.
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
				// Same transfer type for assets and fees, rebuilt (the type isn't `Clone`).
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

			/// SCALE-encoded `ForeignAssets::force_create(bridged_asset, owner, is_sufficient,
			/// min_balance)` call, wrapped in a relay-chain governance `Transact` (root).
			#[allow(dead_code)]
			pub async fn force_create_foreign_asset_call(
				client: &OnlineClient<PolkadotConfig>,
				owner: subxt::utils::AccountId32,
				min_balance: u128,
			) -> Result<Vec<u8>, anyhow::Error> {
				let who = subxt::utils::MultiAddress::Id(owner);
				let call = crate::$ah::tx().foreign_assets().force_create(
					bridged_asset(),
					who,
					true,
					min_balance,
				);
				Ok(call.encode_call_data(&client.metadata())?)
			}

			/// Free balance of `account` (native asset) via `system.account`.
			#[allow(dead_code)]
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
			#[allow(dead_code)]
			pub async fn bridged_asset_owner_is(
				client: &OnlineClient<PolkadotConfig>,
				account: &subxt::utils::AccountId32,
			) -> Result<bool, anyhow::Error> {
				let addr = crate::$ah::storage().foreign_assets().asset(bridged_asset());
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
	($name:ident, $bh:ident, $remote_network:expr) => {
		pub mod $name {
			use crate::{
				common::utils::{free_balance_at, sign_submit_wait},
				$bh::runtime_types::staging_xcm::v5::{
					junction::{Junction, NetworkId},
					junctions::Junctions,
					location::Location,
				},
			};
			use subxt::{OnlineClient, PolkadotConfig};
			use subxt_signer::sr25519::Keypair;

			/// The bridged/remote consensus network this Bridge Hub bridges to.
			#[allow(dead_code)]
			pub fn remote_network() -> NetworkId {
				$remote_network
			}

			/// The remote Bridge Hub location, `{ parents: 2, X2(GlobalConsensus, Parachain) }`.
			#[allow(dead_code)]
			pub fn remote_bridge_hub(para: u32) -> Location {
				Location {
					parents: 2,
					interior: Junctions::X2([
						Junction::GlobalConsensus(remote_network()),
						Junction::Parachain(para),
					]),
				}
			}

			/// `tx.balances.transferAllowDeath(target, amount)`.
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
