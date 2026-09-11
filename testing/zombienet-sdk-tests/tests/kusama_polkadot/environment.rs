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
//!   * bootstraps as much as possible **from genesis** — the local + remote XCM versions
//!     (`polkadotXcm.safeXcmVersion` / `supportedVersion`), the bridge-GRANDPA pallet owner
//!     (`//Alice`, so `init-bridge` can be owner-signed without `sudo`) and the funded
//!     sovereign/reward accounts (Bridge Hub `balances`) — leaving the HRMP channels (opened after
//!     spawn via the permissionless `Hrmp::establish_system_channel`, since pre-opening them at
//!     genesis corrupts the Bridge Hub's downward-message-queue head) and the `//Bob`-signed
//!     asset-conversion pools to be set up post-spawn.
//!
//! The bridged foreign asset (wKSM on Asset Hub Polkadot, wDOT on Asset Hub Kusama) and `//Bob`'s
//! balance of it are already pre-registered by the fellows Asset Hub genesis presets, so nothing
//! needs to create them here.

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
	BRIDGE_HUB_KUSAMA_PARA_ID, BRIDGE_HUB_POLKADOT_PARA_ID, ENDOWMENT, KUSAMA_UNIT, POLKADOT_UNIT,
	SOVEREIGN_FUNDING, XCM_VERSION,
};
use crate::common::{
	light_client::{self, LightClient, RelayTransport, Target},
	relayer::{init_bridge_confirmed, spawn_relayer, Relayer},
	utils::{
		best_finalized_bridged_header, bridge_hub_balances, config_errs, dev_account,
		global_settings, retry_until, sign_submit_wait, spawn_with_retry,
		wait_for_finalized_height,
	},
};

/// Locally built binaries used by the native provider.
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
	/// smoldot sidecars backing the relayers, when `BRIDGE_RELAY_TRANSPORT` asks for them. Held
	/// here only to keep them alive for as long as the relayers that read from them.
	_light_clients: Vec<LightClient>,
}

