// Copyright 2019-2021 Parity Technologies (UK) Ltd.
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

//! Layered Substrate client implementation.

use crate::{Chain, ConnectionParams};

use caching::CachingClient;
use rpc::RpcClient;

pub mod caching;
pub mod client;
pub mod rpc;
pub mod subscription;

pub use client::Client;
pub use subscription::{SharedSubscriptionFactory, Subscription};

/// Type of RPC client with caching support.
pub type RpcWithCachingClient<C> = CachingClient<C, RpcClient<C>>;

/// Creates new RPC client with caching support.
pub async fn rpc_with_caching<C: Chain>(params: ConnectionParams) -> RpcWithCachingClient<C> {
	let rpc = rpc::RpcClient::<C>::new(params).await;
	let caching = caching::CachingClient::new(rpc);
	caching
}
