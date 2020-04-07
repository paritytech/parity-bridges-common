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
	/// Maximal number of ethereum headers to pre-download.
	pub max_future_headers_to_download: usize,
	/// Maximal number of active (we believe) submit header transactions.
	pub max_headers_in_submitted_status: usize,
	/// Maximal number of headers in single submit request.
	pub max_headers_in_single_submit: usize,
	/// Maximal total headers size in single submit request.
	pub max_headers_size_in_single_submit: usize,
	/// We only may store and accept (from Ethereum node) headers that have
	/// number >= than best_substrate_header.number - prune_depth.
	pub prune_depth: u64,
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
			max_future_headers_to_download: 128,
			max_headers_in_submitted_status: 128,
			max_headers_in_single_submit: 32,
			max_headers_size_in_single_submit: 131_072,
			prune_depth: 4096,
		}
	}
}

/// Run Substrate headers synchronization.
pub fn run(params: SubstrateSyncParams) {
	let mut local_pool = futures::executor::LocalPool::new();
//	let mut progress_context = (std::time::Instant::now(), None, None);

	local_pool.run_until(async move {
		let eth_uri = format!("http://{}:{}", params.eth_host, params.eth_port);
		let sub_uri = format!("http://{}:{}", params.sub_host, params.sub_port);

		let _eth_best_block_number_future = ethereum_client::best_block_number(ethereum_client::client(&eth_uri)).fuse();

		let _sub_best_block_future =
			substrate_client::best_ethereum_block(substrate_client::client(&sub_uri)).fuse();

		loop {

		}
	});
}

