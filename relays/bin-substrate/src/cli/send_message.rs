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

use crate::cli::encode_call::{CliEncodeCall, EncodeCallBridge};
use crate::cli::{
	source_chain_client, target_chain_client, AccountId, Balance, CliChain, ExplicitOrMaximal, HexBytes, HexLaneId,
	Origins, SourceConnectionParams, SourceSigningParams, TargetSigningParams,
};
use crate::select_bridge;
use codec::{Decode, Encode};
use frame_support::{dispatch::GetDispatchInfo, weights::Weight};
use relay_substrate_client::{Chain, TransactionSignScheme};
use sp_core::Pair;
use structopt::{clap::arg_enum, StructOpt};

/// Send bridge message.
#[derive(StructOpt)]
pub struct SendMessage {
	/// A bridge instance to encode call for.
	#[structopt(possible_values = &EncodeCallBridge::variants(), case_insensitive = true)]
	bridge: EncodeCallBridge,
	#[structopt(flatten)]
	source: SourceConnectionParams,
	#[structopt(flatten)]
	source_sign: SourceSigningParams,
	// TODO [ToDr] Move TargetSign to origins
	#[structopt(flatten)]
	target_sign: TargetSigningParams,
	/// Hex-encoded lane id. Defaults to `00000000`.
	#[structopt(long, default_value = "00000000")]
	lane: HexLaneId,
	/// Dispatch weight of the message. If not passed, determined automatically.
	#[structopt(long)]
	dispatch_weight: Option<ExplicitOrMaximal<Weight>>,
	/// Delivery and dispatch fee in source chain base currency units. If not passed, determined automatically.
	#[structopt(long)]
	fee: Option<Balance>,
	/// Message type.
	#[structopt(subcommand)]
	message: crate::cli::encode_call::Call,
	/// The origin to use when dispatching the message on the target chain. Defaults to
	/// `SourceAccount`.
	#[structopt(long, possible_values = &Origins::variants(), default_value = "Source")]
	origin: Origins,
}

impl SendMessage {
	/// Run the command.
	pub async fn run(mut self) -> anyhow::Result<()> {
		let Self {
			source,
			source_sign,
			target_sign,
			lane,
			mut message,
			dispatch_weight,
			fee,
			origin,
			bridge,
		} = self;

		select_bridge!(bridge, {
			// Bridge-specific
			// let account_ownership_digest = |target_call, source_account_id| {
			// 	millau_runtime::rialto_account_ownership_digest(
			// 		&target_call,
			// 		source_account_id,
			// 		Target::RUNTIME_VERSION.spec_version,
			// 	)
			// };
			// let estimate_message_fee_method = bp_rialto::TO_RIALTO_ESTIMATE_MESSAGE_FEE_METHOD;
			// let fee = fee.map(|x| x.cast());
			// let send_message_call = |lane, payload, fee| {
			// 	millau_runtime::Call::BridgeRialtoMessages(millau_runtime::MessagesCall::send_message(
			// 		lane, payload, fee,
			// 	))
			// };

			let source_client = self.source.into_client::<Source>().await?;
			let target_client = self.target.into_client::<Target>().await?;
			let target_sign = self.target_sign.into_keypair::<Target>()?;

			encode_call::preprocess_call::<Source, Target>(&mut message, bridge.pallet_index());
			let target_call = Target::encode_call(&message)?;

			let payload = {
				let target_call_weight = prepare_call_dispatch_weight(
					dispatch_weight,
					ExplicitOrMaximal::Explicit(target_call.get_dispatch_info().weight),
					compute_maximal_message_dispatch_weight(Target::max_extrinsic_weight()),
				);
				let source_sender_public: MultiSigner = source_sign.public().into();
				let source_account_id = source_sender_public.into_account();

				message_payload(
					Target::RUNTIME_VERSION.spec_version,
					target_call_weight,
					match origin {
						Origins::Source => CallOrigin::SourceAccount(source_account_id),
						Origins::Target => {
							let digest = account_ownership_digest(&target_call, source_account_id.clone());
							let target_origin_public = target_sign.public();
							let digest_signature = target_sign.sign(&digest);
							CallOrigin::TargetAccount(
								source_account_id,
								target_origin_public.into(),
								digest_signature.into(),
							)
						}
					},
					&target_call,
				)
			};
			let dispatch_weight = payload.weight;

			let lane = lane.into();
			let fee = get_fee(fee, || {
				estimate_message_delivery_and_dispatch_fee(
					&source_client,
					estimate_message_fee_method,
					lane,
					payload.clone(),
				)
			})
			.await?;

			source_client
				.submit_signed_extrinsic(source_sign.public().into(), |transaction_nonce| {
					let send_message_call = send_message_call(lane, payload, fee);

					let signed_source_call = Source::sign_transaction(
						*source_client.genesis_hash(),
						&source_sign,
						transaction_nonce,
						send_message_call,
					)
					.encode();

					log::info!(
						target: "bridge",
						"Sending message to {}. Size: {}. Dispatch weight: {}. Fee: {}",
						Target::NAME,
						signed_source_call.len(),
						dispatch_weight,
						fee,
					);
					log::info!(
						target: "bridge",
						"Signed {} Call: {:?}",
						Source::NAME,
						HexBytes::encode(&signed_source_call)
					);

					Bytes(signed_source_call)
				})
				.await?;
		});

		Ok(())
	}
}

fn prepare_call_dispatch_weight(
	user_specified_dispatch_weight: Option<ExplicitOrMaximal<Weight>>,
	weight_from_pre_dispatch_call: ExplicitOrMaximal<Weight>,
	maximal_allowed_weight: Weight,
) -> Weight {
	match user_specified_dispatch_weight.unwrap_or(weight_from_pre_dispatch_call) {
		ExplicitOrMaximal::Explicit(weight) => weight,
		ExplicitOrMaximal::Maximal => maximal_allowed_weight,
	}
}

async fn get_fee<Fee, F, R, E>(fee: Option<Fee>, f: F) -> Result<Fee, String>
where
	Fee: Decode,
	F: FnOnce() -> R,
	R: std::future::Future<Output = Result<Option<Fee>, E>>,
	E: Debug,
{
	match fee {
		Some(fee) => Ok(fee),
		None => match f().await {
			Ok(Some(fee)) => Ok(fee),
			Ok(None) => Err("Failed to estimate message fee. Message is too heavy?".into()),
			Err(error) => Err(format!("Failed to estimate message fee: {:?}", error)),
		},
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn should_have_tests() {
		assert_eq!(true, false)
	}
}
