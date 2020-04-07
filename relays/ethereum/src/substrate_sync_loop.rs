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

use crate::ethereum_client;
use crate::substrate_client;
use crate::sync::HeadersSyncParams;
use futures::future::FutureExt;
use parity_crypto::publickey::KeyPair;

/// Substrate synchronization parameters.
pub struct SubstrateSyncParams {
	/// Ethereum RPC host.
	pub eth_host: String,
	/// Ethereum RPC port.
	pub eth_port: u16,
	/// Ethereum transactions signer.
	pub eth_signer: KeyPair,
	/// Substrate RPC host.
	pub sub_host: String,
	/// Substrate RPC port.
	pub sub_port: u16,
	/// Synchronization parameters.
	pub sync_params: HeadersSyncParams,
}

impl Default for SubstrateSyncParams {
	fn default() -> Self {
		SubstrateSyncParams {
			eth_host: "localhost".into(),
			eth_port: 8545,
			// that the account that has a lot of ether when we run instant seal engine
			// address: 0x00a329c0648769a73afac7f9381e08fb43dbea72
			// secret: 0x4d5db4107d237df6a3d58ee5f70ae63d73d7658d4026f2eefd2f204c81682cb7
			eth_signer: KeyPair::from_secret_slice(
				&[0x4d, 0x5d, 0xb4, 0x10, 0x7d, 0x23, 0x7d, 0xf6, 0xa3, 0xd5, 0x8e, 0xe5, 0xf7, 0x0a,
				0xe6, 0x3d, 0x73, 0xd7, 0x65, 0x8d, 0x40, 0x26, 0xf2, 0xee, 0xfd, 0x2f, 0x20, 0x4c,
				0x81, 0x68, 0x2c, 0xb7]
			).expect("secret is hardcoded, thus valid; qed"),
			sub_host: "localhost".into(),
			sub_port: 9933,
			sync_params: Default::default(),
		}
	}
}

/// Run Substrate headers synchronization.
pub fn run(params: SubstrateSyncParams) {
/*	let mut local_pool = futures::executor::LocalPool::new();
	let mut progress_context = (std::time::Instant::now(), None, None);

	local_pool.run_until(async move {
		let eth_uri = format!("http://{}:{}", params.eth_host, params.eth_port);
		let sub_uri = format!("http://{}:{}", params.sub_host, params.sub_port);

		let mut sub_sync = crate::sync::HeadersSync::new(params.sync_params);
		let mut stall_countdown = None;

		let mut eth_maybe_client = None;
		let mut eth_best_block_number_required = false;
		let eth_best_block_number_future = ethereum_client::best_block_number(ethereum_client::client(&eth_uri)).fuse();
		let eth_tick_stream = interval(ETHEREUM_TICK_INTERVAL_MS).fuse();

		let mut sub_maybe_client = None;
		let mut sub_best_block_required = false;
		let sub_best_block_future =
			substrate_client::best_ethereum_block(substrate_client::client(&sub_uri)).fuse();
		let sub_tick_stream = interval(SUBSTRATE_TICK_INTERVAL_MS).fuse();

		futures::pin_mut!(
			eth_best_block_number_future,
			eth_tick_stream,
			sub_best_block_future,
			sub_tick_stream
		);

		loop {
			futures::select! {
				(eth_client, eth_best_block_number) = eth_best_block_number_future => {
					eth_best_block_number_required = false;

					process_future_result(
						&mut eth_maybe_client,
						eth_client,
						eth_best_block_number,
						|eth_best_block_number| eth_sync.source_best_header_number_response(eth_best_block_number),
						&mut eth_go_offline_future,
						|eth_client| delay(CONNECTION_ERROR_DELAY_MS, eth_client),
						"Error retrieving best header number from Ethereum number",
					);
				},
				eth_client = eth_go_offline_future => {
					eth_maybe_client = Some(eth_client);
				},
				_ = eth_tick_stream.next() => {
					if eth_sync.is_almost_synced() {
						eth_best_block_number_required = true;
					}
				},
				(sub_client, sub_best_block) = sub_best_block_future => {
					sub_best_block_required = false;

					process_future_result(
						&mut sub_maybe_client,
						sub_client,
						sub_best_block,
						|sub_best_block| {
							let head_updated = eth_sync.target_best_header_response(sub_best_block);
							match head_updated {
								// IF head is updated AND there are still our transactions:
								// => restart stall countdown timer
								true if eth_sync.headers().headers_in_status(EthereumHeaderStatus::Submitted) != 0 =>
									stall_countdown = Some(std::time::Instant::now()),
								// IF head is updated AND there are no our transactions:
								// => stop stall countdown timer
								true => stall_countdown = None,
								// IF head is not updated AND stall countdown is not yet completed
								// => do nothing
								false if stall_countdown
									.map(|stall_countdown| std::time::Instant::now() - stall_countdown <
										std::time::Duration::from_millis(STALL_SYNC_TIMEOUT_MS))
									.unwrap_or(true)
									=> (),
								// IF head is not updated AND stall countdown has completed
								// => restart sync
								false => {
									log::info!(
										target: "bridge",
										"Possible Substrate fork detected. Restarting Ethereum headers synchronization.",
									);
									stall_countdown = None;
									eth_sync.restart();
								},
							}
						},
						&mut sub_go_offline_future,
						|sub_client| delay(CONNECTION_ERROR_DELAY_MS, sub_client),
						"Error retrieving best known header from Substrate node",
					);
				},
				sub_client = sub_go_offline_future => {
					sub_maybe_client = Some(sub_client);
				},
				_ = sub_tick_stream.next() => {
					sub_best_block_required = true;
				},
			}
		}
	});*/
}

