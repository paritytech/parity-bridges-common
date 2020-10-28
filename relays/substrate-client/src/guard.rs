// Copyright 2019-2020 Parity Technologies (UK) Ltd.
// This file is part of Parity Bridges Common.

// Parity Bridges Common is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity Bridges Common is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity Bridges Common.  If not, see <http://www.gnu.org/licenses/>.

//! Module provides a set of guard functions that are running in background threads
//! and are aborting process if some condition fails.

use crate::{Chain, Client};

use async_trait::async_trait;
use num_traits::{CheckedSub, Zero};
use sp_version::RuntimeVersion;
use std::time::{Duration, Instant};

/// Guards environment.
#[async_trait]
pub trait Environment<C: Chain>: Send + Sync + 'static {
	/// Return current runtime version.
	async fn runtime_version(&mut self) -> Result<RuntimeVersion, String>;
	/// Return free native balance of the account on the chain.
	async fn free_native_balance(&mut self, account: C::AccountId) -> Result<C::NativeBalance, String>;

	/// Return current time.
	fn now(&self) -> Instant {
		Instant::now()
	}
	/// Sleep given amount of time.
	async fn sleep(&mut self, duration: Duration) {
		async_std::task::sleep(duration).await
	}
	/// Abort current process. Called when guard condition check fails.
	async fn abort(&mut self) {
		std::process::abort();
	}
}

/// Abort when runtime spec version is different from specified.
pub fn abort_on_spec_version_change<C: Chain>(mut env: impl Environment<C>, expected_spec_version: u32) {
	async_std::task::spawn(async move {
		loop {
			let actual_spec_version = env.runtime_version().await;
			match actual_spec_version {
				Ok(version) if version.spec_version == expected_spec_version => (),
				Ok(version) => {
					log::error!(
						target: "bridge-guard",
						"{} runtime spec version has changed from {} to {}. Aborting relay",
						C::NAME,
						expected_spec_version,
						version.spec_version,
					);

					env.abort().await;
				}
				Err(error) => log::warn!(
					target: "bridge-guard",
					"Failed to read {} runtime version: {:?}. Relay may need to be stopped manually",
					C::NAME,
					error,
				),
			}

			env.sleep(conditions_check_delay::<C>()).await;
		}
	});
}

/// Abort if, during a 24 hours, free balance of given account is decreased at least by given value.
/// Other components may increase (or decrease) balance of account and it WILL affect logic of the guard.
pub fn abort_when_account_balance_decreased<C: Chain>(
	mut env: impl Environment<C>,
	account_id: C::AccountId,
	maximal_decrease: C::NativeBalance,
) {
	const DAY: Duration = Duration::from_secs(60 * 60 * 24);

	async_std::task::spawn(async move {
		let mut interval_start = env.now();
		let mut start_balance = None;
		let mut last_balance = None;

		loop {
			// if 24 hours have passed, start new interval
			let current_time = env.now();
			let duration_since_start = current_time.duration_since(interval_start);
			if duration_since_start > DAY {
				interval_start = current_time;
				start_balance = last_balance;
			}

			// read balance of the account
			let current_balance = env.free_native_balance(account_id.clone()).await;
			let current_balance = match current_balance {
				Ok(current_balance) => Some(current_balance),
				Err(error) => {
					log::warn!(
						target: "bridge-guard",
						"Failed to read {} account {:?} balance: {:?}. Relay may need to be stopped manually",
						C::NAME,
						account_id,
						error,
					);

					last_balance
				}
			};

			// if this is the first balance we were able to read, just continue
			last_balance = current_balance;
			if start_balance.is_none() {
				start_balance = current_balance;
			}

			// check balance difference
			if let (Some(start_balance), Some(current_balance)) = (start_balance, current_balance) {
				let difference = start_balance.checked_sub(&current_balance).unwrap_or_else(Zero::zero);
				if difference > maximal_decrease {
					log::error!(
						target: "bridge-guard",
						"Balance of {} account {:?} has decreased from {:?} to {:?} in {} minutes. Aborting relay",
						C::NAME,
						account_id,
						start_balance,
						current_balance,
						duration_since_start.as_secs() / 60,
					);

					env.abort().await;
				}
			}

			env.sleep(conditions_check_delay::<C>()).await;
		}
	});
}

/// Delay between conditions check.
fn conditions_check_delay<C: Chain>() -> Duration {
	C::AVERAGE_BLOCK_INTERVAL * (10 + rand::random::<u32>() % 10)
}

#[async_trait]
impl<C: Chain> Environment<C> for Client<C> {
	async fn runtime_version(&mut self) -> Result<RuntimeVersion, String> {
		Client::<C>::runtime_version(self).await.map_err(|e| e.to_string())
	}

	async fn free_native_balance(&mut self, account: C::AccountId) -> Result<C::NativeBalance, String> {
		Client::<C>::free_native_balance(self, account)
			.await
			.map_err(|e| e.to_string())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use futures::{
		channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender},
		future::FutureExt,
		stream::StreamExt,
		SinkExt,
	};

	struct TestChain;

	impl bp_runtime::Chain for TestChain {
		type BlockNumber = u32;
		type Hash = sp_core::H256;
		type Hasher = sp_runtime::traits::BlakeTwo256;
		type Header = sp_runtime::generic::Header<u32, sp_runtime::traits::BlakeTwo256>;
	}

