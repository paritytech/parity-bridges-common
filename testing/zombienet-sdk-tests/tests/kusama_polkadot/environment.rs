// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Network spawning and bridge bootstrap for the local Kusama <> Polkadot bridge.
//!
//! Unlike the Rococo <> Westend environment (which runs polkadot-sdk docker images and configures
//! the bridge via `sudo`-signed governance), the Polkadot/Kusama runtimes live in the
//! `polkadot-fellows-runtimes` repo and have **no `sudo` pallet**. So this environment:
//!   * uses the **native** zombienet provider with the locally built `polkadot` /
//!     `polkadot-parachain` binaries and the fellows `chain-spec-generator` (paths from env vars,
//!     defaulting to `~/local_bridge_testing/bin`), and
//!   * bootstraps the bridge **entirely from genesis** — HRMP channels (relay `hrmp` preopen), the
//!     remote XCM version (`polkadotXcm.safeXcmVersion`), the bridge-GRANDPA pallet owner
//!     (`//Alice`, so `init-bridge` can be owner-signed without `sudo`) and the funded
//!     sovereign/reward accounts (Bridge Hub `balances`) — leaving only the non-`sudo`, `//Bob`-
//!     signed asset-conversion pools to be created post-spawn.
//!
//! The bridged foreign asset (wKSM on Asset Hub Polkadot, wDOT on Asset Hub Kusama) and `//Bob`'s
//! balance of it are already pre-registered by the fellows Asset Hub genesis presets, so nothing
//! needs to create them here.

use anyhow::anyhow;
use std::{path::PathBuf, time::Duration};
use subxt::{OnlineClient, PolkadotConfig};
use subxt_signer::sr25519::dev;
use zombienet_sdk::{
	environment::Provider, Arg, LocalFileSystem, Network, NetworkConfig, NetworkConfigBuilder,
};

use super::{
	asset_hub_kusama, asset_hub_polkadot, ASSET_HUB_PARA_ID,
	ASSET_HUB_SOVEREIGN_AT_BRIDGE_HUB_KUSAMA, ASSET_HUB_SOVEREIGN_AT_BRIDGE_HUB_POLKADOT,
	BHK_LANE_BRIDGED_CHAIN, BHK_LANE_THIS_CHAIN, BHP_LANE_BRIDGED_CHAIN, BHP_LANE_THIS_CHAIN,
	BRIDGE_HUB_KUSAMA_PARA_ID, BRIDGE_HUB_POLKADOT_PARA_ID, KUSAMA_UNIT, POLKADOT_UNIT,
	SOVEREIGN_FUNDING, XCM_VERSION,
};
use crate::common::{
	relayer::{init_bridge_confirmed, spawn_relayer, Relayer},
	utils::{best_finalized_bridged_header, dev_account, retry_until, wait_for_finalized_height},
};

/// Locally built binaries used by the native provider. Paths come from env vars, defaulting to
/// `~/local_bridge_testing/bin`.
struct Binaries {
	polkadot: String,
	polkadot_parachain: String,
	chain_spec_generator_polkadot: String,
	chain_spec_generator_kusama: String,
}

fn bins() -> Binaries {
	let bin_dir = {
		let home = std::env::var("HOME").unwrap_or_default();
		PathBuf::from(home).join("local_bridge_testing/bin")
	};
	let from_env = |var: &str, default_file: &str| {
		std::env::var(var)
			.unwrap_or_else(|_| bin_dir.join(default_file).to_string_lossy().into_owned())
	};
	Binaries {
		polkadot: from_env("POLKADOT_BINARY", "polkadot"),
		polkadot_parachain: from_env("POLKADOT_PARACHAIN_BINARY", "polkadot-parachain"),
		chain_spec_generator_polkadot: from_env(
			"CHAIN_SPEC_GEN_BINARY_FOR_POLKADOT",
			"chain-spec-generator-polkadot",
		),
		chain_spec_generator_kusama: from_env(
			"CHAIN_SPEC_GEN_BINARY_FOR_KUSAMA",
			"chain-spec-generator-kusama",
		),
	}
}

