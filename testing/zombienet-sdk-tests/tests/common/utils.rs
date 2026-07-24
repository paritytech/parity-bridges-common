// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Generic, runtime-agnostic helpers shared by every bridge pair: reorg-tolerant extrinsic
//! submission, balance/header queries, a polling retry combinator, and the zombienet
//! network-spawn / genesis-override helpers their `environment` modules build on.

use anyhow::anyhow;
use codec::Decode;
use std::{future::Future, time::Duration};
use subxt::{config::DefaultExtrinsicParamsBuilder, tx::Payload, OnlineClient, PolkadotConfig};
use subxt_signer::sr25519::{dev, Keypair};
use tokio::time::{sleep, timeout_at, Instant};
use zombienet_sdk::{GlobalSettingsBuilder, LocalFileSystem, Network, NetworkConfig};

/// Whether `e` is a transient reorg failure on the reorgy asset/bridge hubs, safe to retry (nothing
/// durable was committed). subxt only exposes these as `Display` text, so we match on it: a
/// pruned/reorged block (`discarded`, `unknown Block`, `no longer be found`, `non-finalized fork`)
/// or a nonce not yet reflecting a just-submitted predecessor (`Invalid Transaction`).
fn is_transient_reorg_error(e: &subxt::Error) -> bool {
	let s = e.to_string();
	s.contains("discarded") ||
		s.contains("unknown Block") ||
		s.contains("no longer be found") ||
		s.contains("non-finalized fork") ||
		s.contains("Invalid Transaction")
}

/// Signs, submits and waits for finalized success. The finalized-success wait is reorg-tolerant;
/// the only racy step is building the tx (subxt reads state at the best block, which the fast hubs
/// can prune before it returns) — nothing is submitted then, so we just rebuild and retry.
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

/// Signs, submits and waits only for **in-block** success (the test confirms cross-chain outcomes
/// via [`retry_until`], so inclusion suffices). Reorg-tolerant: a pruned best block while building
/// is retried, a retracted in-block report is re-watched, and a tx re-validated `Invalid`/`Dropped`
/// by a reorg is rebuilt and resubmitted.
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
			// Pre-pool failure (nothing submitted): pruned best block, or a nonce not yet
			// reflecting a just-submitted predecessor. Retry with a fresh build/nonce.
			Err(e) if attempt < ATTEMPTS && is_transient_reorg_error(&e) => {
				sleep(Duration::from_secs(3)).await;
				continue 'attempts;
			},
			Err(e) => return Err(e.into()),
		};
		loop {
			let status = match progress.next().await {
				Some(Ok(status)) => status,
				// Status subscription failed because its block was reorged away; rebuild +
				// resubmit.
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
						// In-block report retracted by a reorg; keep watching for re-inclusion.
						Err(e) if is_transient_reorg_error(&e) => continue,
						Err(e) => return Err(e.into()),
					}
				},
				// A reorg re-validated the tx as Invalid/Dropped/Error, so it never reached the
				// canonical chain and its nonce is unconsumed: rebuild and resubmit.
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

/// Polls `f` every `4s` until it yields `Some`, or `timeout` elapses.
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
		sleep(Duration::from_secs(4)).await;
	}
}

// ---------------------------------------------------------------------------------------------
// Dev-account helpers.
// ---------------------------------------------------------------------------------------------

/// Account id of a dev signer, as a `subxt` `AccountId32`.
pub fn dev_account(keypair: &Keypair) -> subxt::utils::AccountId32 {
	subxt::utils::AccountId32(keypair.public_key().0)
}

/// 32-byte public key of a dev signer.
pub fn dev_public(keypair: &Keypair) -> [u8; 32] {
	keypair.public_key().0
}

// ---------------------------------------------------------------------------------------------
// Network spawn & genesis helpers (shared by every bridge pair's `environment`).
// ---------------------------------------------------------------------------------------------

/// Shared zombienet global settings: disable `tear_down_on_failure` so a transient, load-induced
/// node-monitor timeout doesn't tear the network down, and allow 600s per node spawn.
pub fn global_settings(settings: GlobalSettingsBuilder) -> GlobalSettingsBuilder {
	settings.with_tear_down_on_failure(false).with_node_spawn_timeout(600)
}

/// Collapses zombienet network-config build errors into a single `anyhow::Error`.
pub fn config_errs(errs: Vec<anyhow::Error>) -> anyhow::Error {
	anyhow!(
		"network config errors: {}",
		errs.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; ")
	)
}

/// Spawns one network with the given `spawn_fn` (native or docker provider), retrying the flaky
/// zombienet chain-spec panic (`is_raw()` unwraps a truncated spec read and panics with `EOF while
/// parsing ...`). Rebuilds the config each attempt for a fresh namespace; a genuine `Err` is
/// retried too and surfaces after the last attempt.
pub async fn spawn_with_retry<F, Fut, E>(
	spawn_fn: F,
	config_fn: impl Fn() -> Result<NetworkConfig, anyhow::Error>,
	name: &str,
) -> Result<Network<LocalFileSystem>, anyhow::Error>
where
	F: Fn(NetworkConfig) -> Fut,
	Fut: Future<Output = Result<Network<LocalFileSystem>, E>> + Send + 'static,
	E: std::fmt::Display + Send + 'static,
{
	const MAX_ATTEMPTS: usize = 3;
	let mut last_err = String::new();
	for attempt in 1..=MAX_ATTEMPTS {
		let config = config_fn()?;
		// Run in a task so a panic in zombienet is captured as a `JoinError`, not unwound.
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

/// Genesis `balances` override for a bridge hub. `with_genesis_overrides` *replaces* the
/// `balances.balances` array (it doesn't append), so we re-list the well-known dev accounts
/// (collators + relayer signers) with `endowment`, then fund the `sovereign_accounts` (sovereign /
/// reward accounts) with `sovereign_funding`.
pub fn bridge_hub_balances(
	endowment: u128,
	sovereign_accounts: &[&str],
	sovereign_funding: u128,
) -> serde_json::Value {
	let mut balances: Vec<serde_json::Value> =
		[dev::alice(), dev::bob(), dev::charlie(), dev::dave(), dev::eve(), dev::ferdie()]
			.iter()
			.map(|k| serde_json::json!([dev_account(k).to_string(), endowment]))
			.collect();
	for account in sovereign_accounts {
		balances.push(serde_json::json!([*account, sovereign_funding]));
	}
	serde_json::json!({ "balances": { "balances": balances } })
}

/// Waits (up to 10 minutes) until the native free balance of `account` on `client` exceeds
/// `initial + min_increase`.
pub async fn wait_for_native_increase(
	client: &OnlineClient<PolkadotConfig>,
	account: [u8; 32],
	initial: u128,
	min_increase: u128,
) -> Result<(), anyhow::Error> {
	retry_until(Duration::from_secs(600), || {
		let client = client.clone();
		async move {
			let balance = free_balance_at(&client, account).await?;
			Ok((balance > initial + min_increase).then_some(()))
		}
	})
	.await
}