	impl Chain for TestChain {
		const NAME: &'static str = "Test";
		const AVERAGE_BLOCK_INTERVAL: Duration = Duration::from_millis(1);

		type AccountId = u32;
		type Index = u32;
		type SignedBlock = ();
		type Call = ();
		type NativeBalance = u32;

		fn account_data_storage_key(_account_id: &u32) -> sp_core::storage::StorageKey {
			unreachable!()
		}
	}

	struct TestEnvironment {
		runtime_version_rx: UnboundedReceiver<RuntimeVersion>,
		free_native_balance_rx: UnboundedReceiver<u32>,
		slept_tx: UnboundedSender<()>,
		aborted_tx: UnboundedSender<()>,
	}

	#[async_trait]
	impl Environment<TestChain> for TestEnvironment {
		async fn runtime_version(&mut self) -> Result<RuntimeVersion, String> {
			Ok(self.runtime_version_rx.next().await.unwrap_or_default())
		}

		async fn free_native_balance(&mut self, _account: u32) -> Result<u32, String> {
			Ok(self.free_native_balance_rx.next().await.unwrap_or_default())
		}

		async fn sleep(&mut self, _duration: Duration) {
			let _ = self.slept_tx.send(()).await;
		}

		async fn abort(&mut self) {
			let _ = self.aborted_tx.send(()).await;
			// simulate process abort :)
			async_std::task::sleep(Duration::from_secs(60)).await;
		}
	}

	#[test]
	fn aborts_when_spec_version_is_changed() {
		async_std::task::block_on(async {
			let (
				(mut runtime_version_tx, runtime_version_rx),
				(_free_native_balance_tx, free_native_balance_rx),
				(slept_tx, mut slept_rx),
				(aborted_tx, mut aborted_rx),
			) = (unbounded(), unbounded(), unbounded(), unbounded());
			abort_on_spec_version_change(
				TestEnvironment {
					runtime_version_rx,
					free_native_balance_rx,
					slept_tx,
					aborted_tx,
				},
				0,
			);

			// client responds with wrong version
			runtime_version_tx
				.send(RuntimeVersion {
					spec_version: 42,
					..Default::default()
				})
				.await
				.unwrap();

			// then the `abort` function is called
			aborted_rx.next().await;
			// and we do not reach the `sleep` function call
			assert!(slept_rx.next().now_or_never().is_none());
		});
	}

	#[test]
	fn does_not_aborts_when_spec_version_is_unchanged() {
		async_std::task::block_on(async {
			let (
				(mut runtime_version_tx, runtime_version_rx),
				(_free_native_balance_tx, free_native_balance_rx),
				(slept_tx, mut slept_rx),
				(aborted_tx, mut aborted_rx),
			) = (unbounded(), unbounded(), unbounded(), unbounded());
			abort_on_spec_version_change(
				TestEnvironment {
					runtime_version_rx,
					free_native_balance_rx,
					slept_tx,
					aborted_tx,
				},
				42,
			);

			// client responds with the same version
			runtime_version_tx
				.send(RuntimeVersion {
					spec_version: 42,
					..Default::default()
				})
				.await
				.unwrap();

			// then the `sleep` function is called
			slept_rx.next().await;
			// and the `abort` function is not called
			assert!(aborted_rx.next().now_or_never().is_none());
		});
	}

	#[test]
	fn aborts_when_balance_is_too_low() {
		async_std::task::block_on(async {
			let (
				(_runtime_version_tx, runtime_version_rx),
				(mut free_native_balance_tx, free_native_balance_rx),
				(slept_tx, mut slept_rx),
				(aborted_tx, mut aborted_rx),
			) = (unbounded(), unbounded(), unbounded(), unbounded());
			abort_when_account_balance_decreased(
				TestEnvironment {
					runtime_version_rx,
					free_native_balance_rx,
					slept_tx,
					aborted_tx,
				},
				0,
				100,
			);

			// client responds with initial balance
			free_native_balance_tx.send(1000).await.unwrap();

			// then the guard sleeps
			slept_rx.next().await;

			// and then client responds with updated balance, which is too low
			free_native_balance_tx.send(899).await.unwrap();

			// then the `abort` function is called
			aborted_rx.next().await;
			// and we do not reach next `sleep` function call
			assert!(slept_rx.next().now_or_never().is_none());
		});
	}

	#[test]
	fn does_not_aborts_when_balance_is_enough() {
		async_std::task::block_on(async {
			let (
				(_runtime_version_tx, runtime_version_rx),
				(mut free_native_balance_tx, free_native_balance_rx),
				(slept_tx, mut slept_rx),
				(aborted_tx, mut aborted_rx),
			) = (unbounded(), unbounded(), unbounded(), unbounded());
			abort_when_account_balance_decreased(
				TestEnvironment {
					runtime_version_rx,
					free_native_balance_rx,
					slept_tx,
					aborted_tx,
				},
				0,
				100,
			);

			// client responds with initial balance
			free_native_balance_tx.send(1000).await.unwrap();

			// then the guard sleeps
			slept_rx.next().await;

			// and then client responds with updated balance, which is enough
			free_native_balance_tx.send(950).await.unwrap();

			// then the `sleep` function is called
			slept_rx.next().await;
			// and `abort` is not called
			assert!(aborted_rx.next().now_or_never().is_none());
		});
	}
}
