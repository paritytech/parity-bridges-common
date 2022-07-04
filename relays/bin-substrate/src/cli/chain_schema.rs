// Copyright 2019-2022 Parity Technologies (UK) Ltd.
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

use sp_core::Pair;
use structopt::StructOpt;
use strum::{EnumString, EnumVariantNames};

use crate::cli::CliChain;
use relay_substrate_client::ChainRuntimeVersion;
use substrate_relay_helper::TransactionParams;

#[doc = "Runtime version params."]
#[derive(StructOpt, Debug, PartialEq, Eq, Clone, Copy, EnumString, EnumVariantNames)]
pub enum RuntimeVersionType {
	/// Auto query version from chain
	Auto,
	/// Custom `spec_version` and `transaction_version`
	Custom,
	/// Read version from bundle dependencies directly.
	Bundle,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct RuntimeVersionParams {
	pub prefix: &'static str,

	pub version_mode: RuntimeVersionType,
	pub spec_version: Option<u32>,
	pub transaction_version: Option<u32>,
}

impl RuntimeVersionParams {
	/// Converts self into `ChainRuntimeVersion`.
	pub fn into_runtime_version(
		self,
		bundle_runtime_version: Option<sp_version::RuntimeVersion>,
	) -> anyhow::Result<ChainRuntimeVersion> {
		Ok(match self.version_mode {
			RuntimeVersionType::Auto => ChainRuntimeVersion::Auto,
			RuntimeVersionType::Custom => {
				let except_spec_version = self.spec_version.ok_or_else(|| {
					anyhow::Error::msg(format!(
						"The {}-spec-version is required when choose custom mode",
						self.prefix
					))
				})?;
				let except_transaction_version = self.transaction_version.ok_or_else(|| {
					anyhow::Error::msg(format!(
						"The {}-transaction-version is required when choose custom mode",
						self.prefix
					))
				})?;
				ChainRuntimeVersion::Custom(except_spec_version, except_transaction_version)
			},
			RuntimeVersionType::Bundle => match bundle_runtime_version {
				Some(runtime_version) => ChainRuntimeVersion::Custom(
					runtime_version.spec_version,
					runtime_version.transaction_version,
				),
				None => ChainRuntimeVersion::Auto,
			},
		})
	}
}

/// Create chain-specific set of runtime version parameters.
#[macro_export]
macro_rules! declare_chain_runtime_version_params_cli_schema {
	($chain:ident, $chain_prefix:ident) => {
		paste::item! {
			#[doc = $chain " runtime version params."]
			#[derive(StructOpt, Debug, PartialEq, Eq, Clone, Copy)]
			pub struct [<$chain RuntimeVersionParams>] {
				#[doc = "The type of runtime version for chain " $chain]
				#[structopt(long, default_value = "Bundle")]
				pub [<$chain_prefix _version_mode>]: RuntimeVersionType,
				#[doc = "The custom sepc_version for chain " $chain]
				#[structopt(long)]
				pub [<$chain_prefix _spec_version>]: Option<u32>,
				#[doc = "The custom transaction_version for chain " $chain]
				#[structopt(long)]
				pub [<$chain_prefix _transaction_version>]: Option<u32>,
			}

			impl From<[<$chain RuntimeVersionParams>]> for RuntimeVersionParams {
				fn from(item: [<$chain RuntimeVersionParams>]) -> RuntimeVersionParams {
					RuntimeVersionParams {
						prefix: stringify!([<$chain_prefix>]),

						version_mode: item.[<$chain_prefix _version_mode>],
						spec_version: item.[<$chain_prefix _spec_version>],
						transaction_version: item.[<$chain_prefix _transaction_version>],
					}
				}
			}
		}
	};
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ConnectionParams {
	pub host: String,
	pub port: u16,
	pub secure: bool,
	pub runtime_version: RuntimeVersionParams,
}

impl ConnectionParams {
	/// Returns `true` if version guard can be started.
	///
	/// There's no reason to run version guard when version mode is set to `Auto`. It can
	/// lead to relay shutdown when chain is upgraded, even though we have explicitly
	/// said that we don't want to shutdown.
	#[allow(dead_code)]
	pub fn can_start_version_guard(&self) -> bool {
		self.runtime_version.version_mode != RuntimeVersionType::Auto
	}

	/// Convert connection params into Substrate client.
	#[allow(dead_code)]
	pub async fn to_client<Chain: CliChain>(
		&self,
	) -> anyhow::Result<relay_substrate_client::Client<Chain>> {
		let chain_runtime_version =
			self.runtime_version.into_runtime_version(Some(Chain::RUNTIME_VERSION))?;
		Ok(relay_substrate_client::Client::new(relay_substrate_client::ConnectionParams {
			host: self.host.clone(),
			port: self.port,
			secure: self.secure,
			chain_runtime_version,
		})
		.await)
	}

	/// Return selected `chain_spec` version.
	///
	/// This function only connects to the node if version mode is set to `Auto`.
	#[allow(dead_code)]
	pub async fn selected_chain_spec_version<Chain: CliChain>(&self) -> anyhow::Result<u32> {
		let chain_runtime_version =
			self.runtime_version.into_runtime_version(Some(Chain::RUNTIME_VERSION))?;
		Ok(match chain_runtime_version {
			ChainRuntimeVersion::Auto =>
				self.to_client::<Chain>().await?.simple_runtime_version().await?.0,
			ChainRuntimeVersion::Custom(spec_version, _) => spec_version,
		})
	}
}

/// Create chain-specific set of runtime version parameters.
#[macro_export]
macro_rules! declare_chain_connection_params_cli_schema {
	($chain:ident, $chain_prefix:ident) => {
		paste::item! {
			#[doc = $chain " connection params."]
			#[derive(StructOpt, Debug, PartialEq, Eq, Clone)]
			pub struct [<$chain ConnectionParams>] {
				#[doc = "Connect to " $chain " node at given host."]
				#[structopt(long, default_value = "127.0.0.1")]
				pub [<$chain_prefix _host>]: String,
				#[doc = "Connect to " $chain " node websocket server at given port."]
				#[structopt(long, default_value = "9944")]
				pub [<$chain_prefix _port>]: u16,
				#[doc = "Use secure websocket connection."]
				#[structopt(long)]
				pub [<$chain_prefix _secure>]: bool,
				#[doc = "Custom runtime version"]
				#[structopt(flatten)]
				pub [<$chain_prefix _runtime_version>]: [<$chain RuntimeVersionParams>],
			}

			impl From<[<$chain ConnectionParams>]> for ConnectionParams {
				fn from(item: [<$chain ConnectionParams>]) -> ConnectionParams {
					ConnectionParams {
						host: item.[<$chain_prefix _host>],
						port: item.[<$chain_prefix _port>],
						secure: item.[<$chain_prefix _secure>],
						runtime_version: item.[<$chain_prefix _runtime_version>].into(),
					}
				}
			}
		}
	};
}

/// Helper trait to override transaction parameters differently.
pub trait TransactionParamsProvider {
	/// Returns `true` if transaction parameters are defined by this provider.
	fn is_defined(&self) -> bool;
	/// Returns transaction parameters.
	fn transaction_params<Chain: CliChain>(
		&self,
	) -> anyhow::Result<TransactionParams<Chain::KeyPair>>;

	/// Returns transaction parameters, defined by `self` provider or, if they're not defined,
	/// defined by `other` provider.
	fn transaction_params_or<Chain: CliChain, T: TransactionParamsProvider>(
		&self,
		other: &T,
	) -> anyhow::Result<TransactionParams<Chain::KeyPair>> {
		if self.is_defined() {
			self.transaction_params::<Chain>()
		} else {
			other.transaction_params::<Chain>()
		}
	}
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SigningParams {
	pub prefix: &'static str,

	pub signer: Option<String>,
	pub signer_password: Option<String>,

	pub signer_file: Option<std::path::PathBuf>,
	pub signer_password_file: Option<std::path::PathBuf>,

	pub transactions_mortality: Option<u32>,
}

impl SigningParams {
	/// Return transactions mortality.
	pub fn transactions_mortality(&self) -> anyhow::Result<Option<u32>> {
		self.transactions_mortality
			.map(|transactions_mortality| {
				if !(4..=65536).contains(&transactions_mortality) ||
					!transactions_mortality.is_power_of_two()
				{
					Err(anyhow::format_err!(
						"Transactions mortality {} is not a power of two in a [4; 65536] range",
						transactions_mortality,
					))
				} else {
					Ok(transactions_mortality)
				}
			})
			.transpose()
	}

	/// Parse signing params into chain-specific KeyPair.
	#[allow(dead_code)]
	pub fn to_keypair<Chain: CliChain>(&self) -> anyhow::Result<Chain::KeyPair> {
		let suri = match (self.signer.as_ref(), self.signer_file.as_ref()) {
			(Some(suri), _) => suri.to_owned(),
			(None, Some(suri_file)) => std::fs::read_to_string(suri_file).map_err(|err| {
				anyhow::format_err!("Failed to read SURI from file {:?}: {}", suri_file, err,)
			})?,
			(None, None) =>
				return Err(anyhow::format_err!(
					"One of options must be specified: '{0}_signer' or '{0}_signer_file'",
					self.prefix
				)),
		};

		let suri_password =
			match (self.signer_password.as_ref(), self.signer_password_file.as_ref()) {
				(Some(suri_password), _) => Some(suri_password.to_owned()),
				(None, Some(suri_password_file)) =>
					std::fs::read_to_string(suri_password_file).map(Some).map_err(|err| {
						anyhow::format_err!(
							"Failed to read SURI password from file {:?}: {}",
							suri_password_file,
							err,
						)
					})?,
				_ => None,
			};

		Chain::KeyPair::from_string(&suri, suri_password.as_deref())
			.map_err(|e| anyhow::format_err!("{:?}", e))
	}
}

impl TransactionParamsProvider for SigningParams {
	fn is_defined(&self) -> bool {
		self.signer.is_some() || self.signer_file.is_some()
	}

	fn transaction_params<Chain: CliChain>(
		&self,
	) -> anyhow::Result<TransactionParams<Chain::KeyPair>> {
		Ok(TransactionParams {
			mortality: self.transactions_mortality()?,
			signer: self.to_keypair::<Chain>()?,
		})
	}
}

/// Create chain-specific set of signing parameters.
#[macro_export]
macro_rules! declare_chain_signing_params_cli_schema {
	($chain:ident, $chain_prefix:ident) => {
		paste::item! {
			#[doc = $chain " signing params."]
			#[derive(StructOpt, Debug, PartialEq, Eq, Clone)]
			pub struct [<$chain SigningParams>] {
				#[doc = "The SURI of secret key to use when transactions are submitted to the " $chain " node."]
				#[structopt(long)]
				pub [<$chain_prefix _signer>]: Option<String>,
				#[doc = "The password for the SURI of secret key to use when transactions are submitted to the " $chain " node."]
				#[structopt(long)]
				pub [<$chain_prefix _signer_password>]: Option<String>,

				#[doc = "Path to the file, that contains SURI of secret key to use when transactions are submitted to the " $chain " node. Can be overridden with " $chain_prefix "_signer option."]
				#[structopt(long)]
				pub [<$chain_prefix _signer_file>]: Option<std::path::PathBuf>,
				#[doc = "Path to the file, that password for the SURI of secret key to use when transactions are submitted to the " $chain " node. Can be overridden with " $chain_prefix "_signer_password option."]
				#[structopt(long)]
				pub [<$chain_prefix _signer_password_file>]: Option<std::path::PathBuf>,

				#[doc = "Transactions mortality period, in blocks. MUST be a power of two in [4; 65536] range. MAY NOT be larger than `BlockHashCount` parameter of the chain system module."]
				#[structopt(long)]
				pub [<$chain_prefix _transactions_mortality>]: Option<u32>,
			}

			impl From<[<$chain SigningParams>]> for SigningParams {
				fn from(item: [<$chain SigningParams>]) -> SigningParams {
					SigningParams {
						prefix: stringify!([<$chain_prefix>]),

						signer: item.[<$chain_prefix _signer>],
						signer_password: item.[<$chain_prefix _signer_password>],
						signer_file: item.[<$chain_prefix _signer_file>],
						signer_password_file: item.[<$chain_prefix _signer_password_file>],
						transactions_mortality: item.[<$chain_prefix _transactions_mortality>],
					}
				}
			}
		}
	};
}

#[derive(Debug, PartialEq, Eq)]
pub struct MessagesPalletOwnerSigningParams {
	pub messages_pallet_owner: Option<String>,
	pub messages_pallet_owner_password: Option<String>,
}

impl MessagesPalletOwnerSigningParams {
	/// Parse signing params into chain-specific KeyPair.
	#[allow(dead_code)]
	pub fn to_keypair<Chain: CliChain>(&self) -> anyhow::Result<Option<Chain::KeyPair>> {
		let messages_pallet_owner = match self.messages_pallet_owner {
			Some(ref messages_pallet_owner) => messages_pallet_owner,
			None => return Ok(None),
		};
		Chain::KeyPair::from_string(
			messages_pallet_owner,
			self.messages_pallet_owner_password.as_deref(),
		)
		.map_err(|e| anyhow::format_err!("{:?}", e))
		.map(Some)
	}
}

/// Create chain-specific set of messages pallet owner signing parameters.
#[macro_export]
macro_rules! declare_chain_messages_pallet_owner_signing_params_cli_schema {
	($chain:ident, $chain_prefix:ident) => {
		paste::item! {
			#[doc = "Parameters required to sign transaction on behalf of owner of the messages pallet at " $chain "."]
			#[derive(StructOpt, Debug, PartialEq, Eq)]
			pub struct [<$chain MessagesPalletOwnerSigningParams>] {
				#[doc = "The SURI of secret key to use when transactions are submitted to the " $chain " node."]
				#[structopt(long)]
				pub [<$chain_prefix _messages_pallet_owner>]: Option<String>,
				#[doc = "The password for the SURI of secret key to use when transactions are submitted to the " $chain " node."]
				#[structopt(long)]
				pub [<$chain_prefix _messages_pallet_owner_password>]: Option<String>,
			}

			impl From<[<$chain MessagesPalletOwnerSigningParams>]> for MessagesPalletOwnerSigningParams {
				fn from(item: [<$chain MessagesPalletOwnerSigningParams>]) -> MessagesPalletOwnerSigningParams {
					MessagesPalletOwnerSigningParams {
						messages_pallet_owner: item.[<$chain_prefix _messages_pallet_owner>],
						messages_pallet_owner_password: item.[<$chain_prefix _messages_pallet_owner_password>],
					}
				}
			}
		}
	};
}

/// Create chain-specific set of configuration objects: connection parameters,
/// signing parameters and bridge initialization parameters.
#[macro_export]
macro_rules! declare_chain_cli_schema {
	($chain:ident, $chain_prefix:ident) => {
		$crate::declare_chain_runtime_version_params_cli_schema!($chain, $chain_prefix);
		$crate::declare_chain_connection_params_cli_schema!($chain, $chain_prefix);
		$crate::declare_chain_signing_params_cli_schema!($chain, $chain_prefix);
		$crate::declare_chain_messages_pallet_owner_signing_params_cli_schema!(
			$chain,
			$chain_prefix
		);
	};
}

declare_chain_cli_schema!(Source, source);
declare_chain_cli_schema!(Target, target);
declare_chain_cli_schema!(Relaychain, relaychain);
declare_chain_cli_schema!(Parachain, parachain);