/// Bridge Hub genesis override: fund the sovereign/reward accounts, make `//Alice` the
/// with-bridged-chain GRANDPA pallet owner (so `init-bridge` needs no `sudo`), and set the local
/// safe + remote Bridge Hub XCM versions. `grandpa_pallet` is that pallet's camelCased genesis key
/// (`bridgeKusamaGrandpa` on Polkadot BH, `bridgePolkadotGrandpa` on Kusama BH).
fn bridge_hub_genesis_override(
	sovereign_accounts: &[&str],
	grandpa_pallet: &str,
	remote_network: &str,
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

/// Genesis `supportedVersion` entry recording [`XCM_VERSION`] for the remote bridged
/// `(parents: 2, GlobalConsensus(remote_network) / Parachain(para_id))` — the sudo-free equivalent
/// of `force_xcm_version`, without which the bridged transfer fails with `SendFailure`.
fn supported_remote_version(remote_network: &str, para_id: u32) -> serde_json::Value {
	serde_json::json!([[
		{ "parents": 2, "interior": { "X2": [
			{ "GlobalConsensus": remote_network },
			{ "Parachain": para_id },
		] } },
		XCM_VERSION,
	]])
}

/// Asset Hub genesis override: set the local safe XCM version and the remote Asset Hub's supported
/// version (`remote_network`: `"Kusama"` on Polkadot AH, `"Polkadot"` on Kusama AH), and record the
/// remote Asset Hub as trusted reserve of the bridged asset. The executor's `IsReserve`
/// (`NonTeleportableAssetFromTrustedReserve`) only accepts reserves stored in
/// `ForeignAssets::Reserves`; the fellows preset registers the asset (and `//Bob`'s balance) but no
/// reserves, so incoming transfers fail with `UntrustedReserveLocation`. Genesis is the sudo-free
/// stand-in for the owner's `set_reserves` call.
fn asset_hub_genesis_override(remote_network: &str) -> serde_json::Value {
	let bridged_asset = serde_json::json!({ "parents": 2, "interior": { "X1": [
			{ "GlobalConsensus": remote_network },
		] } });
	let remote_asset_hub = serde_json::json!({ "parents": 2, "interior": { "X2": [
			{ "GlobalConsensus": remote_network },
			{ "Parachain": ASSET_HUB_PARA_ID },
		] } });
	serde_json::json!({
		"polkadotXcm": {
			"safeXcmVersion": XCM_VERSION,
			"supportedVersion": supported_remote_version(remote_network, ASSET_HUB_PARA_ID),
		},
		"foreignAssets": {
			"reserves": [[
				bridged_asset,
				[ { "reserve": remote_asset_hub, "teleportable": false } ],
			]],
		},
	})
}

/// Relay-chain genesis override: set the async-backing params the Asset Hub collators need (see the
/// Rococo <> Westend environment for the rationale) and deepen the relay's scheduling lookahead.
///
/// The HRMP channels are deliberately **not** pre-opened here. Opening them via the `hrmp`
/// `preopenHrmpChannels` genesis leaves the Bridge Hub parachain with a downward-message-queue
/// whose MQC head it cannot reconcile on its first block, so `set_validation_data` traps
/// (`cumulus-pallet-parachain-system: DMQ head mismatch`), the collator can never build block #1
/// and the Bridge Hubs never finalize. They are instead opened after spawn through the live HRMP
/// pipeline via the permissionless `Hrmp::establish_system_channel` (see `open_hrmp_channels_*`).
fn relay_genesis_override() -> serde_json::Value {
	serde_json::json!({
		"configuration": { "config": {
			"async_backing_params": { "max_candidate_depth": 1, "allowed_ancestry_len": 2 },
			"scheduler_params": { "lookahead": 3 },
		} },
	})
}

/// Collator args — all parachains author slot-based (otherwise `set_validation_data` traps, see
/// R<>W).
fn bridge_hub_args() -> Vec<Arg> {
	vec![
		"-lparachain=info,runtime::bridge=trace,xcm=debug,txpool=debug".into(),
		"--authoring".into(),
		"slot-based".into(),
	]
}

fn asset_hub_args() -> Vec<Arg> {
	vec![
		"-lparachain=info,xcm=debug,runtime::bridge=trace,txpool=debug".into(),
		"--authoring".into(),
		"slot-based".into(),
	]
}

fn polkadot_network_config() -> Result<NetworkConfig, anyhow::Error> {
	let bins = bins();
	let bh_args = bridge_hub_args();
	let ah_args = asset_hub_args();
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
				.with_genesis_overrides(relay_genesis_override())
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
				.with_genesis_overrides(bridge_hub_genesis_override(
					&[
						ASSET_HUB_SOVEREIGN_AT_BRIDGE_HUB_POLKADOT,
						BHP_LANE_THIS_CHAIN,
						BHP_LANE_BRIDGED_CHAIN,
					],
					"bridgeKusamaGrandpa",
					"Kusama",
					BRIDGE_HUB_KUSAMA_PARA_ID,
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
				.with_genesis_overrides(asset_hub_genesis_override("Kusama"))
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
	let bh_args = bridge_hub_args();
	let ah_args = asset_hub_args();
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
				.with_genesis_overrides(relay_genesis_override())
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
					"Polkadot",
					BRIDGE_HUB_POLKADOT_PARA_ID,
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
				.with_genesis_overrides(asset_hub_genesis_override("Polkadot"))
				.with_collator(|n| {
					n.with_name("asset-hub-kusama-collator1").with_args(ah_args.clone())
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

		log::info!("Spawning Polkadot and Kusama networks concurrently");
		let (polkadot, kusama) = tokio::try_join!(
			spawn_with_retry(Provider::Native.get_spawn_fn(), polkadot_network_config, "Polkadot"),
			spawn_with_retry(Provider::Native.get_spawn_fn(), kusama_network_config, "Kusama"),
		)?;

		let mut env =
			BridgeTestEnv { polkadot, kusama, _relayers: Vec::new(), _light_clients: Vec::new() };

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

	// Typed relay-chain clients, used post-spawn to open the HRMP channels (see
	// `open_hrmp_channels_*`).
	pub async fn polkadot_relay_client(
		&self,
	) -> Result<OnlineClient<PolkadotConfig>, anyhow::Error> {
		Self::client_of(&self.polkadot, "alice-polkadot-validator").await
	}
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

	/// Opens the Asset Hub <-> Bridge Hub HRMP channels (both directions) on the **Polkadot** relay
	/// via the permissionless `Hrmp::establish_system_channel` — both endpoints are system
	/// parachains, so no `sudo`/root is needed. Signed by `//Alice`; each direction waits for
	/// finalized success before the next (shared signer nonce).
	async fn open_hrmp_channels_polkadot(&self) -> Result<(), anyhow::Error> {
		use crate::polkadot::runtime_types::polkadot_parachain_primitives::primitives::Id;
		let relay = self.polkadot_relay_client().await?;
		let alice = dev::alice();
		for (sender, recipient) in [
			(ASSET_HUB_PARA_ID, BRIDGE_HUB_POLKADOT_PARA_ID),
			(BRIDGE_HUB_POLKADOT_PARA_ID, ASSET_HUB_PARA_ID),
		] {
			let tx =
				crate::polkadot::tx().hrmp().establish_system_channel(Id(sender), Id(recipient));
			sign_submit_wait(&relay, &tx, &alice).await?;
		}
		Ok(())
	}

	/// The Kusama-side counterpart of [`Self::open_hrmp_channels_polkadot`].
	async fn open_hrmp_channels_kusama(&self) -> Result<(), anyhow::Error> {
		use crate::kusama::runtime_types::polkadot_parachain_primitives::primitives::Id;
		let relay = self.kusama_relay_client().await?;
		let alice = dev::alice();
		for (sender, recipient) in [
			(ASSET_HUB_PARA_ID, BRIDGE_HUB_KUSAMA_PARA_ID),
			(BRIDGE_HUB_KUSAMA_PARA_ID, ASSET_HUB_PARA_ID),
		] {
			let tx = crate::kusama::tx().hrmp().establish_system_channel(Id(sender), Id(recipient));
			sign_submit_wait(&relay, &tx, &alice).await?;
		}
		Ok(())
	}

	/// Opens the HRMP channels post-spawn and seeds the asset-conversion pools.
	///
	/// The HRMP channels can't be pre-opened at genesis (see `relay_genesis_override`), so they're
	/// opened here, once the Bridge Hubs produce blocks, via the permissionless
	/// `Hrmp::establish_system_channel`; then the `//Bob`-signed pools (no `sudo`).
	async fn init_bridge(&self) -> Result<(), anyhow::Error> {
		let ahp = self.asset_hub_polkadot_client().await?;
		let ahk = self.asset_hub_kusama_client().await?;
		let bhp = self.bridge_hub_polkadot_client().await?;
		let bhk = self.bridge_hub_kusama_client().await?;

		// Wait until each bridge hub finalizes its first block.
		log::info!("Waiting for bridge hubs to finalize their first block");
		tokio::try_join!(
			wait_for_finalized_height(&bhp, 1, Duration::from_secs(300)),
			wait_for_finalized_height(&bhk, 1, Duration::from_secs(300)),
		)?;

		// Open the HRMP channels now that the Bridge Hubs produce blocks, so the channel-open
		// notifications flow through the live downward-message pipeline (see
		// `relay_genesis_override`).
		log::info!("Opening HRMP channels between Asset Hub and Bridge Hub on both relays");
		tokio::try_join!(self.open_hrmp_channels_polkadot(), self.open_hrmp_channels_kusama())?;

		// Confirm the HRMP egress channels towards the Bridge Hubs have opened.
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

		// Only the *relayers* move to the light clients (#3270); the assertions below keep
		// reading through `subxt` against the nodes, so a light-client defect shows up as a
		// bridge that stops relaying rather than as a test passing against a lying data source.
		let transport = RelayTransport::from_env()?;
		log::info!("Relayer transport: {transport:?}");

		let (polkadot_relay, kusama_relay) = if transport.relays_via_light_client() {
			let (polkadot_lc, kusama_lc) = tokio::try_join!(
				light_client::spawn(
					&self.polkadot,
					Target::Relay { bootnode: "alice-polkadot-validator" },
					"polkadot",
				),
				light_client::spawn(
					&self.kusama,
					Target::Relay { bootnode: "alice-kusama-validator" },
					"kusama",
				),
			)?;
			let uris = (polkadot_lc.uri().to_string(), kusama_lc.uri().to_string());
			self._light_clients.push(polkadot_lc);
			self._light_clients.push(kusama_lc);
			uris
		} else {
			(polkadot_relay, kusama_relay)
		};

		let (bh_polkadot, bh_kusama) = if transport.bridge_hubs_via_light_client() {
			let (bhp_lc, bhk_lc) = tokio::try_join!(
				light_client::spawn(
					&self.polkadot,
					Target::Para {
						para_id: BRIDGE_HUB_POLKADOT_PARA_ID,
						bootnode: "bridge-hub-polkadot-collator1",
						relay_bootnode: "alice-polkadot-validator",
					},
					"bridge-hub-polkadot",
				),
				light_client::spawn(
					&self.kusama,
					Target::Para {
						para_id: BRIDGE_HUB_KUSAMA_PARA_ID,
						bootnode: "bridge-hub-kusama-collator1",
						relay_bootnode: "alice-kusama-validator",
					},
					"bridge-hub-kusama",
				),
			)?;
			let uris = (bhp_lc.uri().to_string(), bhk_lc.uri().to_string());
			self._light_clients.push(bhp_lc);
			self._light_clients.push(bhk_lc);
			uris
		} else {
			(bh_polkadot, bh_kusama)
		};

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
