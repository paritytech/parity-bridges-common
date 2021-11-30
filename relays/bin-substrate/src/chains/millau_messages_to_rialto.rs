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

//! Millau-to-Rialto messages sync entrypoint.

use codec::Encode;
use sp_core::{Bytes, Pair};

use messages_relay::relay_strategy::MixStrategy;
use relay_millau_client::Millau;
use relay_rialto_client::Rialto;
use relay_substrate_client::{Client, TransactionSignScheme, UnsignedTransaction};
use substrate_relay_helper::{
	messages_lane::{
		SubstrateMessageLane, DirectReceiveMessagesProofCallBuilder, DirectReceiveMessagesDeliveryProofCallBuilder,
	},
	messages_metrics::StandaloneMessagesMetrics,
};

/// Description of Millau -> Rialto messages bridge.
#[derive(Clone, Debug)]
pub struct MillauMessagesToRialto;

impl SubstrateMessageLane for MillauMessagesToRialto {
	type SourceChain = Millau;
	type TargetChain = Rialto;

	type SourceTransactionSignScheme = Millau;
	type TargetTransactionSignScheme = Rialto;

	type ReceiveMessagesProofCallBuilder = DirectReceiveMessagesProofCallBuilder<
		Self,
		rialto_runtime::Runtime,
		rialto_runtime::WithMillauMessagesInstance,
	>;
	type ReceiveMessagesDeliveryProofCallBuilder = DirectReceiveMessagesDeliveryProofCallBuilder<
		Self,
		millau_runtime::Runtime,
		millau_runtime::WithRialtoMessagesInstance,
	>;

	type RelayStrategy = MixStrategy;
}

/// Create standalone metrics for the Millau -> Rialto messages loop.
pub(crate) fn standalone_metrics(
	source_client: Client<Millau>,
	target_client: Client<Rialto>,
) -> anyhow::Result<StandaloneMessagesMetrics<Millau, Rialto>> {
	substrate_relay_helper::messages_metrics::standalone_metrics(
		source_client,
		target_client,
		Some(crate::chains::millau::ASSOCIATED_TOKEN_ID),
		Some(crate::chains::rialto::ASSOCIATED_TOKEN_ID),
		Some(crate::chains::rialto::millau_to_rialto_conversion_rate_params()),
		Some(crate::chains::millau::rialto_to_millau_conversion_rate_params()),
	)
}

/// Update Rialto -> Millau conversion rate, stored in Millau runtime storage.
pub(crate) async fn update_rialto_to_millau_conversion_rate(
	client: Client<Millau>,
	signer: <Millau as TransactionSignScheme>::AccountKeyPair,
	updated_rate: f64,
) -> anyhow::Result<()> {
	let genesis_hash = *client.genesis_hash();
	let signer_id = (*signer.public().as_array_ref()).into();
	client
		.submit_signed_extrinsic(signer_id, move |_, transaction_nonce| {
			Bytes(
				Millau::sign_transaction(
					genesis_hash,
					&signer,
					relay_substrate_client::TransactionEra::immortal(),
					UnsignedTransaction::new(
						millau_runtime::MessagesCall::update_pallet_parameter {
							parameter: millau_runtime::rialto_messages::MillauToRialtoMessagesParameter::RialtoToMillauConversionRate(
								sp_runtime::FixedU128::from_float(updated_rate),
							),
						}
						.into(),
						transaction_nonce,
					),
				)
				.encode(),
			)
		})
		.await
		.map(drop)
		.map_err(|err| anyhow::format_err!("{:?}", err))
}
