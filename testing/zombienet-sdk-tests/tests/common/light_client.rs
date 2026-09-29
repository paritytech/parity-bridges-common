// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

//! smoldot light-client sidecar driver, for running the bridge relayers without a trusted RPC
//! node (see [paritytech/parity-bridges-common#3270]).
//!
//! A sidecar is smoldot's `json-rpc-server` example: an in-process light client for one chain,
//! exposed as a JSON-RPC WebSocket server. `substrate-relay` points its `--source-uri` /
//! `--target-uri` at that server instead of at a node's RPC port, so every header, storage proof
//! and GRANDPA justification the relayer consumes is verified locally from the p2p network rather
//! than taken on trust from an operated node.
//!
//! The sidecar joins the network as a normal light client, so all it needs is the chain
//! specification zombienet materialised for the network - which, usefully, zombienet has already
//! post-processed into a light-client-friendly shape. [`smoldot_chain_spec`] copies it with a
//! couple of small adjustments; see its docs.
//!
//! [paritytech/parity-bridges-common#3270]: https://github.com/paritytech/parity-bridges-common/issues/3270

use anyhow::anyhow;
use std::{
	path::{Path, PathBuf},
	process::Stdio,
	time::Duration,
};
use subxt::{backend::rpc::RpcClient, ext::subxt_rpcs::client::RpcParams};
use tokio::{
	process::{Child, Command},
	time::{sleep, Instant},
};
use zombienet_sdk::{LocalFileSystem, Network};

/// `RUST_LOG` for the sidecars. `info` is what makes the "listening on" and "Chain initialization
/// complete" lines - which [`spawn`] parses - appear.
const SIDECAR_RUST_LOG: &str = "info";

const LISTEN_TIMEOUT: Duration = Duration::from_secs(30);

/// How long a sidecar may take to warp-sync far enough to answer with a finalized block. Generous:
/// it covers joining the p2p network, the warp-sync round trip and downloading + compiling the
/// runtime, all against a chain that may itself only just have started producing blocks.
const SYNC_TIMEOUT: Duration = Duration::from_secs(240);

/// Where the relayers get their chain data from.
///
/// Selected by the `BRIDGE_RELAY_TRANSPORT` environment variable. The light-client variants are
/// staged deliberately: reading from a light client (headers, justifications, storage proofs) is
/// what #3270 is about and is the lower-risk half, while *submitting* through one additionally
/// depends on smoldot's transaction service reporting `finalized`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayTransport {
	/// Relayers talk to the zombienet nodes' RPC ports. The default, and what CI runs today.
	Rpc,
	/// Relay chains are served by smoldot sidecars; the Bridge Hubs stay on RPC.
	///
	/// This covers the GRANDPA finality relay (justifications) and the parachain-head relay
	/// (`Paras::Heads` storage proofs) - the two things a bridge fundamentally needs from its
	/// source chain.
	LightClientRelays,
	/// Every chain the relayers touch is served by a smoldot sidecar, Bridge Hubs included.
	///
	/// This additionally puts message-delivery proofs on a parachain light client and routes all
	/// transaction submission through smoldot.
	LightClientAll,
}

impl RelayTransport {
	pub fn from_env() -> Result<Self, anyhow::Error> {
		match std::env::var("BRIDGE_RELAY_TRANSPORT").as_deref() {
			Err(_) | Ok("") | Ok("rpc") => Ok(Self::Rpc),
			Ok("light-client-relays") => Ok(Self::LightClientRelays),
			Ok("light-client-all") => Ok(Self::LightClientAll),
			Ok(other) => Err(anyhow!(
				"unknown BRIDGE_RELAY_TRANSPORT `{other}`; expected one of \
				 `rpc`, `light-client-relays`, `light-client-all`"
			)),
		}
	}

	pub fn relays_via_light_client(&self) -> bool {
		matches!(self, Self::LightClientRelays | Self::LightClientAll)
	}

	pub fn bridge_hubs_via_light_client(&self) -> bool {
		matches!(self, Self::LightClientAll)
	}
}