/// The full Kusama <> Polkadot bridge environment: both networks plus any running relayer processes
/// (kept alive for the lifetime of the value).
pub struct BridgeTestEnv {
	pub polkadot: Network<LocalFileSystem>,
	pub kusama: Network<LocalFileSystem>,
	_relayers: Vec<Relayer>,
}

/// Genesis `balances` override for a bridge hub.
///
/// `with_genesis_overrides` *replaces* the `balances.balances` array (it doesn't append), so we
/// must re-list the well-known dev accounts the network relies on — the collators and the relayer
/// signers (`//Alice`, `//Bob`, `//Charlie`, `//Dave`, `//Eve`, `//Ferdie`) all need balance to pay
/// fees — and then add the bridge sovereign/reward accounts.
fn bridge_hub_balances_override(sovereign_accounts: &[&str]) -> serde_json::Value {
	const DEV_FUNDING: u128 = 1u128 << 60;
	let mut balances: Vec<serde_json::Value> =
		[dev::alice(), dev::bob(), dev::charlie(), dev::dave(), dev::eve(), dev::ferdie()]
			.iter()
			.map(|k| serde_json::json!([dev_account(k).to_string(), DEV_FUNDING]))
			.collect();
	for account in sovereign_accounts {
		balances.push(serde_json::json!([*account, SOVEREIGN_FUNDING]));
	}
	serde_json::json!({ "balances": { "balances": balances } })
}

/// Bridge Hub genesis override: fund the sovereign/reward accounts, make `//Alice` the owner of the
/// with-bridged-chain GRANDPA pallet (so `init-bridge` can be signed by `//Alice` without `sudo`)
/// and pin the safe XCM version to [`XCM_VERSION`] (the sudo-free way to set the remote Bridge
/// Hub's XCM version). `grandpa_pallet` is the camelCased genesis key of the with-bridged-chain
/// GRANDPA pallet (`bridgeKusamaGrandpa` on Polkadot BH, `bridgePolkadotGrandpa` on Kusama BH).
fn bridge_hub_genesis_override(
	sovereign_accounts: &[&str],
	grandpa_pallet: &str,
) -> serde_json::Value {
	let alice = dev_account(&dev::alice()).to_string();
	let mut value = bridge_hub_balances_override(sovereign_accounts);
	let obj = value.as_object_mut().expect("object");
	obj.insert(grandpa_pallet.to_string(), serde_json::json!({ "owner": alice }));
	obj.insert("polkadotXcm".to_string(), serde_json::json!({ "safeXcmVersion": XCM_VERSION }));
	value
}

/// Asset Hub genesis override: pin the safe XCM version to [`XCM_VERSION`] (the sudo-free way to
/// set the remote Asset Hub's XCM version). The bridged foreign asset and `//Bob`'s balance of it
/// are pre-registered by the fellows Asset Hub preset, so nothing else is needed.
fn asset_hub_genesis_override() -> serde_json::Value {
	serde_json::json!({ "polkadotXcm": { "safeXcmVersion": XCM_VERSION } })
}

/// Relay-chain genesis override: pre-open the HRMP channels between the Asset Hub and Bridge Hub
/// (the sudo-free way to open them) and set the async-backing params the Asset Hub collators need
/// (see the Rococo <> Westend environment for the rationale).
fn relay_genesis_override(bridge_hub_para_id: u32) -> serde_json::Value {
	serde_json::json!({
		"hrmp": {
			"preopenHrmpChannels": [
				[ASSET_HUB_PARA_ID, bridge_hub_para_id, 4, 524288],
				[bridge_hub_para_id, ASSET_HUB_PARA_ID, 4, 524288],
			],
		},
		"configuration": { "config": {
			"async_backing_params": { "max_candidate_depth": 1, "allowed_ancestry_len": 2 },
		} },
	})
}

