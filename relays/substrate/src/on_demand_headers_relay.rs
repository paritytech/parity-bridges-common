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

//! On-demand Substrate -> Substrate headers relay.

use std::{future::Future, pin::Pin};

/// On-demand headers relay future.
type OnDemandHeadersRelayFuture = Pin<Box<dyn Future<Output = ()> + 'static + Send>>;
/// On-demand headers relay start function.
type StartOnDemandHeadersRelay = Box<dyn Fn() -> OnDemandHeadersRelayFuture + 'static + Send>;

/// On-demand Substrate <-> Substrate headers relay.
///
/// This relay may be started by messages whenever some other relay (e.g. messages relay) needs more
/// headers to be relayed to continue its regular work. When enough headers are relayed, on-demand
/// relay may be deactivated.
pub struct OnDemandHeadersRelay {
	/// Name of headers relay to use in logs.
	name: String,
	/// Function that returns headers relay future.
	run_headers_relay: StartOnDemandHeadersRelay,
	/// Active headers relay task.
	active_headers_relay: Option<async_std::task::JoinHandle<()>>,
}

impl OnDemandHeadersRelay {
	/// Create new on-demand headers relay.
	pub fn new(name: String, run_headers_relay: StartOnDemandHeadersRelay) -> Self {
		OnDemandHeadersRelay {
			name,
			run_headers_relay,
			active_headers_relay: None,
		}
	}

	/// Activate or deactivate relay.
	pub async fn activate(&mut self, activate: bool) {
		match (activate, self.active_headers_relay.is_some()) {
			(true, false) => {
				let name = self.name.clone();
				let headers_relay_future = (self.run_headers_relay)();
				let active_headers_relay = async_std::task::spawn(async move {
					log::info!(target: "bridge", "Starting on-demand {} headers relay", name);
					headers_relay_future.await;
					log::trace!(target: "bridge", "On-demand {} headers relay has been stopped", name);
				});
				self.active_headers_relay = Some(active_headers_relay);
			}
			(false, true) => {
				log::trace!(target: "bridge", "Cancelling on-demand {} headers relay", self.name);
				self.active_headers_relay
					.take()
					.expect("guaranteed by match expression")
					.cancel()
					.await;
				log::info!(target: "bridge", "Cancelled on-demand {} headers relay", self.name);
			}
			_ => (),
		}
	}
}
