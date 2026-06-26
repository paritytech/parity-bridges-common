// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Network spawning and bridge bootstrap for the Rococo <> Westend bridge.
//!
//! [`BridgeTestEnv`] spawns both relay-chain networks (each with a Bridge Hub and an Asset Hub
//! parachain), initializes the bridge (HRMP channels, remote XCM versions and bridged foreign
//! assets) and drives the `substrate-relay` binary as a set of subprocesses. The generic `subxt`
//! helpers, relayer driver and node images come from [`crate::common`]; the per-runtime typed
//! operations from [`super`].

use anyhow::anyhow;
use std::time::Duration;
use subxt::{OnlineClient, PolkadotConfig};
use subxt_signer::sr25519::dev;
use zombienet_sdk::{
	environment::get_spawn_fn, Arg, LocalFileSystem, Network, NetworkConfig, NetworkConfigBuilder,
};

use super::{
	asset_hub_rococo, asset_hub_westend, bridge_hub_rococo, bridge_hub_westend, relay_rococo,
	relay_westend, ASSET_HUB_PARA_ID, ASSET_HUB_SOVEREIGN_AT_BRIDGE_HUB, BHR_LANE_BRIDGED_CHAIN,
	BHR_LANE_THIS_CHAIN, BHW_LANE_BRIDGED_CHAIN, BHW_LANE_THIS_CHAIN, BRIDGE_HUB_ROCOCO_PARA_ID,
	BRIDGE_HUB_WESTEND_PARA_ID, ROCOCO_GENESIS_HASH, SOVEREIGN_FUNDING, WESTEND_GENESIS_HASH,
	XCM_VERSION,
};
use crate::common::{
	images::node_images,
	relayer::{init_bridge_confirmed, spawn_relayer, Relayer},
	utils::{best_finalized_bridged_header, dev_account, retry_until, wait_for_finalized_height},
};

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
				// Single slot-based collator (see `rococo_network_config`): the test drives one
				// endpoint and one builder avoids fork competition.
				.with_collator(|n| {
					n.with_name("asset-hub-westend-collator1").with_args(ah_args.clone())
				})
		})
		.with_global_settings(global_settings)
		.build()
		.map_err(config_errs)
}

