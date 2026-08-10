// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Network spawning and bridge bootstrap for the Rococo <> Westend bridge.
//!
//! [`BridgeTestEnv`] spawns both relay-chain networks (each with a Bridge Hub and an Asset Hub
//! parachain) from the polkadot-sdk `*-debug` docker images and drives the `substrate-relay` binary
//! as a set of subprocesses. Like the Kusama <> Polkadot environment, the bridge is bootstrapped
//! **from genesis** rather than via `sudo`-signed governance: the local + remote XCM versions
//! (`polkadotXcm.safeXcmVersion` / `supportedVersion`), the bridge-GRANDPA pallet owner (`//Alice`,
//! so `init-bridge` is owner-signed), the bridged-asset reserves and the funded sovereign/reward
//! accounts are all set at genesis (see the `*_genesis_override` helpers). Only the HRMP channels
//! (opened post-spawn via the permissionless `Hrmp::establish_system_channel`) and the
//! `//Bob`-signed asset-conversion pools are set up after spawn. The generic `subxt` helpers,
//! relayer driver and node images come from [`crate::common`]; the per-runtime typed operations
//! from [`super`].

use std::time::Duration;
use subxt::{OnlineClient, PolkadotConfig};
use subxt_signer::sr25519::dev;
use zombienet_sdk::{
	environment::get_spawn_fn, Arg, LocalFileSystem, Network, NetworkConfig, NetworkConfigBuilder,
};

use super::{
	asset_hub_rococo, asset_hub_westend, ASSET_HUB_PARA_ID, ASSET_HUB_SOVEREIGN_AT_BRIDGE_HUB,
	BHR_LANE_BRIDGED_CHAIN, BHR_LANE_THIS_CHAIN, BHW_LANE_BRIDGED_CHAIN, BHW_LANE_THIS_CHAIN,
	BRIDGE_HUB_ROCOCO_PARA_ID, BRIDGE_HUB_WESTEND_PARA_ID, ENDOWMENT, ROCOCO_GENESIS_HASH,
	SOVEREIGN_FUNDING, WESTEND_GENESIS_HASH, XCM_VERSION,
};
use crate::common::{
	images::node_images,
	relayer::{init_bridge_confirmed, spawn_relayer, Relayer},
	utils::{
		best_finalized_bridged_header, bridge_hub_balances, config_errs, dev_account,
		global_settings, retry_until, sign_submit_wait, spawn_with_retry,
		wait_for_finalized_height,
	},
};

/// The full Rococo <> Westend bridge environment: both networks plus any running relayer
/// processes (kept alive for the lifetime of the value).
pub struct BridgeTestEnv {
	pub rococo: Network<LocalFileSystem>,
	pub westend: Network<LocalFileSystem>,
	_relayers: Vec<Relayer>,
}

/// Relay-chain genesis override for the async-backing params both networks need.
///
/// `allowed_ancestry_len: 2` (presets ship `0`, latest-only): asset-hub-westend authors with
/// `RELAY_PARENT_OFFSET = 1`, i.e. on a relay parent one block behind best, which the relay must
/// accept. `max_candidate_depth: 1` keeps the bridge-hub unincluded segment shallow so a
/// fast-runtime relay reorg strands few already-authored parablocks; empirically it lost the fewest
/// relayer proof txs to reorgs (vs deeper or shallower depths).
fn relay_async_backing_override() -> serde_json::Value {
	serde_json::json!({
		"configuration": { "config": {
			"async_backing_params": { "max_candidate_depth": 1, "allowed_ancestry_len": 2 }
		} }
	})
}

/// `GlobalConsensus` body for a network identified by genesis hash: `{ "ByGenesis": <hash> }`.
/// Rococo and Westend identify each other this way (not by a named `NetworkId` variant), so the
/// genesis JSON encodes the remote network as `ByGenesis` rather than a bare string like the
/// Kusama <> Polkadot environment.
fn by_genesis(genesis_hash: [u8; 32]) -> serde_json::Value {
	serde_json::json!({ "ByGenesis": genesis_hash })
}

/// Genesis `supportedVersion` entry recording [`XCM_VERSION`] for the remote bridged
/// `(parents: 2, GlobalConsensus(remote_network) / Parachain(para_id))` — the sudo-free equivalent
/// of `force_xcm_version`, without which the bridged transfer fails with `SendFailure`.
fn supported_remote_version(remote_network: &serde_json::Value, para_id: u32) -> serde_json::Value {
	serde_json::json!([[
		{ "parents": 2, "interior": { "X2": [
			{ "GlobalConsensus": remote_network },
			{ "Parachain": para_id },
		] } },
		XCM_VERSION,
	]])
}

