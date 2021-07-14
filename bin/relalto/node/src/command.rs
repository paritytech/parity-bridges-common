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

use sc_cli::{SubstrateCli, RuntimeVersion, Role};
use futures::future::TryFutureExt;
/*
#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error(transparent)]
	PolkadotService(#[from] crate::service::Error),

	#[error(transparent)]
	SubstrateCli(#[from] sc_cli::Error),

	#[error(transparent)]
	SubstrateService(sc_service::Error),

	#[error("Other: {0}")]
	Other(String),
}

impl std::convert::From<String> for Error {
	fn from(s: String) -> Self {
		Self::Other(s)
	}
}

type Result<T> = std::result::Result<T, Error>;
*/
fn get_exec_name() -> Option<String> {
	std::env::current_exe()
		.ok()
		.and_then(|pb| pb.file_name().map(|s| s.to_os_string()))
		.and_then(|s| s.into_string().ok())
}

impl SubstrateCli for crate::cli::Cli {
	fn impl_name() -> String { "Parity Relalto".into() }

	fn impl_version() -> String { env!("SUBSTRATE_CLI_IMPL_VERSION").into() }

	fn description() -> String { env!("CARGO_PKG_DESCRIPTION").into() }

	fn author() -> String { env!("CARGO_PKG_AUTHORS").into() }

	fn support_url() -> String { "https://github.com/paritytech/parity-bridges-common/issues/new".into() }

	fn copyright_start_year() -> i32 { 2017 }

	fn executable_name() -> String { "rialto-bridge-node".into() }

	fn load_spec(&self, id: &str) -> std::result::Result<Box<dyn sc_service::ChainSpec>, String> {
		Ok(Box::new(
			match id {
				"" | "dev" => crate::service::chain_spec::Alternative::Development,
				"local" => crate::service::chain_spec::Alternative::LocalTestnet,
				_ => return Err(format!("Unsupported chain specification: {}", id)),
			}
			.load(),
		))
	}

	fn native_runtime_version(spec: &Box<dyn sc_service::ChainSpec>) -> &'static RuntimeVersion {
		&relalto_runtime::VERSION
	}
}

/// Parse and run command line arguments
pub fn run() -> Result<(), crate::service::Error> {
	let cli = crate::cli::Cli::from_args();
	sp_core::crypto::set_default_ss58_version(sp_core::crypto::Ss58AddressFormat::Custom(
		relalto_runtime::SS58Prefix::get() as u16,
	));

	match &cli.subcommand {
		Some(crate::cli::Subcommand::BuildSpec(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			Ok(runner.sync_run(|config| {
				cmd.run(config.chain_spec, config.network)
			})?)
		},
		None => {
			let jaeger_agent = None;
			let runner = cli.create_runner(&cli.run)?;
			let grandpa_pause = None;
			let overseer_gen = crate::service::RealOverseerGen;
			let no_beefy = true;
			runner
				.run_node_until_exit(|config| async move {
					match config.role {
						Role::Light => Err(crate::service::Error::Temp("Light is not supported".into())),
						_ => crate::service::build_full(
							config,
							crate::service::IsCollator::No,
							grandpa_pause,
							no_beefy,
							jaeger_agent,
							None,
							overseer_gen,
						).map(|full| full.task_manager).map_err(Into::into),
					}
				})
		}
	}
}