/// Shared global settings for both networks. We disable `tear_down_on_failure` so the background
/// node-monitoring task (which declares a node "crashed" if its metrics endpoint does not respond
/// within `~5s`) does not tear the network down on a transient, load-induced timeout — the same
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
			async {
				spawn_fn(rococo_network_config()?)
					.await
					.map_err(|e| anyhow!("Rococo spawn failed: {e}"))
			},
			async {
				spawn_fn(westend_network_config()?)
					.await
					.map_err(|e| anyhow!("Westend spawn failed: {e}"))
			},
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
		let owner_acc = dev_account(&alice);

		// Wait until each bridge hub has finalized its first block: proof the full
		// collation -> backing -> inclusion -> finality pipeline is live (a stronger and faster
		// signal than N best blocks). The asset hubs only need their RPC up to build the calls
		// below (already guaranteed by `wait_client`), and their on-chain effects are confirmed
		// by the `retry_until` checks further down.
		log::info!("Waiting for bridge hubs to finalize their first block");
		tokio::try_join!(
			wait_for_finalized_height(&bhr, 1, Duration::from_secs(300)),
			wait_for_finalized_height(&bhw, 1, Duration::from_secs(300)),
		)?;

		// Build the full bridge-init governance for each relay as a single `sudo(batch_all(..))`:
		// HRMP channel opens + remote XCM versions + bridged foreign-asset creation. The two relays
		// are independent chains, so both batches are submitted concurrently. (The on-parachain XCM
		// effects are async and confirmed by the `retry_until` checks below.)
		log::info!("Submitting batched bridge-init governance to both relays");
		let ahw_on_ahr = asset_hub_rococo::remote_asset_hub(WESTEND_GENESIS_HASH);
		let force_ahw =
			asset_hub_rococo::force_xcm_version_call(&ahr, ahw_on_ahr, XCM_VERSION).await?;
		let bhw_on_bhr =
			bridge_hub_rococo::remote_bridge_hub(WESTEND_GENESIS_HASH, BRIDGE_HUB_WESTEND_PARA_ID);
		let force_bhw = bridge_hub_rococo::force_xcm_version_call(&bhr, bhw_on_bhr).await?;
		let create_wwnd = asset_hub_rococo::force_create_foreign_asset_call(
			&ahr,
			WESTEND_GENESIS_HASH,
			owner_acc.clone(),
			10_000_000_000,
		)
		.await?;
		let rococo_calls = vec![
			relay_rococo::force_open_hrmp_channel_call(
				ASSET_HUB_PARA_ID,
				BRIDGE_HUB_ROCOCO_PARA_ID,
				4,
				524288,
			),
			relay_rococo::force_open_hrmp_channel_call(
				BRIDGE_HUB_ROCOCO_PARA_ID,
				ASSET_HUB_PARA_ID,
				4,
				524288,
			),
			relay_rococo::governance_transact_call(
				ASSET_HUB_PARA_ID,
				force_ahw,
				200_000_000,
				12_000,
			),
			relay_rococo::governance_transact_call(
				BRIDGE_HUB_ROCOCO_PARA_ID,
				force_bhw,
				200_000_000,
				12_000,
			),
			relay_rococo::governance_transact_call(
				ASSET_HUB_PARA_ID,
				create_wwnd,
				5_000_000_000,
				100_000,
			),
		];

		let ahr_on_ahw = asset_hub_westend::remote_asset_hub(ROCOCO_GENESIS_HASH);
		let force_ahr =
			asset_hub_westend::force_xcm_version_call(&ahw, ahr_on_ahw, XCM_VERSION).await?;
		let bhr_on_bhw =
			bridge_hub_westend::remote_bridge_hub(ROCOCO_GENESIS_HASH, BRIDGE_HUB_ROCOCO_PARA_ID);
		let force_bhr = bridge_hub_westend::force_xcm_version_call(&bhw, bhr_on_bhw).await?;
		let create_wroc = asset_hub_westend::force_create_foreign_asset_call(
			&ahw,
			ROCOCO_GENESIS_HASH,
			owner_acc.clone(),
			10_000_000_000,
		)
		.await?;
		let westend_calls = vec![
			relay_westend::force_open_hrmp_channel_call(
				ASSET_HUB_PARA_ID,
				BRIDGE_HUB_WESTEND_PARA_ID,
				4,
				524288,
			),
			relay_westend::force_open_hrmp_channel_call(
				BRIDGE_HUB_WESTEND_PARA_ID,
				ASSET_HUB_PARA_ID,
				4,
				524288,
			),
			relay_westend::governance_transact_call(
				ASSET_HUB_PARA_ID,
				force_ahr,
				200_000_000,
				12_000,
			),
			relay_westend::governance_transact_call(
				BRIDGE_HUB_WESTEND_PARA_ID,
				force_bhr,
				200_000_000,
				12_000,
			),
			relay_westend::governance_transact_call(
				ASSET_HUB_PARA_ID,
				create_wroc,
				5_000_000_000,
				100_000,
			),
		];

		tokio::try_join!(
			relay_rococo::sudo_batch_all(&rococo_relay, &alice, rococo_calls),
			relay_westend::sudo_batch_all(&westend_relay, &alice, westend_calls),
		)?;

		// Confirm the on-parachain effects of the batches (both Asset Hubs concurrently): HRMP
		// egress channels open, and the bridged foreign assets exist with the expected owner.
		log::info!("Waiting for HRMP channels to open and bridged foreign assets to be created");
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
			retry_until(Duration::from_secs(300), || {
				let ahr = ahr.clone();
				let owner_acc = owner_acc.clone();
				async move {
					Ok(asset_hub_rococo::bridged_asset_owner_is(
						&ahr,
						WESTEND_GENESIS_HASH,
						&owner_acc,
					)
					.await?
					.then_some(()))
				}
			}),
			retry_until(Duration::from_secs(300), || {
				let ahw = ahw.clone();
				let owner_acc = owner_acc.clone();
				async move {
					Ok(asset_hub_westend::bridged_asset_owner_is(
						&ahw,
						ROCOCO_GENESIS_HASH,
						&owner_acc,
					)
					.await?
					.then_some(()))
				}
			}),
		)?;
		log::info!("HRMP channels open and bridged foreign assets created on both Asset Hubs");

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

		// A freshly launched bridge hub reorgs heavily for the first few dozen blocks (collators
		// racing, finality lagging), so a one-shot `init-bridge` tx submitted during that window
		// can be orphaned. Rather than pre-waiting for a fixed finalized height before init, we
		// drive `init-bridge` directly and wait (bounded, inside `init_bridge_confirmed`) for the
		// real init signal — `operating_mode == Normal` at a finalized block — re-submitting
		// until it holds. Early orphaned attempts are simply retried.
		let bhr_client = Self::client_of(&self.rococo, "bridge-hub-rococo-collator1").await?;
		let bhw_client = Self::client_of(&self.westend, "bridge-hub-westend-collator1").await?;

		// init-bridge (idempotent), driven from finalized state until the GRANDPA pallet is
		// confirmed `Normal` at a finalized block (best-state can be a reorg victim). The two
		// directions target different bridge hubs and are independent, so initialize them
		// concurrently. (Args are bound to locals so they outlive the concurrently-awaited
		// futures that borrow them.)
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
			"//Bob",
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
			"//Bob",
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