/// Path to smoldot's `json-rpc-server` example binary.
///
/// Build it from a smoldot checkout with
/// `cargo build --release -p smoldot-light --example json-rpc-server`.
fn sidecar_binary() -> PathBuf {
	if let Ok(path) = std::env::var("SMOLDOT_JSON_RPC_SERVER_BINARY") {
		return PathBuf::from(path);
	}
	let home = std::env::var("HOME").unwrap_or_default();
	PathBuf::from(home).join("local_bridge_testing/bin/json-rpc-server")
}

/// A running smoldot sidecar. Killed when dropped, like [`crate::common::relayer::Relayer`].
pub struct LightClient {
	_child: Child,
	uri: String,
	log_path: PathBuf,
}

impl LightClient {
	pub fn uri(&self) -> &str {
		&self.uri
	}

	pub fn log_path(&self) -> &Path {
		&self.log_path
	}
}

impl Drop for LightClient {
	fn drop(&mut self) {
		let _ = self._child.start_kill();
	}
}

/// Which chain of a zombienet network a sidecar should follow.
pub enum Target<'a> {
	/// The relay chain itself, seeded from the given relay-chain node.
	Relay { bootnode: &'a str },
	/// A parachain, seeded from the given collator. The relay chain is added to the light client
	/// too (smoldot derives a parachain's finality from the relay chain's `Paras::Heads`), so a
	/// relay-chain node to seed *it* from is needed as well.
	Para { para_id: u32, bootnode: &'a str, relay_bootnode: &'a str },
}

