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
use pallet_bridge_dispatch::{CallOrigin, MessagePayload};
use relay_substrate_client::{Chain, TransactionSignScheme};
use sp_core::{Bytes, Pair};
use sp_runtime::{traits::IdentifyAccount, AccountId32, MultiSignature, MultiSigner};
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

// TODO [ToDr] Use common macro.
macro_rules! select_bridge {
	($bridge: expr, $generic: tt) => {
		match $bridge {
			FullBridge::MillauToRialto => {
				type Source = relay_millau_client::Millau;
				type Target = relay_rialto_client::Rialto;

				#[allow(unused_imports)]
				use bp_millau::TO_MILLAU_ESTIMATE_MESSAGE_FEE_METHOD as ESTIMATE_MESSAGE_FEE_METHOD;
				#[allow(unused_imports)]
				use millau_runtime::rialto_account_ownership_digest as account_ownership_digest;

				#[allow(dead_code)]
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

				#[allow(unused_imports)]
				use bp_rialto::TO_RIALTO_ESTIMATE_MESSAGE_FEE_METHOD as ESTIMATE_MESSAGE_FEE_METHOD;
				#[allow(unused_imports)]
				use rialto_runtime::millau_account_ownership_digest as account_ownership_digest;

				#[allow(dead_code)]
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
	pub fn encode_payload(
		&mut self,
	) -> anyhow::Result<MessagePayload<AccountId32, MultiSigner, MultiSignature, Vec<u8>>> {
		select_bridge!(self.bridge, {
			let SendMessage {
				source_sign,
				target_sign,
				ref mut message,
				dispatch_weight,
				origin,
				bridge,
				..
			} = self;

			let source_sign = source_sign.into_keypair::<Source>()?;
			let target_sign = target_sign.into_keypair::<Target>()?;

			encode_call::preprocess_call::<Source, Target>(message, bridge.bridge_instance_index());
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
			Ok(payload)
		})
	}

	/// Run the command.
	pub async fn run(mut self) -> anyhow::Result<()> {
		select_bridge!(self.bridge, {
			let payload = self.encode_payload()?;

			let source_client = self.source.into_client::<Source>().await?;
			let source_sign = self.source_sign.into_keypair::<Source>()?;

			let lane = self.lane.into();
			let fee = match self.fee {
				Some(fee) => fee,
				None => crate::rialto_millau::estimate_message_delivery_and_dispatch_fee::<
					<Source as relay_substrate_client::ChainWithBalances>::NativeBalance,
					_,
					_,
				>(&source_client, ESTIMATE_MESSAGE_FEE_METHOD, lane, payload.clone())
				.await?
				.map(|v| Balance(v as _))
				.ok_or(anyhow::format_err!(
					"Failed to estimate message fee. Message is too heavy?"
				))?,
			};
			let dispatch_weight = payload.weight;
			let send_message_call = send_message_call(lane, payload, fee);

			source_client
				.submit_signed_extrinsic(source_sign.public().into(), |transaction_nonce| {
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
	user_specified_dispatch_weight: &Option<ExplicitOrMaximal<Weight>>,
	weight_from_pre_dispatch_call: ExplicitOrMaximal<Weight>,
	maximal_allowed_weight: Weight,
) -> Weight {
	match user_specified_dispatch_weight
		.clone()
		.unwrap_or(weight_from_pre_dispatch_call)
	{
		ExplicitOrMaximal::Explicit(weight) => weight,
		ExplicitOrMaximal::Maximal => maximal_allowed_weight,
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use hex_literal::hex;

	#[test]
	fn send_remark_rialto_to_millau() {
		// given
		let mut send_message = SendMessage::from_iter(vec![
			"send-message",
			"RialtoToMillau",
			"--source-port",
			"1234",
			"--source-signer",
			"//Alice",
			"--target-signer",
			"//Bob",
			"remark",
			"--remark-payload",
			"1234",
		]);

		// when
		let payload = send_message.encode_payload().unwrap();

		// then
		assert_eq!(
			payload,
			MessagePayload {
				spec_version: relay_millau_client::Millau::RUNTIME_VERSION.spec_version,
				weight: 1345000,
				origin: CallOrigin::SourceAccount(sp_keyring::AccountKeyring::Alice.to_account_id()),
				call: hex!("0401081234").to_vec(),
			}
		);
	}

	#[test]
	fn send_remark_millau_to_rialto() {
		// given
		let mut send_message = SendMessage::from_iter(vec![
			"send-message",
			"MillauToRialto",
			"--source-port",
			"1234",
			"--source-signer",
			"//Alice",
			"--origin",
			"Target",
			"--target-signer",
			"//Bob",
			"remark",
			"--remark-payload",
			"1234",
		]);

		// when
		let payload = send_message.encode_payload().unwrap();

		// then
		// Since signatures are randomized we extract it from here and only check the rest.
		let signature = match payload.origin {
			CallOrigin::TargetAccount(_, _, ref sig) => sig.clone(),
			_ => panic!("Unexpected `CallOrigin`: {:?}", payload),
		};
		assert_eq!(
			payload,
			MessagePayload {
				spec_version: relay_millau_client::Millau::RUNTIME_VERSION.spec_version,
				weight: 1345000,
				origin: CallOrigin::TargetAccount(
					sp_keyring::AccountKeyring::Alice.to_account_id(),
					sp_keyring::AccountKeyring::Bob.into(),
					signature,
				),
				call: hex!("0701081234").to_vec(),
			}
		);
	}
}
