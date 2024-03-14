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

use async_trait::async_trait;
use codec::Encode;

use crate::{bridge::CliBridgeBase, chain_schema::*};
use bp_runtime::Chain as ChainBase;
use relay_substrate_client::{AccountKeyPairOf, Chain, UnsignedTransaction};
use sp_core::Pair;
use structopt::StructOpt;
use substrate_relay_helper::finality_base::engine::Engine;

/// Bridge initialization params.
#[derive(StructOpt)]
pub struct InitBridgeParams {
	#[structopt(flatten)]
	source: SourceConnectionParams,
	#[structopt(flatten)]
	target: TargetConnectionParams,
	#[structopt(flatten)]
	target_sign: TargetSigningParams,
	/// Generates all required data, but does not submit extrinsic
	#[structopt(long)]
	dry_run: bool,
}

#[async_trait]
pub trait BridgeInitializer: CliBridgeBase
where
	<Self::Target as ChainBase>::AccountId: From<<AccountKeyPairOf<Self::Target> as Pair>::Public>,
{
	type Engine: Engine<Self::Source>;

	/// Get the encoded call to init the bridge.
	fn encode_init_bridge(
		init_data: <Self::Engine as Engine<Self::Source>>::InitializationData,
	) -> <Self::Target as Chain>::Call;

	/// Initialize the bridge.
	async fn init_bridge(data: InitBridgeParams) -> anyhow::Result<()> {
		let source_client = data.source.into_client::<Self::Source>().await?;
		let target_client = data.target.into_client::<Self::Target>().await?;
		let target_sign = data.target_sign.to_keypair::<Self::Target>()?;
		let dry_run = data.dry_run;

		substrate_relay_helper::finality::initialize::initialize::<Self::Engine, _, _, _>(
			source_client,
			target_client.clone(),
			target_sign,
			move |transaction_nonce, initialization_data| {
				let call = Self::encode_init_bridge(initialization_data);
				log::info!(
					target: "bridge",
					"Initialize bridge call encoded as hex string: {:?}",
					format!("0x{}", hex::encode(call.encode()))
				);
				Ok(UnsignedTransaction::new(call.into(), transaction_nonce))
			},
			dry_run,
		)
		.await;

		Ok(())
	}
}