/// Derives a smoldot-ready chain specification from the one zombienet materialised.
///
/// Zombienet already does most of the work: it writes a **raw** spec, fills `bootNodes` in with a
/// running node's libp2p address (nodes listen on `--listen-addr /ip4/0.0.0.0/tcp/<p2p>/ws`, so the
/// addresses are the `/ws` form smoldot dials happily), and rewrites a parachain's `relay_chain` to
/// the relay-chain spec's `id`. Every top-level key it emits is one smoldot's `ClientSpec` knows,
/// which matters because that struct is `deny_unknown_fields`.
///
/// So this only:
///
/// 1. **adds `bootnode` to `bootNodes`** if it is not already there, so the sidecar is guaranteed a
///    peer we know to be up rather than only whichever node zombienet happened to record;
/// 2. **normalises `relay_chain`** to `relay_chain_id` for parachains. smoldot matches that string
///    against the `id` of the relay-chain spec it was handed and refuses to treat the chain as a
///    parachain if they differ; zombienet gets this right, so this is belt-and-braces (and guards
///    against a raw `polkadot-parachain build-spec` spec, which writes the *chain argument* -
///    `bridge-hub-rococo-local` - and would never match);
/// 3. **drops `protocolId`**, which smoldot ignores and warns about on every start.
///
/// A parachain spec must also carry a non-null `para_id`: smoldot treats a chain as a parachain
/// only when *both* `relay_chain` and `para_id` are present, and would otherwise try to warp-sync a
/// chain that has no GRANDPA of its own. `polkadot-parachain build-spec` emits `para_id: null`
/// nowadays and zombienet fills it in, so this is asserted rather than fixed up - if it trips, the
/// spec did not come from zombienet.
fn smoldot_chain_spec(
	zombienet_spec: &Path,
	out: &Path,
	bootnode: &str,
	relay_chain_id: Option<&str>,
) -> Result<(), anyhow::Error> {
	let raw = std::fs::read_to_string(zombienet_spec)
		.map_err(|e| anyhow!("failed to read chain spec {}: {e}", zombienet_spec.display()))?;
	let mut spec: serde_json::Value = serde_json::from_str(&raw)
		.map_err(|e| anyhow!("chain spec {} is not valid JSON: {e}", zombienet_spec.display()))?;
	let obj = spec
		.as_object_mut()
		.ok_or_else(|| anyhow!("chain spec {} is not a JSON object", zombienet_spec.display()))?;

	let mut boot_nodes: Vec<String> = obj
		.get("bootNodes")
		.and_then(|v| v.as_array())
		.map(|list| list.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
		.unwrap_or_default();
	if !boot_nodes.iter().any(|addr| addr == bootnode) {
		boot_nodes.push(bootnode.to_owned());
	}
	obj.insert("bootNodes".into(), serde_json::json!(boot_nodes));
	obj.remove("protocolId");

	if let Some(relay_chain_id) = relay_chain_id {
		if obj.get("para_id").map(|v| v.is_null()).unwrap_or(true) {
			return Err(anyhow!(
				"parachain spec {} has no `para_id`; smoldot needs both `para_id` and \
				 `relay_chain` to treat a chain as a parachain",
				zombienet_spec.display(),
			));
		}
		obj.insert("relay_chain".into(), serde_json::json!(relay_chain_id));
	}

	if let Some(parent) = out.parent() {
		std::fs::create_dir_all(parent)?;
	}
	std::fs::write(out, serde_json::to_vec(&spec)?)
		.map_err(|e| anyhow!("failed to write smoldot chain spec {}: {e}", out.display()))?;
	Ok(())
}

/// Resolves a chain-spec path reported by zombienet.
///
/// `Relaychain::chain_spec_path` / `Parachain::chain_spec_path` are relative to the network's base
/// directory (zombienet reads them through a filesystem scoped to it), so they are unusable as-is
/// from outside.
fn resolve_spec_path(base_dir: &Path, spec: &Path) -> PathBuf {
	if spec.is_absolute() {
		spec.to_path_buf()
	} else {
		base_dir.join(spec)
	}
}

/// Reads the `id` field of a chain specification.
fn chain_spec_id(path: &Path) -> Result<String, anyhow::Error> {
	let raw = std::fs::read_to_string(path)
		.map_err(|e| anyhow!("failed to read chain spec {}: {e}", path.display()))?;
	let spec: serde_json::Value = serde_json::from_str(&raw)?;
	spec.get("id")
		.and_then(|v| v.as_str())
		.map(str::to_owned)
		.ok_or_else(|| anyhow!("chain spec {} has no `id`", path.display()))
}

/// Spawns a smoldot sidecar for one chain of `network` and waits until it can answer with a
/// finalized block.
///
/// The sidecar is given `127.0.0.1:0` to listen on and reports the port it actually bound, which
/// avoids racing zombienet (and other sidecars) for a fixed port.
pub async fn spawn(
	network: &Network<LocalFileSystem>,
	target: Target<'_>,
	name: &str,
) -> Result<LightClient, anyhow::Error> {
	let base_dir = PathBuf::from(
		network.base_dir().ok_or_else(|| anyhow!("zombienet network has no base dir"))?,
	);
	let out_dir = base_dir.join("smoldot");

	let relay_spec = resolve_spec_path(&base_dir, network.relaychain().chain_spec_path());
	let relay_id = chain_spec_id(&relay_spec)?;

	let (chain_spec, relay_chain_spec) = match &target {
		Target::Relay { bootnode } => {
			let multiaddr = network.get_node(*bootnode)?.multiaddr().to_string();
			let out = out_dir.join(format!("{name}.json"));
			smoldot_chain_spec(&relay_spec, &out, &multiaddr, None)?;
			(out, None)
		},
		Target::Para { para_id, bootnode, relay_bootnode } => {
			let para_spec = resolve_spec_path(
				&base_dir,
				network
					.parachain(*para_id)
					.ok_or_else(|| anyhow!("no parachain {para_id} in network"))?
					.chain_spec_path()
					.ok_or_else(|| anyhow!("parachain {para_id} has no chain spec path"))?,
			);

			let para_out = out_dir.join(format!("{name}.json"));
			let para_multiaddr = network.get_node(*bootnode)?.multiaddr().to_string();
			smoldot_chain_spec(&para_spec, &para_out, &para_multiaddr, Some(&relay_id))?;

			let relay_out = out_dir.join(format!("{name}-relay.json"));
			let relay_multiaddr = network.get_node(*relay_bootnode)?.multiaddr().to_string();
			smoldot_chain_spec(&relay_spec, &relay_out, &relay_multiaddr, None)?;

			(para_out, Some(relay_out))
		},
	};

	let log_path = out_dir.join(format!("{name}.log"));
	let log = std::fs::File::create(&log_path)
		.map_err(|e| anyhow!("failed to create sidecar log {}: {e}", log_path.display()))?;

	let mut args = vec!["127.0.0.1:0".to_string(), chain_spec.display().to_string()];
	if let Some(relay_chain_spec) = &relay_chain_spec {
		args.push(relay_chain_spec.display().to_string());
	}
	log::info!("Spawning smoldot sidecar `{name}`: json-rpc-server {}", args.join(" "));

	let child = Command::new(sidecar_binary())
		.args(&args)
		.env("RUST_LOG", SIDECAR_RUST_LOG)
		// env_logger writes to stderr; keep the file so a failing test can show it.
		.stdout(Stdio::from(log.try_clone()?))
		.stderr(Stdio::from(log))
		.kill_on_drop(true)
		.spawn()
		.map_err(|e| {
			anyhow!(
				"failed to spawn smoldot sidecar `{}` ({}): {e}. Build it with \
				 `cargo build --release -p smoldot-light --example json-rpc-server` and point \
				 SMOLDOT_JSON_RPC_SERVER_BINARY at it",
				name,
				sidecar_binary().display(),
			)
		})?;

	let uri = wait_for_listening_uri(&log_path, name).await?;
	let light_client = LightClient { _child: child, uri, log_path };
	wait_until_synced(&light_client, name).await?;
	Ok(light_client)
}

async fn wait_for_listening_uri(log_path: &Path, name: &str) -> Result<String, anyhow::Error> {
	const NEEDLE: &str = "JSON-RPC server listening on ";
	let deadline = Instant::now() + LISTEN_TIMEOUT;
	loop {
		if let Ok(log) = std::fs::read_to_string(log_path) {
			if let Some(uri) = log
				.lines()
				.filter_map(|line| line.split_once(NEEDLE).map(|(_, rest)| rest.trim()))
				.next()
			{
				log::info!("smoldot sidecar `{name}` listening on {uri}");
				return Ok(uri.to_string());
			}
		}
		if Instant::now() >= deadline {
			return Err(anyhow!(
				"smoldot sidecar `{name}` did not report a listening address within \
				 {LISTEN_TIMEOUT:?}; see {}",
				log_path.display(),
			));
		}
		sleep(Duration::from_millis(200)).await;
	}
}

/// A finalized block above genesis is the readiness signal for both chain kinds: it means the
/// light client is following the chain rather than still sitting on the spec's starting point. A
/// relayer started before that just fails its first reads.
async fn wait_until_synced(light_client: &LightClient, name: &str) -> Result<(), anyhow::Error> {
	let deadline = Instant::now() + SYNC_TIMEOUT;
	let rpc = loop {
		match RpcClient::from_insecure_url(light_client.uri()).await {
			Ok(rpc) => break rpc,
			Err(e) if Instant::now() < deadline => {
				log::trace!("smoldot sidecar `{name}` not accepting connections yet: {e}");
				sleep(Duration::from_millis(500)).await;
			},
			Err(e) =>
				return Err(anyhow!(
					"could not connect to smoldot sidecar `{name}` at {} within {SYNC_TIMEOUT:?}: \
					 {e}; see {}",
					light_client.uri(),
					light_client.log_path().display(),
				)),
		}
	};

	loop {
		if let Some(number) = finalized_number(&rpc).await {
			if number >= 1 {
				log::info!("smoldot sidecar `{name}` synced: finalized #{number}");
				return Ok(());
			}
		}
		if Instant::now() >= deadline {
			return Err(anyhow!(
				"smoldot sidecar `{name}` did not reach a finalized block above genesis within \
				 {SYNC_TIMEOUT:?}; see {}",
				light_client.log_path().display(),
			));
		}
		sleep(Duration::from_secs(2)).await;
	}
}

/// Best-effort read of the sidecar's finalized block number. `None` on any transport or shape
/// problem, so the caller can simply keep polling until its deadline.
async fn finalized_number(rpc: &RpcClient) -> Option<u64> {
	let hash: String = rpc.request("chain_getFinalizedHead", RpcParams::new()).await.ok()?;
	let mut params = RpcParams::new();
	params.push(&hash).ok()?;
	let header: serde_json::Value = rpc.request("chain_getHeader", params).await.ok()?;
	let number = header.get("number")?.as_str()?;
	u64::from_str_radix(number.trim_start_matches("0x"), 16).ok()
}