fn polkadot_network_config() -> Result<NetworkConfig, anyhow::Error> {
	let bins = bins();
	// Bridge hubs author slot-based and keep the default fork-aware tx pool so the relayer's proof
	// txs survive relay-parent reorgs (see the Rococo <> Westend environment for details).
	let bh_args: Vec<Arg> = vec![
		"-lparachain=info,runtime::bridge=trace,xcm=debug,txpool=debug".into(),
		"--authoring".into(),
		"slot-based".into(),
	];
	let ah_args: Vec<Arg> =
		vec!["-lparachain=info,xcm=debug,runtime::bridge=trace,txpool=debug".into()];
	NetworkConfigBuilder::new()
		.with_relaychain(|r| {
			r.with_chain("polkadot-local")
				.with_default_command(bins.polkadot.as_str())
				.with_chain_spec_command(format!(
					"{} {{{{chainName}}}}",
					bins.chain_spec_generator_polkadot
				))
				.chain_spec_command_is_local(true)
				.with_default_args(vec!["-lparachain=info,xcm=debug".into()])
				.with_genesis_overrides(relay_genesis_override(BRIDGE_HUB_POLKADOT_PARA_ID))
				.with_validator(|n| {
					n.with_name("alice-polkadot-validator").with_initial_balance(2_000_000_000_000)
				})
				.with_validator(|n| {
					n.with_name("bob-polkadot-validator").with_initial_balance(2_000_000_000_000)
				})
				.with_validator(|n| {
					n.with_name("charlie-polkadot-validator")
						.with_initial_balance(2_000_000_000_000)
				})
		})
		.with_parachain(|p| {
			p.with_id(BRIDGE_HUB_POLKADOT_PARA_ID)
				.with_chain("bridge-hub-polkadot-local")
				.cumulus_based(true)
				.with_default_command(bins.polkadot_parachain.as_str())
				.with_chain_spec_command(format!(
					"{} {{{{chainName}}}}",
					bins.chain_spec_generator_polkadot
				))
				.chain_spec_command_is_local(true)
				// Fund sovereign/reward accounts, set //Alice as GRANDPA owner and pin the XCM
				// version — all at genesis, so no `sudo` is needed.
				.with_genesis_overrides(bridge_hub_genesis_override(
					&[
						ASSET_HUB_SOVEREIGN_AT_BRIDGE_HUB_POLKADOT,
						BHP_LANE_THIS_CHAIN,
						BHP_LANE_BRIDGED_CHAIN,
					],
					"bridgeKusamaGrandpa",
				))
				// A single bridge-hub collator builds linearly (a second fork-wars, see R<>W).
				.with_collator(|n| {
					n.with_name("bridge-hub-polkadot-collator1").with_args(bh_args.clone())
				})
		})
		.with_parachain(|p| {
			p.with_id(ASSET_HUB_PARA_ID)
				.with_chain("asset-hub-polkadot-local")
				.cumulus_based(true)
				.with_default_command(bins.polkadot_parachain.as_str())
				.with_chain_spec_command(format!(
					"{} {{{{chainName}}}}",
					bins.chain_spec_generator_polkadot
				))
				.chain_spec_command_is_local(true)
				.with_genesis_overrides(asset_hub_genesis_override())
				.with_collator(|n| {
					n.with_name("asset-hub-polkadot-collator1").with_args(ah_args.clone())
				})
		})
		.with_global_settings(global_settings)
		.build()
		.map_err(config_errs)
}

