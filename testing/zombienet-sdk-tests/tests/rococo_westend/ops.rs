// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Per-runtime typed operations for the Rococo <> Westend bridge.
//!
//! Rococo/Westend (relays) and the two Asset Hubs / Bridge Hubs share the same type layout for the
//! calls and storage we touch, but `subxt` generates a distinct module per runtime, so the types
//! are nominally different. The macros below generate the (identical) bodies once per runtime; they
//! are invoked in the parent module (`super`), so `super::{ASSET_HUB_PARA_ID, XCM_VERSION}` resolve
//! to its constants. The runtime-agnostic submission helpers come from `crate::common::utils`.

macro_rules! relay_ops {
	($name:ident, $relay:ident, $runtime:ident) => {
		pub mod $name {
			use crate::{
				common::utils::sign_submit_wait_in_block,
				$relay::runtime_types::{
					pallet_utility::pallet::Call as UtilityCall,
					pallet_xcm::pallet::Call as XcmPalletCall,
					polkadot_parachain_primitives::primitives::Id,
					polkadot_runtime_parachains::hrmp::pallet::Call as HrmpCall,
					sp_weights::weight_v2::Weight,
					staging_xcm::v4::{
						junction::Junction, junctions::Junctions, location::Location, Instruction,
						Xcm,
					},
					xcm::{
						double_encoded::DoubleEncoded,
						v3::{OriginKind, WeightLimit},
						VersionedLocation, VersionedXcm,
					},
					$runtime::RuntimeCall,
				},
			};
			use subxt::{OnlineClient, PolkadotConfig};
			use subxt_signer::sr25519::Keypair;

			/// Builds (does not submit) a `Hrmp::force_open_hrmp_channel(..)` runtime call.
			pub fn force_open_hrmp_channel_call(
				sender: u32,
				recipient: u32,
				max_capacity: u32,
				max_message_size: u32,
			) -> RuntimeCall {
				RuntimeCall::Hrmp(HrmpCall::force_open_hrmp_channel {
					sender: Id(sender),
					recipient: Id(recipient),
					max_capacity,
					max_message_size,
				})
			}

			/// Builds (does not submit) a `XcmPallet::send(..)` runtime call carrying an
			/// `UnpaidExecution` + `Transact{Superuser}` message to the given parachain — the
			/// governance primitive used to configure the system parachains.
			pub fn governance_transact_call(
				para_id: u32,
				encoded_call: Vec<u8>,
				require_weight_ref_time: u64,
				require_weight_proof_size: u64,
			) -> RuntimeCall {
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
				RuntimeCall::XcmPallet(XcmPalletCall::send {
					dest: Box::new(dest),
					message: Box::new(message),
				})
			}

			/// Submits `sudo(Utility::batch_all(calls))` and waits for in-block success. Lets the
			/// whole bridge-init governance for one relay land in a single extrinsic instead of
			/// one finalized round-trip per call; the on-parachain effects are confirmed
			/// separately by the `retry_until` checks in `init_bridge`.
			pub async fn sudo_batch_all(
				client: &OnlineClient<PolkadotConfig>,
				sudo: &Keypair,
				calls: Vec<RuntimeCall>,
			) -> Result<(), anyhow::Error> {
				let batch = RuntimeCall::Utility(UtilityCall::batch_all { calls });
				let tx = crate::$relay::tx().sudo().sudo(batch);
				sign_submit_wait_in_block(client, &tx, sudo).await
			}
		}
	};
}

macro_rules! asset_hub_ops {
	($name:ident, $ah:ident) => {
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
			use super::XCM_VERSION;
			use crate::{
				common::utils::{free_balance_at, sign_submit_wait},
				$bh::runtime_types::staging_xcm::v5::{
					junction::{Junction, NetworkId},
					junctions::Junctions,
					location::Location,
				},
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
