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

use crate::cli::{
	bridge::{
		CliBridgeBase, KusamaToPolkadotCliBridge, MillauToRialtoCliBridge,
		MillauToRialtoParachainCliBridge, PolkadotToKusamaCliBridge, RialtoToMillauCliBridge,
		RococoToWococoCliBridge, WestendToMillauCliBridge, WococoToRococoCliBridge,
	},
	SourceConnectionParams, TargetConnectionParams, TargetSigningParams,
};
use bp_runtime::Chain as ChainBase;
use codec::Encode;
use relay_substrate_client::{
	AccountKeyPairOf, Chain, SignParam, TransactionSignScheme, UnsignedTransaction,
};
use sp_core::{Bytes, Pair};
use std::{future::Future, pin::Pin};
use structopt::StructOpt;
use strum::{EnumString, EnumVariantNames, VariantNames};
use substrate_relay_helper::finality::engine::{Engine, Grandpa as GrandpaFinalityEngine};

/// Initialize bridge pallet.
#[derive(StructOpt)]
pub struct InitBridge {
	/// A bridge instance to initialize.
	#[structopt(possible_values = InitBridgeName::VARIANTS, case_insensitive = true)]
	bridge: InitBridgeName,
	#[structopt(flatten)]
	source: SourceConnectionParams,
	#[structopt(flatten)]
	target: TargetConnectionParams,
	#[structopt(flatten)]
	target_sign: TargetSigningParams,
}

#[derive(Debug, EnumString, EnumVariantNames)]
#[strum(serialize_all = "kebab_case")]
/// Bridge to initialize.
pub enum InitBridgeName {
	MillauToRialto,
	RialtoToMillau,
	WestendToMillau,
	RococoToWococo,
	WococoToRococo,
	KusamaToPolkadot,
	PolkadotToKusama,
	MillauToRialtoParachain,
}

trait BridgeInitializer: CliBridgeBase
where
	<Self::Target as ChainBase>::AccountId: From<<AccountKeyPairOf<Self::Target> as Pair>::Public>,
{
	type Engine: Engine<Self::Source>;

	/// Get the encoded call to init the bridge.
	fn encode_init_bridge(
		init_data: <Self::Engine as Engine<Self::Source>>::InitializationData,
	) -> <Self::Target as Chain>::Call;

	/// Initialize the bridge.
	fn init_bridge(
		data: InitBridge,
	) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'static>> {
		Box::pin(async move {
			let source_client = data.source.to_client::<Self::Source>().await?;
			let target_client = data.target.to_client::<Self::Target>().await?;
			let target_sign = data.target_sign.to_keypair::<Self::Target>()?;

			let (spec_version, transaction_version) =
				target_client.simple_runtime_version().await?;
			substrate_relay_helper::finality::initialize::initialize::<Self::Engine, _, _, _>(
				source_client,
				target_client.clone(),
				target_sign.public().into(),
				move |transaction_nonce, initialization_data| {
					Ok(Bytes(
						Self::Target::sign_transaction(SignParam {
							spec_version,
							transaction_version,
							genesis_hash: *target_client.genesis_hash(),
							signer: target_sign,
							era: relay_substrate_client::TransactionEra::immortal(),
							unsigned: UnsignedTransaction::new(
								Self::encode_init_bridge(initialization_data).into(),
								transaction_nonce,
							),
						})?
						.encode(),
					))
				},
			)
			.await;

			Ok(())
		})
	}
}

impl BridgeInitializer for MillauToRialtoCliBridge {
	type Engine = GrandpaFinalityEngine<Self::Source>;

	fn encode_init_bridge(
		init_data: <Self::Engine as Engine<Self::Source>>::InitializationData,
	) -> <Self::Target as Chain>::Call {
		rialto_runtime::SudoCall::sudo {
			call: Box::new(
				rialto_runtime::BridgeGrandpaMillauCall::initialize { init_data }.into(),
			),
		}
		.into()
	}
}

impl BridgeInitializer for MillauToRialtoParachainCliBridge {
	type Engine = GrandpaFinalityEngine<Self::Source>;

	fn encode_init_bridge(
		init_data: <Self::Engine as Engine<Self::Source>>::InitializationData,
	) -> <Self::Target as Chain>::Call {
		let initialize_call = rialto_parachain_runtime::BridgeGrandpaCall::<
			rialto_parachain_runtime::Runtime,
			rialto_parachain_runtime::MillauGrandpaInstance,
		>::initialize {
			init_data,
		};
		rialto_parachain_runtime::SudoCall::sudo { call: Box::new(initialize_call.into()) }.into()
	}
}

