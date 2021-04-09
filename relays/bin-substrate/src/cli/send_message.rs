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

use crate::cli::bridge::FullBridge;
use crate::cli::encode_call::{self, CliEncodeCall};
use crate::cli::{
	Balance, CliChain, ExplicitOrMaximal, HexBytes, HexLaneId, Origins, SourceConnectionParams, SourceSigningParams,
	TargetSigningParams,
};
use bp_messages::LaneId;
use codec::Encode;
use frame_support::{dispatch::GetDispatchInfo, weights::Weight};
use pallet_bridge_dispatch::CallOrigin;
use relay_substrate_client::{Chain, TransactionSignScheme};
use sp_core::{Bytes, Pair};
use sp_runtime::{traits::IdentifyAccount, MultiSigner};
use structopt::StructOpt;

/// Send bridge message.
#[derive(StructOpt)]
pub struct SendMessage {
	/// A bridge instance to encode call for.
	#[structopt(possible_values = &FullBridge::variants(), case_insensitive = true)]
	bridge: FullBridge,
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

macro_rules! select_bridge {
	($bridge: expr, $generic: tt) => {
		match $bridge {
			FullBridge::MillauToRialto => {
				type Source = relay_millau_client::Millau;
				type Target = relay_rialto_client::Rialto;

				use bp_millau::TO_MILLAU_ESTIMATE_MESSAGE_FEE_METHOD as ESTIMATE_MESSAGE_FEE_METHOD;
				use millau_runtime::rialto_account_ownership_digest as account_ownership_digest;

				fn send_message_call(
					lane: LaneId,
					payload: <Source as CliChain>::MessagePayload,
					fee: Balance,
				) -> millau_runtime::Call {
					millau_runtime::Call::BridgeRialtoMessages(millau_runtime::MessagesCall::send_message(
						lane,
						payload,
						fee.cast(),
					))
				}

				$generic
			}
			FullBridge::RialtoToMillau => {
				type Source = relay_rialto_client::Rialto;
				type Target = relay_millau_client::Millau;

				use bp_rialto::TO_RIALTO_ESTIMATE_MESSAGE_FEE_METHOD as ESTIMATE_MESSAGE_FEE_METHOD;
				use rialto_runtime::millau_account_ownership_digest as account_ownership_digest;

				fn send_message_call(
					lane: LaneId,
					payload: <Source as CliChain>::MessagePayload,
					fee: Balance,
				) -> rialto_runtime::Call {
					rialto_runtime::Call::BridgeMillauMessages(rialto_runtime::MessagesCall::send_message(
						lane, payload, fee.0,
					))
				}

				$generic
			}
		}
	};
}

impl SendMessage {
	/// Run the command.
	pub async fn run(self) -> anyhow::Result<()> {
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
			let source_client = source.into_client::<Source>().await?;
			let source_sign = source_sign.into_keypair::<Source>()?;
			let target_sign = target_sign.into_keypair::<Target>()?;

			encode_call::preprocess_call::<Source, Target>(&mut message, bridge.bridge_instance_index());
			let target_call = Target::encode_call(&message)?;

			let payload = {
				let target_call_weight = prepare_call_dispatch_weight(
					dispatch_weight,
					ExplicitOrMaximal::Explicit(target_call.get_dispatch_info().weight),
					crate::rialto_millau::compute_maximal_message_dispatch_weight(Target::max_extrinsic_weight()),
				);
				let source_sender_public: MultiSigner = source_sign.public().into();
				let source_account_id = source_sender_public.into_account();

				crate::rialto_millau::message_payload(
					Target::RUNTIME_VERSION.spec_version,
					target_call_weight,
					match origin {
						Origins::Source => CallOrigin::SourceAccount(source_account_id),
						Origins::Target => {
							let digest = account_ownership_digest(
								&target_call,
								source_account_id.clone(),
								Target::RUNTIME_VERSION.spec_version,
							);
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
			let fee = get_fee(fee, || async {
				crate::rialto_millau::estimate_message_delivery_and_dispatch_fee::<
					<Source as relay_substrate_client::ChainWithBalances>::NativeBalance,
					_,
					_,
				>(&source_client, ESTIMATE_MESSAGE_FEE_METHOD, lane, payload.clone())
				.await
				.map(|v| v.map(|v| Balance(v as _)))
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

async fn get_fee<F, R, E>(fee: Option<Balance>, f: F) -> anyhow::Result<Balance>
where
	F: FnOnce() -> R,
	R: std::future::Future<Output = Result<Option<Balance>, E>>,
	E: Send + Sync + std::error::Error + 'static,
{
	match fee {
		Some(fee) => Ok(fee),
		None => f().await?.ok_or(anyhow::format_err!(
			"Failed to estimate message fee. Message is too heavy?"
		)),
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