/// Bridge Hub genesis override: fund the sovereign/reward accounts, make `//Alice` the
/// with-bridged-chain GRANDPA pallet owner (so `init-bridge` is owner-signed, no `sudo`), and set
/// the local safe + remote Bridge Hub XCM versions. `grandpa_pallet` is that pallet's camelCased
/// genesis key (`bridgeWestendGrandpa` on Rococo BH, `bridgeRococoGrandpa` on Westend BH).
///
/// The sudo-free replacement for the relay-chain governance batch this environment used to submit:
/// the BH sovereign/reward funding, the GRANDPA owner (was implicit in `//Bob`-signed
/// `init-bridge`) and the remote Bridge Hub `force_xcm_version` are all pinned here at genesis
/// instead.
fn bridge_hub_genesis_override(
	sovereign_accounts: &[&str],
	grandpa_pallet: &str,
	remote_network: &serde_json::Value,
	remote_bridge_hub_para_id: u32,
) -> serde_json::Value {
	let alice = dev_account(&dev::alice()).to_string();
	// `ENDOWMENT` — matched by `assert_relayer_balances_unchanged`.
	let mut value = bridge_hub_balances(ENDOWMENT, sovereign_accounts, SOVEREIGN_FUNDING);
	let obj = value.as_object_mut().expect("object");
	obj.insert(grandpa_pallet.to_string(), serde_json::json!({ "owner": alice }));
	obj.insert(
		"polkadotXcm".to_string(),
		serde_json::json!({
			"safeXcmVersion": XCM_VERSION,
			"supportedVersion": supported_remote_version(remote_network, remote_bridge_hub_para_id),
		}),
	);
	value
}

/// Asset Hub genesis override: set the local safe XCM version and the remote Asset Hub's supported
/// version, plus (optionally) the trusted reserves for the bridged foreign asset. The XCM versions
/// are the sudo-free stand-in for the `force_xcm_version` governance transacts the environment used
/// to submit. Asset Hub Westend also needs the reserve seeded (see [`asset_hub_westend_reserves`]);
/// Asset Hub Rococo uses a static bridging reserve, so it passes `None`.
fn asset_hub_genesis_override(
	remote_network: &serde_json::Value,
	reserves: Option<serde_json::Value>,
) -> serde_json::Value {
	let mut value = serde_json::json!({
		"polkadotXcm": {
			"safeXcmVersion": XCM_VERSION,
			"supportedVersion": supported_remote_version(remote_network, ASSET_HUB_PARA_ID),
		},
	});
	if let Some(reserves) = reserves {
		value
			.as_object_mut()
			.expect("object")
			.insert("foreignAssets".to_string(), serde_json::json!({ "reserves": reserves }));
	}
	value
}

/// Trusted-reserve genesis entry for the bridged ROC foreign asset that the Asset Hub Westend
/// runtime pre-registers at genesis.
///
/// Asset Hub Westend trusts XCM reserves only via per-asset `pallet-assets` `Reserves` (no static
/// bridging fallback), and that storage is normally populated by a runtime-upgrade migration —
/// which a freshly-genesis'd chain never runs. So the pre-registered bridged ROC asset would have
/// no reserve and inbound bridged reserve-transfers fail with `UntrustedReserveLocation`. Seed it
/// here to match the migration's rule (a Rococo-ecosystem asset's reserve is Asset Hub Rococo,
/// non-teleportable). `with_genesis_overrides` deep-merges into `foreignAssets`, so the preset's
/// `assets`/`accounts` are preserved.
fn asset_hub_westend_reserves() -> serde_json::Value {
	// The bridged ROC asset id `{ parents: 2, X1(GlobalConsensus(Rococo)) }` and its trusted
	// reserve, Asset Hub Rococo `{ parents: 2, X2(GlobalConsensus(Rococo),
	// Parachain(ASSET_HUB_PARA_ID)) }`.
	let bridged_roc = serde_json::json!({
		"parents": 2,
		"interior": { "X1": [{ "GlobalConsensus": by_genesis(ROCOCO_GENESIS_HASH) }] },
	});
	let asset_hub_rococo = serde_json::json!({
		"parents": 2,
		"interior": { "X2": [
			{ "GlobalConsensus": by_genesis(ROCOCO_GENESIS_HASH) },
			{ "Parachain": ASSET_HUB_PARA_ID },
		] },
	});
	serde_json::json!([[bridged_roc, [{ "reserve": asset_hub_rococo, "teleportable": false }]]])
}

