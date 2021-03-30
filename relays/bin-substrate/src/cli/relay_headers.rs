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

use crate::cli::{
	SourceConnectionParams, TargetConnectionParams, TargetSigningParams, PrometheusParams,
};
use structopt::{StructOpt, clap::arg_enum};
use crate::rialto_millau::CliChain;

arg_enum! {
	#[derive(Debug)]
	/// Headers relay bridge.
	pub enum RelayHeadersBridge {
		MillauToRialto,
		RialtoToMillau,
		WestendToMillau,
	}
}

macro_rules! select_bridge {
    ($bridge: expr, $generic: tt) => {
        match $bridge {
            RelayHeadersBridge::MillauToRialto => {
                type Source = relay_millau_client::Millau;
                type Target = relay_rialto_client::Rialto;
				type Finality =
					crate::rialto_millau::millau_headers_to_rialto::MillauFinalityToRialto;
                $generic
            },
            RelayHeadersBridge::RialtoToMillau => {
                type Source = relay_rialto_client::Rialto;
                type Target = relay_millau_client::Millau;
				type Finality =
					crate::rialto_millau::rialto_headers_to_millau::RialtoFinalityToMillau;
                $generic
            },
            RelayHeadersBridge::WestendToMillau => {
                type Source = relay_westend_client::Westend;
                type Target = relay_millau_client::Millau;
				type Finality =
					crate::rialto_millau::westend_headers_to_millau::WestendFinalityToMillau;
                $generic
			},
        }
    }
}


/// Start headers relayer process.
#[derive(StructOpt)]
pub struct RelayHeaders {
	bridge: RelayHeadersBridge,
	#[structopt(flatten)]
	source: SourceConnectionParams,
	#[structopt(flatten)]
	target: TargetConnectionParams,
	#[structopt(flatten)]
	target_sign: TargetSigningParams,
	#[structopt(flatten)]
	prometheus_params: PrometheusParams,
}

impl RelayHeaders {
	/// Run the command.
	pub async fn run(self) -> anyhow::Result<()> {
		select_bridge!(self.bridge, {
			let source_client = crate::rialto_millau::source_chain_client::<Source>(self.source).await?;
			let target_client = crate::rialto_millau::target_chain_client::<Target>(self.target).await?;
			let target_sign = Target::target_signing_params(self.target_sign)
				.map_err(|e| anyhow::format_err!("{}", e))?;

			crate::finality_pipeline::run(
				Finality::new(target_client.clone(), target_sign),
				source_client,
				target_client,
				self.prometheus_params.into(),
			)
			.await
		})
	}
}