fn kusama_network_config() -> Result<NetworkConfig, anyhow::Error> {
	let bins = bins();
	let bh_args: Vec<Arg> = vec![
		"-lparachain=info,runtime::bridge=trace,xcm=debug,txpool=debug".into(),
		"--authoring".into(),
		"slot-based".into(),
	];
	let ah_args: Vec<Arg> =
		vec!["-lparachain=info,xcm=debug,runtime::bridge=trace,txpool=debug".into()];
	NetworkConfigBuilder::new()
		.with_relaychain(|r| {
			r.with_chain("kusama-local")
				.with_default_command(bins.polkadot.as_str())
				.with_chain_spec_command(format!(
					"{} {{{{chainName}}}}",
					bins.chain_spec_generator_kusama
				))
				.chain_spec_command_is_local(true)
				.with_default_args(vec!["-lparachain=info,xcm=debug".into()])
				.with_genesis_overrides(relay_genesis_override(BRIDGE_HUB_KUSAMA_PARA_ID))
				.with_validator(|n| {
					n.with_name("alice-kusama-validator").with_initial_balance(2_000_000_000_000)
				})
				.with_validator(|n| {
					n.with_name("bob-kusama-validator").with_initial_balance(2_000_000_000_000)
				})
				.with_validator(|n| {
					n.with_name("charlie-kusama-validator").with_initial_balance(2_000_000_000_000)
				})
		})
		.with_parachain(|p| {
			p.with_id(BRIDGE_HUB_KUSAMA_PARA_ID)
				.with_chain("bridge-hub-kusama-local")
				.cumulus_based(true)
				.with_default_command(bins.polkadot_parachain.as_str())
				.with_chain_spec_command(format!(
					"{} {{{{chainName}}}}",
					bins.chain_spec_generator_kusama
				))
				.chain_spec_command_is_local(true)
				.with_genesis_overrides(bridge_hub_genesis_override(
					&[
						ASSET_HUB_SOVEREIGN_AT_BRIDGE_HUB_KUSAMA,
						BHK_LANE_THIS_CHAIN,
						BHK_LANE_BRIDGED_CHAIN,
					],
					"bridgePolkadotGrandpa",
				))
				.with_collator(|n| {
					n.with_name("bridge-hub-kusama-collator1").with_args(bh_args.clone())
				})
		})
		.with_parachain(|p| {
			p.with_id(ASSET_HUB_PARA_ID)
				.with_chain("asset-hub-kusama-local")
				.cumulus_based(true)
				.with_default_command(bins.polkadot_parachain.as_str())
				.with_chain_spec_command(format!(
					"{} {{{{chainName}}}}",
					bins.chain_spec_generator_kusama
				))
				.chain_spec_command_is_local(true)
				.with_genesis_overrides(asset_hub_genesis_override())
				.with_collator(|n| {
					n.with_name("asset-hub-kusama-collator1").with_args(ah_args.clone())
				})
		})
		.with_global_settings(global_settings)
		.build()
		.map_err(config_errs)
}

/// Shared global settings for both networks. We disable `tear_down_on_failure` so a transient,
/// load-induced node-monitor timeout does not tear the network down (see the R<>W environment).
fn global_settings(
	settings: zombienet_sdk::GlobalSettingsBuilder,
) -> zombienet_sdk::GlobalSettingsBuilder {
	settings.with_tear_down_on_failure(false).with_node_spawn_timeout(600)
}

fn config_errs(errs: Vec<anyhow::Error>) -> anyhow::Error {
	anyhow!(
		"network config errors: {}",
		errs.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; ")
	)
}

/// Spawns one network with the **native** provider, retrying the flaky zombienet chain-spec panic
/// (see the R<>W environment for the details of the retry). The native provider requires the
/// `polkadot` / `polkadot-parachain` / `chain-spec-generator` binaries resolved by [`bins`].
async fn spawn_with_retry(
	config_fn: impl Fn() -> Result<NetworkConfig, anyhow::Error>,
	name: &str,
) -> Result<Network<LocalFileSystem>, anyhow::Error> {
	const MAX_ATTEMPTS: usize = 3;
	let spawn_fn = Provider::Native.get_spawn_fn();
	let mut last_err = String::new();
	for attempt in 1..=MAX_ATTEMPTS {
		let config = config_fn()?;
		match tokio::spawn(spawn_fn(config)).await {
			Ok(Ok(network)) => return Ok(network),
			Ok(Err(e)) => {
				last_err = e.to_string();
				log::warn!(
					"{name} network spawn attempt {attempt}/{MAX_ATTEMPTS} failed: {last_err}"
				);
			},
			Err(join_err) if join_err.is_panic() => {
				let panic = join_err.into_panic();
				last_err = panic
					.downcast_ref::<&str>()
					.map(|s| s.to_string())
					.or_else(|| panic.downcast_ref::<String>().cloned())
					.unwrap_or_else(|| "unknown panic".to_string());
				log::warn!(
					"{name} network spawn attempt {attempt}/{MAX_ATTEMPTS} panicked inside \
					 zombienet (likely the chain-spec truncation flake); retrying: {last_err}"
				);
			},
			Err(join_err) => return Err(anyhow!("{name} network spawn task cancelled: {join_err}")),
		}
	}
	Err(anyhow!("{name} network spawn failed after {MAX_ATTEMPTS} attempts: {last_err}"))
}

