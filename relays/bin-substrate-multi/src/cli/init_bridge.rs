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

use crate::cli::{GatewayIdParams, SourceConnectionParams, TargetConnectionParams, TargetSigningParams};
use bp_runtime::Chain as ChainBase;
use codec::Encode;
use pallet_multi_finality_verifier::InitializationData;
use relay_substrate_client::{Chain, TransactionSignScheme};
use sp_core::{Bytes, Pair};
use structopt::{clap::arg_enum, StructOpt};

/// Initialize bridge pallet.
#[derive(StructOpt)]
pub struct InitBridge {
	/// A bridge instance to initalize.
	#[structopt(possible_values = &InitBridgeName::variants(), case_insensitive = true)]
	bridge: InitBridgeName,
	#[structopt(long, default_value = "gate")]
	gateway_id: GatewayIdParams,
	#[structopt(flatten)]
	source: SourceConnectionParams,
	#[structopt(flatten)]
	target: TargetConnectionParams,
	#[structopt(flatten)]
	target_sign: TargetSigningParams,
}

// TODO [#851] Use kebab-case.
arg_enum! {
	#[derive(Debug)]
	/// Bridge to initialize.
	pub enum InitBridgeName {
		GatewayToCircuit,
	}
}

macro_rules! select_bridge {
	($bridge: expr, $generic: tt) => {
		match $bridge {
			InitBridgeName::GatewayToCircuit => {
				type Source = relay_gateway_client::Gateway;
				type Target = relay_circuit_client::Circuit;

				fn encode_init_bridge(
					init_data: InitializationData<<Source as ChainBase>::Header>,
					gateway_id: bp_runtime::InstanceId,
				) -> <Target as Chain>::Call {
					let initialize_call = circuit_runtime::BridgePolkadotLikeMultiFinalityVerifierCall::<
						circuit_runtime::Runtime,
						circuit_runtime::PolkadotLikeGrandpaInstance,
					>::initialize_single(init_data, gateway_id);
					circuit_runtime::SudoCall::sudo(Box::new(initialize_call.into())).into()
				}

				$generic
			}
		}
	};
}

impl InitBridge {
	/// Run the command.
	pub async fn run(self) -> anyhow::Result<()> {
		select_bridge!(self.bridge, {
			let source_client = self.source.to_client::<Source>().await?;
			let target_client = self.target.to_client::<Target>().await?;
			let target_sign = self.target_sign.to_keypair::<Target>()?;

			crate::headers_multi_initialize::initialize(
				source_client,
				target_client.clone(),
				target_sign.public().into(),
				move |transaction_nonce, initialization_data| {
					Bytes(
						Target::sign_transaction(
							*target_client.genesis_hash(),
							&target_sign,
							transaction_nonce,
							encode_init_bridge(initialization_data, self.gateway_id.0),
						)
						.encode(),
					)
				},
			)
			.await;

			Ok(())
		})
	}
}
