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

/// Error type that can signal connection errors.
pub trait MaybeConnectionError {
	/// Returns true if error (maybe) represents connection error.
	fn is_connection_error(&self) -> bool;
}

/// Future that resolves into given value after given timeout.
pub async fn delay<T>(timeout_ms: u64, retval: T) -> T {
	async_std::task::sleep(std::time::Duration::from_millis(timeout_ms)).await;
	retval
}

/// Stream that emits item every `timeout_ms` milliseconds.
pub fn interval(timeout_ms: u64) -> impl futures::Stream<Item = ()> {
	futures::stream::unfold((), move |_| async move {
		delay(timeout_ms, ()).await;
		Some(((), ()))
	})
}

/// Process result of the future that may have been caused by connection failure.
pub fn process_future_result<TClient, TResult, TError, TGoOfflineFuture>(
	maybe_client: &mut Option<TClient>,
	client: TClient,
	result: Result<TResult, TError>,
	on_success: impl FnOnce(TResult),
	go_offline_future: &mut std::pin::Pin<&mut futures::future::Fuse<TGoOfflineFuture>>,
	go_offline: impl FnOnce(TClient) -> TGoOfflineFuture,
	error_pattern: &'static str,
) where
	TError: std::fmt::Debug + MaybeConnectionError,
	TGoOfflineFuture: FutureExt,
{
	match result {
		Ok(result) => {
			*maybe_client = Some(client);
			on_success(result);
		}
		Err(error) => {
			if error.is_connection_error() {
				go_offline_future.set(go_offline(client).fuse());
			} else {
				*maybe_client = Some(client);
			}

			log::error!(target: "bridge", "{}: {:?}", error_pattern, error);
		}
	}
}

/// Print synchronization progress.
pub fn print_sync_progress(
	progress_context: (std::time::Instant, Option<u64>, Option<u64>),
	eth_sync: &crate::ethereum_sync::HeadersSync,
) -> (std::time::Instant, Option<u64>, Option<u64>) {
	let (prev_time, prev_best_header, prev_target_header) = progress_context;
	let now_time = std::time::Instant::now();
	let (now_best_header, now_target_header) = eth_sync.status();

	let need_update = now_time - prev_time > std::time::Duration::from_secs(10)
		|| match (prev_best_header, now_best_header) {
			(Some(prev_best_header), Some(now_best_header)) => now_best_header.0.saturating_sub(prev_best_header) > 10,
			_ => false,
		};
	if !need_update {
		return (prev_time, prev_best_header, prev_target_header);
	}

	log::info!(
		target: "bridge",
		"Synced {:?} of {:?} headers",
		now_best_header.map(|id| id.0),
		now_target_header,
	);
	(now_time, now_best_header.clone().map(|id| id.0), *now_target_header)
}
