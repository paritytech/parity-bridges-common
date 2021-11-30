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

//! Kusama-to-Polkadot messages sync entrypoint.

use codec::Encode;
use frame_support::weights::Weight;
use sp_core::{Bytes, Pair};

use messages_relay::relay_strategy::MixStrategy;
use relay_kusama_client::Kusama;
use relay_polkadot_client::Polkadot;
use relay_substrate_client::{Client, TransactionSignScheme, UnsignedTransaction};
use substrate_relay_helper::{
	messages_lane::SubstrateMessageLane,
	messages_metrics::StandaloneMessagesMetrics,
};

/// Description of Kusama -> Polkadot messages bridge.
#[derive(Clone, Debug)]
pub struct KusamaMessagesToPolkadot;
substrate_relay_helper::generate_mocked_receive_message_proof_call_builder!(
	KusamaMessagesToPolkadot,
	KusamaMessagesToPolkadotReceiveMessagesProofCallBuilder,
	relay_polkadot_client::runtime::Call::BridgeKusamaMessages,
	relay_polkadot_client::runtime::BridgeKusamaMessagesCall::receive_messages_proof
);
substrate_relay_helper::generate_mocked_receive_message_delivery_proof_call_builder!(
	KusamaMessagesToPolkadot,
	KusamaMessagesToPolkadotReceiveMessagesDeliveryProofCallBuilder,
	relay_kusama_client::runtime::Call::BridgePolkadotMessages,
	relay_kusama_client::runtime::BridgePolkadotMessagesCall::receive_messages_delivery_proof
);

impl SubstrateMessageLane for KusamaMessagesToPolkadot {
	type SourceChain = Kusama;
	type TargetChain = Polkadot;

	type SourceTransactionSignScheme = Kusama;
	type TargetTransactionSignScheme = Polkadot;

	type ReceiveMessagesProofCallBuilder = KusamaMessagesToPolkadotReceiveMessagesProofCallBuilder;
	type ReceiveMessagesDeliveryProofCallBuilder = KusamaMessagesToPolkadotReceiveMessagesDeliveryProofCallBuilder;

	type RelayStrategy = MixStrategy;
}

/// Create standalone metrics for the Kusama -> Polkadot messages loop.
pub(crate) fn standalone_metrics(
	source_client: Client<Kusama>,
	target_client: Client<Polkadot>,
) -> anyhow::Result<StandaloneMessagesMetrics<Kusama, Polkadot>> {
	substrate_relay_helper::messages_metrics::standalone_metrics(
		source_client,
		target_client,
		Some(crate::chains::kusama::TOKEN_ID),
		Some(crate::chains::polkadot::TOKEN_ID),
		Some(crate::chains::polkadot::kusama_to_polkadot_conversion_rate_params()),
		Some(crate::chains::kusama::polkadot_to_kusama_conversion_rate_params()),
	)
}

/// Update Polkadot -> Kusama conversion rate, stored in Kusama runtime storage.
pub(crate) async fn update_polkadot_to_kusama_conversion_rate(
	client: Client<Kusama>,
	signer: <Kusama as TransactionSignScheme>::AccountKeyPair,
	updated_rate: f64,
) -> anyhow::Result<()> {
	let genesis_hash = *client.genesis_hash();
	let signer_id = (*signer.public().as_array_ref()).into();
	client
		.submit_signed_extrinsic(signer_id, move |_, transaction_nonce| {
			Bytes(
				Kusama::sign_transaction(
					genesis_hash,
					&signer,
					relay_substrate_client::TransactionEra::immortal(),
					UnsignedTransaction::new(
						relay_kusama_client::runtime::Call::BridgePolkadotMessages(
							relay_kusama_client::runtime::BridgePolkadotMessagesCall::update_pallet_parameter(
								relay_kusama_client::runtime::BridgePolkadotMessagesParameter::PolkadotToKusamaConversionRate(
									sp_runtime::FixedU128::from_float(updated_rate),
								)
							)
						),
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
