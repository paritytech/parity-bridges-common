// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Generic, runtime-agnostic `subxt` helpers shared by every bridge pair: reorg-tolerant extrinsic
//! submission, balance/header queries, block-height waits and a polling retry combinator.

use anyhow::anyhow;
use codec::Decode;
use std::{future::Future, time::Duration};
use subxt::{config::DefaultExtrinsicParamsBuilder, tx::Payload, OnlineClient, PolkadotConfig};
use subxt_signer::sr25519::Keypair;
use tokio::time::{sleep, timeout_at, Instant};

/// Whether `e` is a transient failure caused by the reorgy asset/bridge hubs, safe to retry by
/// rebuilding/resubmitting (or re-watching) the transaction: nothing durable was committed.
///
/// subxt surfaces these as distinct error strings, so we match on their `Display` text:
///   * `discarded` / `unknown Block` — the best block read while building the tx (or the block a
///     status referenced) was pruned;
///   * `no longer be found` / `non-finalized fork` — subxt's `TransactionError::BlockNotFound`: the
///     block that had just reported the tx in-block was reorged away;
///   * `Invalid Transaction` — a just-submitted predecessor from the same signer is not yet
///     reflected in the queried nonce.
fn is_transient_reorg_error(e: &subxt::Error) -> bool {
	let s = e.to_string();
	s.contains("discarded") ||
		s.contains("unknown Block") ||
		s.contains("no longer be found") ||
		s.contains("non-finalized fork") ||
		s.contains("Invalid Transaction")
}

/// Signs `call` with `signer`, submits it and waits for finalized success.
///
/// `wait_for_finalized_success` is reorg-tolerant: it watches the transaction through best-block
/// retractions until it lands in a finalized block. The only racy step is building the transaction
/// (subxt reads the nonce/runtime at the best block, which the fast asset-hub collator can prune
/// before the call returns — surfacing as `unknown Block`/`State already discarded`); since nothing
/// is submitted when that happens, we simply retry building it against fresh state.
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

/// Signs `call` with `signer`, submits it and waits only for **in-block** success (not finality).
///
/// Used for the asset-hub asset transfers; the test verifies the cross-chain outcomes via
/// [`retry_until`], so inclusion is sufficient. Tolerant of the asset hubs' reorgs: a pruned best
/// block while building the tx (`unknown Block`) is retried (nothing was submitted), a retracted
/// in-block report is handled by re-watching the same tx, and a tx that a reorg re-validates as
/// `Invalid`/`Dropped` (so it never reached the canonical chain) is rebuilt and resubmitted.
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
			// Transient pre-pool failures (nothing submitted): the best block read while building
			// the tx can be pruned (`unknown Block`/`discarded`), or a just-submitted
			// predecessor from the same signer may not be reflected in the queried nonce yet
			// (`Invalid Transaction`). A short wait lets the node catch up; a fresh nonce is
			// queried on the next attempt.
			Err(e) if attempt < ATTEMPTS && is_transient_reorg_error(&e) => {
				sleep(Duration::from_secs(3)).await;
				continue 'attempts;
			},
			Err(e) => return Err(e.into()),
		};
		loop {
			let status = match progress.next().await {
				Some(Ok(status)) => status,
				// The status subscription itself can fail transiently when the referenced block is
				// reorged away (`BlockNotFound` etc.); nothing is committed, so rebuild + resubmit.
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
						// in-block report retracted by a reorg (`discarded`/`unknown Block`/
						// `BlockNotFound`); keep watching the same tx for re-inclusion.
						Err(e) if is_transient_reorg_error(&e) => continue,
						Err(e) => return Err(e.into()),
					}
				},
				// On the reorgy asset hubs the node re-validates pending txs against a new best
				// chain and can report this tx `Invalid`/`Dropped`/`Error` (e.g. the fork it
				// sat in was pruned). Such a tx is not on the canonical chain, so its nonce is
				// unconsumed and it is safe to rebuild against fresh state and resubmit rather
				// than failing the test.
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

/// Waits until `client` reports a best block of at least `height`, or `timeout` elapses.
///
/// Uses best (not finalized) blocks: confirms block production without depending on the finality
/// chain.
// Currently unused: `init_bridge` gates on the bridge hubs' first *finalized* block instead (a
// stronger, faster signal). Kept as a helper for ad-hoc best-block waits.
#[allow(dead_code)]
pub async fn wait_for_block_height(
	client: &OnlineClient<PolkadotConfig>,
	height: u32,
	timeout: Duration,
) -> Result<(), anyhow::Error> {
	let mut sub = client.blocks().subscribe_best().await?;
	let deadline = Instant::now() + timeout;
	while let Ok(Some(block)) = timeout_at(deadline, sub.next()).await {
		if block?.number() >= height {
			return Ok(());
		}
	}
	Err(anyhow!("timeout waiting for block height {height}"))
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

#[allow(dead_code)]
fn parse_account(ss58: &str) -> Result<subxt::utils::AccountId32, anyhow::Error> {
	ss58.parse::<subxt::utils::AccountId32>()
		.map_err(|e| anyhow!("invalid SS58 account {ss58}: {e:?}"))
}