fn rococo_network_config() -> Result<NetworkConfig, anyhow::Error> {
	let images = node_images();
	// Bridge hubs keep the default fork-aware tx pool (re-validates pending txs across reorgs, so
	// the relayer's proof txs survive relay-parent reorgs) and author slot-based, which paces
	// block production to the relay's backing instead of over-producing an unincluded segment that
	// a fast-runtime relay reorg would discard (losing in-flight proof txs).
	let bh_args: Vec<Arg> = vec![
		"-lparachain=info,runtime::bridge=trace,xcm=debug,txpool=debug".into(),
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
				// Bootstrap the bridge from genesis rather than via post-spawn governance: fund
				// the sovereign/reward accounts (a freshly launched bridge hub reorgs at the tip
				// and would invalidate post-spawn txs), make `//Alice` the `BridgeWestendGrandpa`
				// owner so `init-bridge` is owner-signed, and set the local + remote Bridge Hub
				// XCM versions.
				.with_genesis_overrides(bridge_hub_genesis_override(
					&[
						ASSET_HUB_SOVEREIGN_AT_BRIDGE_HUB,
						BHR_LANE_THIS_CHAIN,
						BHR_LANE_BRIDGED_CHAIN,
					],
					"bridgeWestendGrandpa",
					&by_genesis(WESTEND_GENESIS_HASH),
					BRIDGE_HUB_WESTEND_PARA_ID,
				))
				// A single bridge-hub collator: a second one fork-wars, retracting the block
				// carrying the relayer's finality/parachain-head update before it finalizes.
				// One builds linearly.
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
				// Set the local + remote Asset Hub XCM versions at genesis (Asset Hub Rococo uses
				// a static bridging reserve, so no `foreignAssets.reserves` override is needed).
				.with_genesis_overrides(asset_hub_genesis_override(
					&by_genesis(WESTEND_GENESIS_HASH),
					None,
				))
				// Single asset-hub collator: the test only drives one collator endpoint, and one
				// builder avoids fork competition (matching the bridge-hub choice).
				.with_collator(|n| {
					n.with_name("asset-hub-rococo-collator1").with_args(ah_args.clone())
				})
		})
		.with_global_settings(global_settings)
		.build()
		.map_err(config_errs)
}

fn westend_network_config() -> Result<NetworkConfig, anyhow::Error> {
	let images = node_images();
	// Fork-aware pool + slot-based authoring, see `rococo_network_config`.
	let bh_args: Vec<Arg> = vec![
		"-lparachain=info,runtime::bridge=trace,xcm=debug,txpool=debug".into(),
		"--authoring".into(),
		"slot-based".into(),
	];
	// The asset-hub-westend runtime must author slot-based (it panics under default/lookahead
	// authoring — the block lacks the expected relay-parent descendants).
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
				// Bootstrap from genesis (see `rococo_network_config`): fund the sovereign/reward
				// accounts, make `//Alice` the `BridgeRococoGrandpa` owner, and set the local +
				// remote Bridge Hub XCM versions.
				.with_genesis_overrides(bridge_hub_genesis_override(
					&[
						ASSET_HUB_SOVEREIGN_AT_BRIDGE_HUB,
						BHW_LANE_THIS_CHAIN,
						BHW_LANE_BRIDGED_CHAIN,
					],
					"bridgeRococoGrandpa",
					&by_genesis(ROCOCO_GENESIS_HASH),
					BRIDGE_HUB_ROCOCO_PARA_ID,
				))
				// Single bridge-hub collator, see `rococo_network_config`.
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
				// Set the local + remote Asset Hub XCM versions and seed the bridged ROC reserve
				// (Asset Hub Westend has no static bridging reserve fallback).
				.with_genesis_overrides(asset_hub_genesis_override(
					&by_genesis(ROCOCO_GENESIS_HASH),
					Some(asset_hub_westend_reserves()),
				))
				// Single asset-hub collator, see `rococo_network_config`.
				.with_collator(|n| {
					n.with_name("asset-hub-westend-collator1").with_args(ah_args.clone())
				})
		})
		.with_global_settings(global_settings)
		.build()
		.map_err(config_errs)
}

