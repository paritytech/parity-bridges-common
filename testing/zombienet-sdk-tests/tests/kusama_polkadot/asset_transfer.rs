// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Asset transfer test for the local Kusama <> Polkadot bridge.
//!
//! A user transfers DOT from Polkadot Asset Hub to Kusama Asset Hub and back, and KSM from Kusama
//! Asset Hub to Polkadot Asset Hub and back. We assert that:
//!   * the wrapped assets arrive on the remote Asset Hub (at least 4.8 of 5 sent, after fees),
//!   * the message relayers (`//Eve` / `//Ferdie`) are rewarded,
//!   * the unwrapped (native) assets arrive back on the origin Asset Hub (at least 2.8 of 3), and
//!   * the finality/parachain relayers (`//Charlie` / `//Dave`) keep a constant balance, because
//!     their transactions are free.
//!
//! DOT has 10 decimals, KSM has 12, so the two directions use different unit constants.

use crate::{
	common::utils::wait_for_native_increase,
	kusama_polkadot::{
		assert_relayer_balances_unchanged, asset_hub_kusama, asset_hub_polkadot,
		bridge_hub_kusama_relayer_reward, bridge_hub_polkadot_relayer_reward, dev_account,
		dev_public, retry_until, BridgeTestEnv,
	},
};
use std::time::Duration;
use subxt_signer::sr25519::dev;

// DOT amounts (10 decimals).
const FIVE_DOT: u128 = 50_000_000_000;
const THREE_DOT: u128 = 30_000_000_000;
const MIN_WRAPPED_DOT: u128 = 48_000_000_000;
const MIN_NATIVE_DOT: u128 = 28_000_000_000;
// KSM amounts (12 decimals).
const FIVE_KSM: u128 = 5_000_000_000_000;
const THREE_KSM: u128 = 3_000_000_000_000;
const MIN_WRAPPED_KSM: u128 = 4_800_000_000_000;
const MIN_NATIVE_KSM: u128 = 2_800_000_000_000;

const MIN_RELAYER_REWARD: u128 = 1;

#[tokio::test(flavor = "multi_thread")]
async fn asset_transfer_works() -> Result<(), anyhow::Error> {
	let env = BridgeTestEnv::spawn(true, true).await?;

	let ahp = env.asset_hub_polkadot_client().await?;
	let ahk = env.asset_hub_kusama_client().await?;
	let bhp = env.bridge_hub_polkadot_client().await?;
	let bhk = env.bridge_hub_kusama_client().await?;

	let alice = dev::alice();
	let alice_pub = dev_public(&alice);
	let alice_acc = dev_account(&alice);
	let charlie = dev_public(&dev::charlie());
	let dave = dev_public(&dev::dave());

	// Relayer balances are constant (before the transfers).
	assert_relayer_balances_unchanged(&bhp, &bhk, charlie, dave).await?;

	let eve = dev_account(&dev::eve());
	let ferdie = dev_account(&dev::ferdie());

	// Phase 1: forward transfers, both directions concurrently (different chains => the shared
	// `//Alice` signer has no nonce contention).
	tokio::try_join!(
		async {
			// DOT is native to Polkadot AH, so Polkadot AH is the reserve.
			asset_hub_polkadot::transfer_assets(
				&ahp,
				&alice,
				alice_pub,
				asset_hub_polkadot::native_asset,
				FIVE_DOT,
				asset_hub_polkadot::TransferType::LocalReserve,
			)
			.await?;
			// //Alice receives at least 4.8 wrapped DOT on Kusama AH.
			retry_until(Duration::from_secs(600), || {
				let ahk = ahk.clone();
				let acc = alice_acc.clone();
				async move {
					let asset = asset_hub_kusama::bridged_asset();
					let balance = asset_hub_kusama::foreign_asset_balance(&ahk, asset, acc).await?;
					Ok(balance.filter(|b| *b > MIN_WRAPPED_DOT).map(|_| ()))
				}
			})
			.await?;
			// //Eve is rewarded on Kusama BH for delivering messages from Polkadot BH.
			retry_until(Duration::from_secs(300), || {
				let bhk = bhk.clone();
				let eve = eve.clone();
				async move {
					let reward = bridge_hub_kusama_relayer_reward(&bhk, eve).await?;
					Ok(reward.filter(|r| *r > MIN_RELAYER_REWARD).map(|_| ()))
				}
			})
			.await?;
			Ok::<(), anyhow::Error>(())
		},
		async {
			// KSM is native to Kusama AH, so Kusama AH is the reserve.
			asset_hub_kusama::transfer_assets(
				&ahk,
				&alice,
				alice_pub,
				asset_hub_kusama::native_asset,
				FIVE_KSM,
				asset_hub_kusama::TransferType::LocalReserve,
			)
			.await?;
			retry_until(Duration::from_secs(600), || {
				let ahp = ahp.clone();
				let acc = alice_acc.clone();
				async move {
					let asset = asset_hub_polkadot::bridged_asset();
					let balance =
						asset_hub_polkadot::foreign_asset_balance(&ahp, asset, acc).await?;
					Ok(balance.filter(|b| *b > MIN_WRAPPED_KSM).map(|_| ()))
				}
			})
			.await?;
			// //Ferdie is rewarded on Polkadot BH for delivering messages from Kusama BH.
			retry_until(Duration::from_secs(300), || {
				let bhp = bhp.clone();
				let ferdie = ferdie.clone();
				async move {
					let reward = bridge_hub_polkadot_relayer_reward(&bhp, ferdie).await?;
					Ok(reward.filter(|r| *r > MIN_RELAYER_REWARD).map(|_| ()))
				}
			})
			.await?;
			Ok::<(), anyhow::Error>(())
		},
	)?;

	// Phase 2: return (unwrap) 3 units each way, concurrently. The phase barrier ensures phase 1
	// delivered the wrapped asset (and keeps both legs off the same `//Alice` nonce per chain). The
	// wrapped asset's reserve is the destination AH, so both use `DestinationReserve`.
	tokio::try_join!(
		async {
			let initial_dot = asset_hub_polkadot::free_balance(&ahp, alice_pub).await?;
			asset_hub_kusama::transfer_assets(
				&ahk,
				&alice,
				alice_pub,
				asset_hub_kusama::bridged_asset,
				THREE_DOT,
				asset_hub_kusama::TransferType::DestinationReserve,
			)
			.await?;
			wait_for_native_increase(&ahp, alice_pub, initial_dot, MIN_NATIVE_DOT).await?;
			Ok::<(), anyhow::Error>(())
		},
		async {
			let initial_ksm = asset_hub_kusama::free_balance(&ahk, alice_pub).await?;
			asset_hub_polkadot::transfer_assets(
				&ahp,
				&alice,
				alice_pub,
				asset_hub_polkadot::bridged_asset,
				THREE_KSM,
				asset_hub_polkadot::TransferType::DestinationReserve,
			)
			.await?;
			wait_for_native_increase(&ahk, alice_pub, initial_ksm, MIN_NATIVE_KSM).await?;
			Ok::<(), anyhow::Error>(())
		},
	)?;

	// Relayer balances are still constant (after the transfers).
	assert_relayer_balances_unchanged(&bhp, &bhk, charlie, dave).await?;

	Ok(())
}