impl BridgeTestEnv {
	/// Spawns both networks and, depending on the flags, initializes the bridge and starts the
	/// relayer.
	pub async fn spawn(init: bool, start_relayer: bool) -> Result<Self, anyhow::Error> {
		let _ = env_logger::try_init_from_env(
			env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
		);

		log::info!("Spawning Polkadot and Kusama networks concurrently");
		let (polkadot, kusama) = tokio::try_join!(
			spawn_with_retry(polkadot_network_config, "Polkadot"),
			spawn_with_retry(kusama_network_config, "Kusama"),
		)?;

		let mut env = BridgeTestEnv { polkadot, kusama, _relayers: Vec::new() };

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

	// The bridge is bootstrapped from genesis (no `sudo`), so — unlike the Rococo <> Westend
	// environment — a typed relay-chain client is never needed; only the relay WS endpoints (read
	// via `get_node(..).ws_uri()` in `start_relayer`). Kept as symmetric helpers.
	#[allow(dead_code)]
	pub async fn polkadot_relay_client(
		&self,
	) -> Result<OnlineClient<PolkadotConfig>, anyhow::Error> {
		Self::client_of(&self.polkadot, "alice-polkadot-validator").await
	}
	#[allow(dead_code)]
	pub async fn kusama_relay_client(&self) -> Result<OnlineClient<PolkadotConfig>, anyhow::Error> {
		Self::client_of(&self.kusama, "alice-kusama-validator").await
	}
	pub async fn asset_hub_polkadot_client(
		&self,
	) -> Result<OnlineClient<PolkadotConfig>, anyhow::Error> {
		Self::client_of(&self.polkadot, "asset-hub-polkadot-collator1").await
	}
	pub async fn asset_hub_kusama_client(
		&self,
	) -> Result<OnlineClient<PolkadotConfig>, anyhow::Error> {
		Self::client_of(&self.kusama, "asset-hub-kusama-collator1").await
	}
	pub async fn bridge_hub_polkadot_client(
		&self,
	) -> Result<OnlineClient<PolkadotConfig>, anyhow::Error> {
		Self::client_of(&self.polkadot, "bridge-hub-polkadot-collator1").await
	}
	pub async fn bridge_hub_kusama_client(
		&self,
	) -> Result<OnlineClient<PolkadotConfig>, anyhow::Error> {
		Self::client_of(&self.kusama, "bridge-hub-kusama-collator1").await
	}

	/// Confirms the genesis-configured bridge state is live and seeds the asset-conversion pools.
	///
	/// The HRMP channels, remote XCM version, GRANDPA owner and funded sovereign/reward accounts
	/// are all configured at genesis (no `sudo`), so here we only wait for the Bridge Hubs to
	/// finalize, confirm the HRMP egress channels are open and create the native<>bridged pools
	/// with `//Bob` (a regular signed extrinsic — no `sudo`).
	async fn init_bridge(&self) -> Result<(), anyhow::Error> {
		let ahp = self.asset_hub_polkadot_client().await?;
		let ahk = self.asset_hub_kusama_client().await?;
		let bhp = self.bridge_hub_polkadot_client().await?;
		let bhk = self.bridge_hub_kusama_client().await?;

		// Wait until each bridge hub finalizes its first block: proof the full
		// collation -> backing -> inclusion -> finality pipeline is live.
		log::info!("Waiting for bridge hubs to finalize their first block");
		tokio::try_join!(
			wait_for_finalized_height(&bhp, 1, Duration::from_secs(300)),
			wait_for_finalized_height(&bhk, 1, Duration::from_secs(300)),
		)?;

		// Confirm the genesis-opened HRMP egress channels towards the Bridge Hubs are live.
		log::info!("Waiting for HRMP channels to open");
		tokio::try_join!(
			retry_until(Duration::from_secs(600), || {
				let ahp = ahp.clone();
				async move {
					Ok(asset_hub_polkadot::hrmp_egress_open(&ahp, BRIDGE_HUB_POLKADOT_PARA_ID)
						.await?
						.then_some(()))
				}
			}),
			retry_until(Duration::from_secs(600), || {
				let ahk = ahk.clone();
				async move {
					Ok(asset_hub_kusama::hrmp_egress_open(&ahk, BRIDGE_HUB_KUSAMA_PARA_ID)
						.await?
						.then_some(()))
				}
			}),
		)?;
		log::info!("HRMP channels open on both Asset Hubs");

		// Seed the native<>bridged asset-conversion pools so the bridged asset's XCM fees can be
		// swapped to native (see the R<>W environment / the `asset_hub_ops!` docs). The fellows
		// Asset Hub preset already registers the bridged asset and endows `//Bob` with it, so
		// `//Bob` funds both sides. Amounts account for the differing decimals (DOT: 10, KSM: 12).
		let bob = dev::bob();
		let bob_acc = dev_account(&bob);
		log::info!("Seeding native<>bridged asset-conversion pools on both Asset Hubs");
		tokio::try_join!(
			async {
				asset_hub_polkadot::create_pool(&ahp, &bob, 0).await?;
				asset_hub_polkadot::add_liquidity(
					&ahp,
					&bob,
					10 * POLKADOT_UNIT,
					4 * KUSAMA_UNIT,
					bob_acc.clone(),
					1,
				)
				.await
			},
			async {
				asset_hub_kusama::create_pool(&ahk, &bob, 0).await?;
				asset_hub_kusama::add_liquidity(
					&ahk,
					&bob,
					KUSAMA_UNIT,
					2 * POLKADOT_UNIT + POLKADOT_UNIT / 2,
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

	/// Initializes the GRANDPA bridge pallets (owner-signed by `//Alice`, no `sudo`) and starts the
	/// finality, parachains and messages relayers.
	pub async fn start_relayer(&mut self) -> Result<(), anyhow::Error> {
		// Resolve the actual node WS endpoints (ports are assigned dynamically by zombienet).
		let polkadot_relay =
			self.polkadot.get_node("alice-polkadot-validator")?.ws_uri().to_string();
		let kusama_relay = self.kusama.get_node("alice-kusama-validator")?.ws_uri().to_string();
		let bh_polkadot =
			self.polkadot.get_node("bridge-hub-polkadot-collator1")?.ws_uri().to_string();
		let bh_kusama = self.kusama.get_node("bridge-hub-kusama-collator1")?.ws_uri().to_string();

		let bhp_client = Self::client_of(&self.polkadot, "bridge-hub-polkadot-collator1").await?;
		let bhk_client = Self::client_of(&self.kusama, "bridge-hub-kusama-collator1").await?;

		// init-bridge each direction, signed by the genesis GRANDPA owner `//Alice`, retried until
		// confirmed at a finalized block. Bridge Hub Polkadot tracks Kusama
		// (`BridgeKusamaGrandpa`), Bridge Hub Kusama tracks Polkadot (`BridgePolkadotGrandpa`).
		let init_bhp_args = [
			"init-bridge",
			"kusama-to-bridge-hub-polkadot",
			"--source-uri",
			kusama_relay.as_str(),
			"--source-version-mode",
			"Auto",
			"--target-uri",
			bh_polkadot.as_str(),
			"--target-version-mode",
			"Auto",
			"--target-signer",
			"//Alice",
		];
		let init_bhk_args = [
			"init-bridge",
			"polkadot-to-bridge-hub-kusama",
			"--source-uri",
			polkadot_relay.as_str(),
			"--source-version-mode",
			"Auto",
			"--target-uri",
			bh_kusama.as_str(),
			"--target-version-mode",
			"Auto",
			"--target-signer",
			"//Alice",
		];
		tokio::try_join!(
			init_bridge_confirmed(&init_bhp_args, &bhp_client, "BridgeKusamaGrandpa"),
			init_bridge_confirmed(&init_bhk_args, &bhk_client, "BridgePolkadotGrandpa"),
		)?;

		// Finality relayers (free relay-chain headers, signed by //Charlie).
		self._relayers.push(spawn_relayer(&[
			"relay-headers",
			"polkadot-to-bridge-hub-kusama",
			"--only-free-headers",
			"--source-uri",
			polkadot_relay.as_str(),
			"--source-version-mode",
			"Auto",
			"--target-uri",
			bh_kusama.as_str(),
			"--target-version-mode",
			"Auto",
			"--target-signer",
			"//Charlie",
			"--target-transactions-mortality",
			"1024",
		])?);
		self._relayers.push(spawn_relayer(&[
			"relay-headers",
			"kusama-to-bridge-hub-polkadot",
			"--only-free-headers",
			"--source-uri",
			kusama_relay.as_str(),
			"--source-version-mode",
			"Auto",
			"--target-uri",
			bh_polkadot.as_str(),
			"--target-version-mode",
			"Auto",
			"--target-signer",
			"//Charlie",
			"--target-transactions-mortality",
			"1024",
		])?);

		// Parachains relayers (free parachain headers, signed by //Dave). Source is the relay chain
		// (to read the sibling parachain heads), target is the remote Bridge Hub.
		self._relayers.push(spawn_relayer(&[
			"relay-parachains",
			"bridge-hub-polkadot-to-bridge-hub-kusama",
			"--only-free-headers",
			"--source-uri",
			polkadot_relay.as_str(),
			"--source-version-mode",
			"Auto",
			"--target-uri",
			bh_kusama.as_str(),
			"--target-version-mode",
			"Auto",
			"--target-signer",
			"//Dave",
			"--target-transactions-mortality",
			"1024",
		])?);
		self._relayers.push(spawn_relayer(&[
			"relay-parachains",
			"bridge-hub-kusama-to-bridge-hub-polkadot",
			"--only-free-headers",
			"--source-uri",
			kusama_relay.as_str(),
			"--source-version-mode",
			"Auto",
			"--target-uri",
			bh_polkadot.as_str(),
			"--target-version-mode",
			"Auto",
			"--target-signer",
			"//Dave",
			"--target-transactions-mortality",
			"1024",
		])?);

		// Messages relayers (lane 0x00000001; //Eve for pol->ksm, //Ferdie for ksm->pol).
		self._relayers.push(spawn_relayer(&[
			"relay-messages",
			"bridge-hub-polkadot-to-bridge-hub-kusama",
			"--source-uri",
			bh_polkadot.as_str(),
			"--source-version-mode",
			"Auto",
			"--source-signer",
			"//Eve",
			"--source-transactions-mortality",
			"1024",
			"--target-uri",
			bh_kusama.as_str(),
			"--target-version-mode",
			"Auto",
			"--target-signer",
			"//Eve",
			"--target-transactions-mortality",
			"1024",
			"--lane",
			"00000001",
		])?);
		self._relayers.push(spawn_relayer(&[
			"relay-messages",
			"bridge-hub-kusama-to-bridge-hub-polkadot",
			"--source-uri",
			bh_kusama.as_str(),
			"--source-version-mode",
			"Auto",
			"--source-signer",
			"//Ferdie",
			"--source-transactions-mortality",
			"1024",
			"--target-uri",
			bh_polkadot.as_str(),
			"--target-version-mode",
			"Auto",
			"--target-signer",
			"//Ferdie",
			"--target-transactions-mortality",
			"1024",
			"--lane",
			"00000001",
		])?);

		log::info!("Waiting for the GRANDPA bridge pallets to be initialized");
		let bhp = self.bridge_hub_polkadot_client().await?;
		let bhk = self.bridge_hub_kusama_client().await?;
		retry_until(Duration::from_secs(400), || {
			let bhp = bhp.clone();
			async move {
				Ok(best_finalized_bridged_header(&bhp, "KusamaFinalityApi")
					.await?
					.filter(|n| *n > 0)
					.map(|_| ()))
			}
		})
		.await?;
		retry_until(Duration::from_secs(400), || {
			let bhk = bhk.clone();
			async move {
				Ok(best_finalized_bridged_header(&bhk, "PolkadotFinalityApi")
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