impl BridgeTestEnv {
	/// Spawns both networks and, depending on the flags, initializes the bridge and starts the
	/// relayer.
	pub async fn spawn(init: bool, start_relayer: bool) -> Result<Self, anyhow::Error> {
		let _ = env_logger::try_init_from_env(
			env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
		);

		// Independent networks: spawn concurrently (saves ~35s vs serial).
		log::info!("Spawning Rococo and Westend networks concurrently");
		let (rococo, westend) = tokio::try_join!(
			spawn_with_retry(get_spawn_fn(), rococo_network_config, "Rococo"),
			spawn_with_retry(get_spawn_fn(), westend_network_config, "Westend"),
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

	/// Opens the Asset Hub <-> Bridge Hub HRMP channels (both directions) on the **Rococo** relay
	/// via the permissionless `Hrmp::establish_system_channel` — both endpoints are system
	/// parachains, so no `sudo`/root is needed. Signed by `//Alice`; each direction waits for
	/// finalized success before the next (shared signer nonce).
	async fn open_hrmp_channels_rococo(&self) -> Result<(), anyhow::Error> {
		use crate::rococo::runtime_types::polkadot_parachain_primitives::primitives::Id;
		let relay = self.rococo_relay_client().await?;
		let alice = dev::alice();
		for (sender, recipient) in [
			(ASSET_HUB_PARA_ID, BRIDGE_HUB_ROCOCO_PARA_ID),
			(BRIDGE_HUB_ROCOCO_PARA_ID, ASSET_HUB_PARA_ID),
		] {
			let tx = crate::rococo::tx().hrmp().establish_system_channel(Id(sender), Id(recipient));
			sign_submit_wait(&relay, &tx, &alice).await?;
		}
		Ok(())
	}

	/// The Westend-side counterpart of [`Self::open_hrmp_channels_rococo`].
	async fn open_hrmp_channels_westend(&self) -> Result<(), anyhow::Error> {
		use crate::westend::runtime_types::polkadot_parachain_primitives::primitives::Id;
		let relay = self.westend_relay_client().await?;
		let alice = dev::alice();
		for (sender, recipient) in [
			(ASSET_HUB_PARA_ID, BRIDGE_HUB_WESTEND_PARA_ID),
			(BRIDGE_HUB_WESTEND_PARA_ID, ASSET_HUB_PARA_ID),
		] {
			let tx =
				crate::westend::tx().hrmp().establish_system_channel(Id(sender), Id(recipient));
			sign_submit_wait(&relay, &tx, &alice).await?;
		}
		Ok(())
	}

	/// Initializes both sides of the bridge: waits for block production, opens the HRMP channels
	/// and seeds the asset-conversion pools. The remote XCM versions, GRANDPA pallet owner and
	/// bridged-asset reserves are all set at genesis (see the `*_genesis_override` helpers), so —
	/// unlike before — there is no `sudo`-signed governance to submit here.
	async fn init_bridge(&self) -> Result<(), anyhow::Error> {
		let ahr = self.asset_hub_rococo_client().await?;
		let ahw = self.asset_hub_westend_client().await?;
		let bhr = self.bridge_hub_rococo_client().await?;
		let bhw = self.bridge_hub_westend_client().await?;

		// Wait until each bridge hub finalizes its first block: proof the full
		// collation -> backing -> inclusion -> finality pipeline is live. The asset hubs only need
		// their RPC up (via `wait_client`); their effects are confirmed by the `retry_until`s
		// below.
		log::info!("Waiting for bridge hubs to finalize their first block");
		tokio::try_join!(
			wait_for_finalized_height(&bhr, 1, Duration::from_secs(300)),
			wait_for_finalized_height(&bhw, 1, Duration::from_secs(300)),
		)?;

		// Open the HRMP channels post-spawn, now that the Bridge Hubs produce blocks, via the
		// permissionless `Hrmp::establish_system_channel` (both endpoints are system parachains, so
		// no `sudo`/root). Independent relays, so open both concurrently.
		log::info!("Opening HRMP channels between Asset Hub and Bridge Hub on both relays");
		tokio::try_join!(self.open_hrmp_channels_rococo(), self.open_hrmp_channels_westend())?;

		// Confirm both HRMP egress channels towards the Bridge Hubs are open (both Asset Hubs
		// concurrently). The bridged foreign assets are pre-registered at genesis, so there is
		// nothing to create or confirm for them here.
		log::info!("Waiting for HRMP channels to open");
		tokio::try_join!(
			retry_until(Duration::from_secs(600), || {
				let ahr = ahr.clone();
				async move {
					Ok(asset_hub_rococo::hrmp_egress_open(&ahr, BRIDGE_HUB_ROCOCO_PARA_ID)
						.await?
						.then_some(()))
				}
			}),
			retry_until(Duration::from_secs(600), || {
				let ahw = ahw.clone();
				async move {
					Ok(asset_hub_westend::hrmp_egress_open(&ahw, BRIDGE_HUB_WESTEND_PARA_ID)
						.await?
						.then_some(()))
				}
			}),
		)?;
		log::info!("HRMP channels open on both Asset Hubs");

		// The bridged foreign assets are pre-registered at genesis, owned by the bridged network's
		// sovereign account and with `is_sufficient: false`, so they can't pay XCM fees directly:
		// the runtime's `SwapFirstAssetTrader` swaps a non-native fee asset to the native token
		// through an asset-conversion pool, and none exists at genesis. Seed a native<>bridged
		// pool on each Asset Hub so the forward transfers (which pay destination fees in the
		// arriving bridged asset) succeed. Genesis endows `//Bob` with both the bridged asset and
		// the native token, so Bob funds both sides. Independent chains => seed both
		// concurrently. Sovereign/reward accounts are funded via genesis (see
		// `bridge_hub_balances`), so nothing else to fund here.
		const POOL_LIQUIDITY: u128 = 100_000_000_000_000;
		let bob = dev::bob();
		let bob_acc = dev_account(&bob);
		log::info!("Seeding native<>bridged asset-conversion pools on both Asset Hubs");
		tokio::try_join!(
			async {
				asset_hub_rococo::create_pool(&ahr, &bob, 0).await?;
				asset_hub_rococo::add_liquidity(
					&ahr,
					&bob,
					POOL_LIQUIDITY,
					POOL_LIQUIDITY,
					bob_acc.clone(),
					1,
				)
				.await
			},
			async {
				asset_hub_westend::create_pool(&ahw, &bob, 0).await?;
				asset_hub_westend::add_liquidity(
					&ahw,
					&bob,
					POOL_LIQUIDITY,
					POOL_LIQUIDITY,
					bob_acc.clone(),
					1,
				)
				.await
			},
		)?;
		log::info!("Asset-conversion pools created on both Asset Hubs");

		log::info!("Bridge initialization complete");
		Ok(())
	}

	/// Initializes the GRANDPA bridge pallets (owner-signed by the genesis owner `//Alice`, no
	/// `sudo`) and starts the finality, parachains and messages relayers.
	pub async fn start_relayer(&mut self) -> Result<(), anyhow::Error> {
		// Resolve the actual node WS endpoints. Ports are assigned dynamically by zombienet, so we
		// read each node's real URI and pass it to `substrate-relay`.
		let rococo_relay = self.rococo.get_node("alice-rococo-validator")?.ws_uri().to_string();
		let westend_relay = self.westend.get_node("alice-westend-validator")?.ws_uri().to_string();
		let bh_rococo = self.rococo.get_node("bridge-hub-rococo-collator1")?.ws_uri().to_string();
		let bh_westend =
			self.westend.get_node("bridge-hub-westend-collator1")?.ws_uri().to_string();

		let bhr_client = Self::client_of(&self.rococo, "bridge-hub-rococo-collator1").await?;
		let bhw_client = Self::client_of(&self.westend, "bridge-hub-westend-collator1").await?;

		// init-bridge each direction, signed by the genesis GRANDPA owner `//Alice` (no `sudo`),
		// retried until confirmed at a finalized block (see `init_bridge_confirmed`). Independent
		// bridge hubs, so run both concurrently; args are bound to locals so they outlive the
		// awaited futures that borrow them.
		let init_bhr_args = [
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
			"//Alice",
		];
		let init_bhw_args = [
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
			"//Alice",
		];
		tokio::try_join!(
			init_bridge_confirmed(&init_bhr_args, &bhr_client, "BridgeWestendGrandpa"),
			init_bridge_confirmed(&init_bhw_args, &bhw_client, "BridgeRococoGrandpa"),
		)?;

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
		// subcommand identifies the bridge by its Bridge Hub pair.
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