impl BridgeInitializer for RialtoToMillauCliBridge {
	type Engine = GrandpaFinalityEngine<Self::Source>;

	fn encode_init_bridge(
		init_data: <Self::Engine as Engine<Self::Source>>::InitializationData,
	) -> <Self::Target as Chain>::Call {
		let initialize_call = millau_runtime::BridgeGrandpaCall::<
			millau_runtime::Runtime,
			millau_runtime::RialtoGrandpaInstance,
		>::initialize {
			init_data,
		};
		millau_runtime::SudoCall::sudo { call: Box::new(initialize_call.into()) }.into()
	}
}

impl BridgeInitializer for WestendToMillauCliBridge {
	type Engine = GrandpaFinalityEngine<Self::Source>;

	fn encode_init_bridge(
		init_data: <Self::Engine as Engine<Self::Source>>::InitializationData,
	) -> <Self::Target as Chain>::Call {
		// at Westend -> Millau initialization we're not using sudo, because otherwise
		// our deployments may fail, because we need to initialize both Rialto -> Millau
		// and Westend -> Millau bridge. => since there's single possible sudo account,
		// one of transaction may fail with duplicate nonce error
		millau_runtime::BridgeGrandpaCall::<
			millau_runtime::Runtime,
			millau_runtime::WestendGrandpaInstance,
		>::initialize {
			init_data,
		}
		.into()
	}
}

impl BridgeInitializer for RococoToWococoCliBridge {
	type Engine = GrandpaFinalityEngine<Self::Source>;

	fn encode_init_bridge(
		init_data: <Self::Engine as Engine<Self::Source>>::InitializationData,
	) -> <Self::Target as Chain>::Call {
		relay_wococo_client::runtime::Call::BridgeGrandpaRococo(
			relay_wococo_client::runtime::BridgeGrandpaRococoCall::initialize(init_data),
		)
	}
}

impl BridgeInitializer for WococoToRococoCliBridge {
	type Engine = GrandpaFinalityEngine<Self::Source>;

	fn encode_init_bridge(
		init_data: <Self::Engine as Engine<Self::Source>>::InitializationData,
	) -> <Self::Target as Chain>::Call {
		relay_rococo_client::runtime::Call::BridgeGrandpaWococo(
			relay_rococo_client::runtime::BridgeGrandpaWococoCall::initialize(init_data),
		)
	}
}

impl BridgeInitializer for KusamaToPolkadotCliBridge {
	type Engine = GrandpaFinalityEngine<Self::Source>;

	fn encode_init_bridge(
		init_data: <Self::Engine as Engine<Self::Source>>::InitializationData,
	) -> <Self::Target as Chain>::Call {
		relay_polkadot_client::runtime::Call::BridgeKusamaGrandpa(
			relay_polkadot_client::runtime::BridgeKusamaGrandpaCall::initialize(init_data),
		)
	}
}

impl BridgeInitializer for PolkadotToKusamaCliBridge {
	type Engine = GrandpaFinalityEngine<Self::Source>;

	fn encode_init_bridge(
		init_data: <Self::Engine as Engine<Self::Source>>::InitializationData,
	) -> <Self::Target as Chain>::Call {
		relay_kusama_client::runtime::Call::BridgePolkadotGrandpa(
			relay_kusama_client::runtime::BridgePolkadotGrandpaCall::initialize(init_data),
		)
	}
}

impl InitBridge {
	/// Run the command.
	pub async fn run(self) -> anyhow::Result<()> {
		match self.bridge {
			InitBridgeName::MillauToRialto => MillauToRialtoCliBridge::init_bridge(self),
			InitBridgeName::RialtoToMillau => RialtoToMillauCliBridge::init_bridge(self),
			InitBridgeName::WestendToMillau => WestendToMillauCliBridge::init_bridge(self),
			InitBridgeName::RococoToWococo => RococoToWococoCliBridge::init_bridge(self),
			InitBridgeName::WococoToRococo => WococoToRococoCliBridge::init_bridge(self),
			InitBridgeName::KusamaToPolkadot => KusamaToPolkadotCliBridge::init_bridge(self),
			InitBridgeName::PolkadotToKusama => PolkadotToKusamaCliBridge::init_bridge(self),
			InitBridgeName::MillauToRialtoParachain =>
				MillauToRialtoParachainCliBridge::init_bridge(self),
		}
		.await
	}
}
