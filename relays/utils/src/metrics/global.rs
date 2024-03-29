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

//! Global system-wide Prometheus metrics exposed by relays.

use crate::metrics::{
	metric_name, register, Gauge, GaugeVec, Metric, Opts, PrometheusError, Registry,
	StandaloneMetric, F64, U64,
};

use async_std::sync::{Arc, Mutex};
use async_trait::async_trait;
use std::time::Duration;
use sysinfo::{RefreshKind, System};

/// Global metrics update interval.
const UPDATE_INTERVAL: Duration = Duration::from_secs(10);

/// Global Prometheus metrics.
#[derive(Debug, Clone)]
pub struct GlobalMetrics {
	system: Arc<Mutex<System>>,
	system_average_load: GaugeVec<F64>,
	process_cpu_usage_percentage: Gauge<F64>,
	process_memory_usage_bytes: Gauge<U64>,
}

impl GlobalMetrics {
	/// Create and register global metrics.
	pub fn new() -> Result<Self, PrometheusError> {
		Ok(GlobalMetrics {
			system: Arc::new(Mutex::new(System::new_with_specifics(RefreshKind::everything()))),
			system_average_load: GaugeVec::new(
				Opts::new(metric_name(None, "system_average_load"), "System load average"),
				&["over"],
			)?,
			process_cpu_usage_percentage: Gauge::new(
				metric_name(None, "process_cpu_usage_percentage"),
				"Process CPU usage",
			)?,
			process_memory_usage_bytes: Gauge::new(
				metric_name(None, "process_memory_usage_bytes"),
				"Process memory (resident set size) usage",
			)?,
		})
	}
}

impl Metric for GlobalMetrics {
	fn register(&self, registry: &Registry) -> Result<(), PrometheusError> {
		register(self.system_average_load.clone(), registry)?;
		register(self.process_cpu_usage_percentage.clone(), registry)?;
		register(self.process_memory_usage_bytes.clone(), registry)?;
		Ok(())
	}
}

#[async_trait]
impl StandaloneMetric for GlobalMetrics {
	async fn update(&self) {
		// update system-wide metrics
		let mut system = self.system.lock().await;
		let load = sysinfo::System::load_average();
		self.system_average_load.with_label_values(&["1min"]).set(load.one);
		self.system_average_load.with_label_values(&["5min"]).set(load.five);
		self.system_average_load.with_label_values(&["15min"]).set(load.fifteen);

		// update process-related metrics
		let pid = sysinfo::get_current_pid().expect(
			"only fails where pid is unavailable (os=unknown || arch=wasm32);\
				relay is not supposed to run in such MetricsParamss;\
				qed",
		);
		let is_process_refreshed = system.refresh_process(pid);
		match (is_process_refreshed, system.process(pid)) {
			(true, Some(process_info)) => {
				let cpu_usage = process_info.cpu_usage() as f64;
				let memory_usage = process_info.memory() * 1024;
				log::trace!(
					target: "bridge-metrics",
					"Refreshed process metrics: CPU={}, memory={}",
					cpu_usage,
					memory_usage,
				);

				self.process_cpu_usage_percentage.set(if cpu_usage.is_finite() {
					cpu_usage
				} else {
					0f64
				});
				self.process_memory_usage_bytes.set(memory_usage);
			},
			_ => {
				log::warn!(
					target: "bridge-metrics",
					"Failed to refresh process information. Metrics may show obsolete values",
				);
			},
		}
	}

	fn update_interval(&self) -> Duration {
		UPDATE_INTERVAL
	}
}
