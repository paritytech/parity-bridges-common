// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

//! `substrate-relay` subprocess driver: locating the binary, spawning long-running relayer
//! processes (killed on drop) and the idempotent, finality-confirmed bridge initialization.

use anyhow::anyhow;
use std::{path::PathBuf, time::Duration};
use subxt::{OnlineClient, PolkadotConfig};
use tokio::{
	process::{Child, Command},
	time::{sleep, Instant},
};

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

pub fn spawn_relayer(args: &[&str]) -> Result<Relayer, anyhow::Error> {
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
/// (non-finalized) header. On a loaded host the bridge hub reorgs tens of blocks deep, so the
/// init tx can land on a branch that is briefly best — fooling that check into reporting success —
/// and is then retracted, leaving the canonical chain `Halted`. We therefore drive init from the
/// **finalized** state: before each attempt we check `operating_mode == Normal` at a finalized
/// block, and only stop once that holds. Each attempt re-submits (the relay no-ops fast when it
/// believes it is already initialized), so once a submission finalizes the bridge is operational.
pub async fn init_bridge_confirmed(
	args: &[&str],
	target_client: &OnlineClient<PolkadotConfig>,
	grandpa_pallet: &str,
) -> Result<(), anyhow::Error> {
	// Overall budget to get the bridge operational. Instead of pre-waiting for a fixed finalized
	// height, we drive `init-bridge` and poll the real init signal (`operating_mode == Normal` at a
	// finalized block) until it holds or this deadline elapses. Early attempts submitted while the
	// bridge hub is still reorging may be orphaned, so we re-submit until one finalizes.
	const OVERALL_TIMEOUT: Duration = Duration::from_secs(300);
	// Max time to wait for a single `init-bridge` invocation to finalize before re-checking and
	// re-submitting.
	const PER_ATTEMPT: Duration = Duration::from_secs(90);
	let deadline = Instant::now() + OVERALL_TIMEOUT;
	let mut attempt = 0;
	loop {
		if bridge_operating_mode_normal_at_finalized(target_client, grandpa_pallet)
			.await
			.unwrap_or(false)
		{
			log::info!("bridge {grandpa_pallet} confirmed operational (Normal) at finalized");
			return Ok(());
		}
		if Instant::now() >= deadline {
			return Err(anyhow!(
				"bridge {grandpa_pallet} did not become operational within {OVERALL_TIMEOUT:?}"
			));
		}
		attempt += 1;
		log::info!("init-bridge attempt {attempt}: substrate-relay {}", args.join(" "));
		let mut child = Command::new(relayer_binary())
			.args(args)
			.env("RUST_LOG", RELAYER_RUST_LOG)
			.kill_on_drop(true)
			.spawn()?;
		// Wait for this invocation to finalize, but never run past the overall deadline.
		let attempt_budget = PER_ATTEMPT.min(deadline.saturating_duration_since(Instant::now()));
		if tokio::time::timeout(attempt_budget, child.wait()).await.is_err() {
			log::warn!(
				"init-bridge attempt {attempt} for {grandpa_pallet} not finalized within \
				 {attempt_budget:?}; re-checking finalized state and retrying"
			);
		}
		let _ = child.start_kill();
		let _ = child.wait().await;
		// Brief pause so finality can advance before the next poll/resubmit (without overrunning
		// the overall deadline / hammering the node with rapid connect-disconnect cycles).
		if Instant::now() < deadline {
			sleep(Duration::from_secs(6)).await;
		}
	}
}
