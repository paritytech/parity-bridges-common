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

//! Substrate-to-substrate relay entrypoint.

#![warn(missing_docs)]

use codec::{Decode, Encode};
use frame_support::weights::{GetDispatchInfo, Weight};
use pallet_bridge_call_dispatch::{CallOrigin, MessagePayload};
use relay_kusama_client::Kusama;
use relay_millau_client::{Millau, SigningParams as MillauSigningParams};
use relay_rialto_client::{Rialto, SigningParams as RialtoSigningParams};
use relay_substrate_client::{Chain, ConnectionParams, TransactionSignScheme};
use relay_utils::initialize::initialize_relay;
use sp_core::{Bytes, Pair};
use sp_runtime::traits::IdentifyAccount;

/// Kusama node client.
pub type KusamaClient = relay_substrate_client::Client<Kusama>;
/// Millau node client.
pub type MillauClient = relay_substrate_client::Client<Millau>;
/// Rialto node client.
pub type RialtoClient = relay_substrate_client::Client<Rialto>;

mod cli;
mod headers_initialize;
mod headers_maintain;
mod headers_pipeline;
mod headers_target;
mod messages_lane;
mod messages_source;
mod messages_target;
mod millau_headers_to_rialto;
mod millau_messages_to_rialto;
mod rialto_headers_to_millau;
mod rialto_messages_to_millau;

fn main() {
	initialize_relay();

	let result = async_std::task::block_on(run_command(cli::parse_args()));
	if let Err(error) = result {
		log::error!(target: "bridge", "Failed to start relay: {}", error);
	}
}

async fn run_command(command: cli::Command) -> Result<(), String> {
	match command {
		cli::Command::InitializeMillauHeadersBridgeInRialto {
			millau,
			rialto,
			rialto_sign,
			millau_bridge_params,
		} => {
			let millau_client = MillauClient::new(ConnectionParams {
				host: millau.millau_host,
				port: millau.millau_port,
			})
			.await?;
			let rialto_client = RialtoClient::new(ConnectionParams {
				host: rialto.rialto_host,
				port: rialto.rialto_port,
			})
			.await?;
			let rialto_sign = RialtoSigningParams::from_suri(
				&rialto_sign.rialto_signer,
				rialto_sign.rialto_signer_password.as_deref(),
			)
			.map_err(|e| format!("Failed to parse rialto-signer: {:?}", e))?;
			let rialto_signer_next_index = rialto_client
				.next_account_index(rialto_sign.signer.public().into())
				.await?;

			headers_initialize::initialize(
				millau_client,
				rialto_client.clone(),
				millau_bridge_params.millau_initial_header,
				millau_bridge_params.millau_initial_authorities,
				millau_bridge_params.millau_initial_authorities_set_id,
				move |initialization_data| {
					Ok(Bytes(
						Rialto::sign_transaction(
							&rialto_client,
							&rialto_sign.signer,
							rialto_signer_next_index,
							rialto_runtime::SudoCall::sudo(Box::new(
								rialto_runtime::BridgeMillauCall::initialize(initialization_data).into(),
							))
							.into(),
						)
						.encode(),
					))
				},
			)
			.await;
		}
		cli::Command::MillauHeadersToRialto {
			millau,
			rialto,
			rialto_sign,
			prometheus_params,
		} => {
			let millau_client = MillauClient::new(ConnectionParams {
				host: millau.millau_host,
				port: millau.millau_port,
			})
			.await?;
			let rialto_client = RialtoClient::new(ConnectionParams {
				host: rialto.rialto_host,
				port: rialto.rialto_port,
			})
			.await?;
			let rialto_sign = RialtoSigningParams::from_suri(
				&rialto_sign.rialto_signer,
				rialto_sign.rialto_signer_password.as_deref(),
			)
			.map_err(|e| format!("Failed to parse rialto-signer: {:?}", e))?;
			millau_headers_to_rialto::run(millau_client, rialto_client, rialto_sign, prometheus_params.into()).await;
		}
		cli::Command::InitializeRialtoHeadersBridgeInMillau {
			rialto,
			millau,
			millau_sign,
			rialto_bridge_params,
		} => {
			let rialto_client = RialtoClient::new(ConnectionParams {
				host: rialto.rialto_host,
				port: rialto.rialto_port,
			})
			.await?;
			let millau_client = MillauClient::new(ConnectionParams {
				host: millau.millau_host,
				port: millau.millau_port,
			})
			.await?;
			let millau_sign = MillauSigningParams::from_suri(
				&millau_sign.millau_signer,
				millau_sign.millau_signer_password.as_deref(),
			)
			.map_err(|e| format!("Failed to parse millau-signer: {:?}", e))?;
			let millau_signer_next_index = millau_client
				.next_account_index(millau_sign.signer.public().into())
				.await?;

			headers_initialize::initialize(
				rialto_client,
				millau_client.clone(),
				rialto_bridge_params.rialto_initial_header,
				rialto_bridge_params.rialto_initial_authorities,
				rialto_bridge_params.rialto_initial_authorities_set_id,
				move |initialization_data| {
					Ok(Bytes(
						Millau::sign_transaction(
							&millau_client,
							&millau_sign.signer,
							millau_signer_next_index,
							millau_runtime::SudoCall::sudo(Box::new(
								millau_runtime::BridgeRialtoCall::initialize(initialization_data).into(),
							))
							.into(),
						)
						.encode(),
					))
				},
			)
			.await;
		}
		cli::Command::RialtoHeadersToMillau {
			rialto,
			millau,
			millau_sign,
			prometheus_params,
		} => {
			let rialto_client = RialtoClient::new(ConnectionParams {
				host: rialto.rialto_host,
				port: rialto.rialto_port,
			})
			.await?;
			let millau_client = MillauClient::new(ConnectionParams {
				host: millau.millau_host,
				port: millau.millau_port,
			})
			.await?;
			let millau_sign = MillauSigningParams::from_suri(
				&millau_sign.millau_signer,
				millau_sign.millau_signer_password.as_deref(),
			)
			.map_err(|e| format!("Failed to parse millau-signer: {:?}", e))?;

			rialto_headers_to_millau::run(rialto_client, millau_client, millau_sign, prometheus_params.into()).await;
		}
		cli::Command::MillauMessagesToRialto {
			millau,
			millau_sign,
			rialto,
			rialto_sign,
			prometheus_params,
			lane,
		} => {
			let millau_client = MillauClient::new(ConnectionParams {
				host: millau.millau_host,
				port: millau.millau_port,
			})
			.await?;
			let millau_sign = MillauSigningParams::from_suri(
				&millau_sign.millau_signer,
				millau_sign.millau_signer_password.as_deref(),
			)
			.map_err(|e| format!("Failed to parse millau-signer: {:?}", e))?;
			let rialto_client = RialtoClient::new(ConnectionParams {
				host: rialto.rialto_host,
				port: rialto.rialto_port,
			})
			.await?;
			let rialto_sign = RialtoSigningParams::from_suri(
				&rialto_sign.rialto_signer,
				rialto_sign.rialto_signer_password.as_deref(),
			)
			.map_err(|e| format!("Failed to parse rialto-signer: {:?}", e))?;

			millau_messages_to_rialto::run(
				millau_client,
				millau_sign,
				rialto_client,
				rialto_sign,
				lane.into(),
				prometheus_params.into(),
			);
		}
		cli::Command::SubmitMillauToRialtoMessage {
			millau,
			millau_sign,
			rialto_sign,
			lane,
			message,
			fee,
			origin,
			..
		} => {
			let millau_client = MillauClient::new(ConnectionParams {
				host: millau.millau_host,
				port: millau.millau_port,
			})
			.await?;
			let millau_sign = MillauSigningParams::from_suri(
				&millau_sign.millau_signer,
				millau_sign.millau_signer_password.as_deref(),
			)
			.map_err(|e| format!("Failed to parse millau-signer: {:?}", e))?;
			let rialto_sign = RialtoSigningParams::from_suri(
				&rialto_sign.rialto_signer,
				rialto_sign.rialto_signer_password.as_deref(),
			)
			.map_err(|e| format!("Failed to parse rialto-signer: {:?}", e))?;

			let rialto_call = match message {
				cli::ToRialtoMessage::Remark => rialto_runtime::SystemCall::remark(
					format!(
						"Unix time: {}",
						std::time::SystemTime::now()
							.duration_since(std::time::SystemTime::UNIX_EPOCH)
							.unwrap_or_default()
							.as_secs(),
					)
					.as_bytes()
					.to_vec(),
				)
				.into(),
				cli::ToRialtoMessage::Transfer {
					ref recipient,
					ref amount,
				} => rialto_runtime::Call::Balances(rialto_runtime::BalancesCall::transfer(recipient.clone(), *amount)),
				cli::ToRialtoMessage::MaximalSizeRemark => rialto_runtime::SystemCall::remark(vec![
					42u8;
					compute_maximal_size_of_remark(bp_millau::max_extrinsic_size(), bp_rialto::max_extrinsic_size(),)
						as _
				])
				.into(),
				cli::ToRialtoMessage::MaximalWeightRemark => {
					rialto_runtime::SystemCall::remark(b"MaximalWeightRemark".to_vec()).into()
				}
			};

			let rialto_call_weight = match message {
				cli::ToRialtoMessage::MaximalWeightRemark => {
					compute_maximal_dispatch_weight(bp_rialto::max_extrinsic_weight())
				}
				_ => rialto_call.get_dispatch_info().weight,
			};
			let millau_sender_public: bp_millau::AccountSigner = millau_sign.signer.public().clone().into();
			let millau_account_id: bp_millau::AccountId = millau_sender_public.into_account();
			let rialto_origin_public = rialto_sign.signer.public();

			let payload = match origin {
				cli::Origins::Source => MessagePayload {
					spec_version: rialto_runtime::VERSION.spec_version,
					weight: rialto_call_weight,
					origin: CallOrigin::SourceAccount(millau_account_id),
					call: rialto_call.encode(),
				},
				cli::Origins::Target => {
					let digest = millau_runtime::rialto_account_ownership_digest(
						&rialto_call,
						millau_account_id.clone(),
						rialto_runtime::VERSION.spec_version,
					);

					let digest_signature = rialto_sign.signer.sign(&digest);

					MessagePayload {
						spec_version: rialto_runtime::VERSION.spec_version,
						weight: rialto_call_weight,
						origin: CallOrigin::TargetAccount(
							millau_account_id,
							rialto_origin_public.into(),
							digest_signature.into(),
						),
						call: rialto_call.encode(),
					}
				}
			};

			let lane = lane.into();
			let fee = match fee {
				Some(fee) => fee,
				None => match estimate_message_delivery_and_dispatch_fee(
					&millau_client,
					bp_rialto::TO_RIALTO_ESTIMATE_MESSAGE_FEE_METHOD,
					lane,
					payload.clone(),
				)
				.await
				{
					Ok(Some(fee)) => fee,
					Ok(None) => return Err("Failed to estimate message fee. Message is too heavy?".into()),
					Err(error) => return Err(format!("Failed to estimate message fee: {:?}", error)),
				},
			};

			log::info!(target: "bridge", "Sending message to Rialto. Fee: {}", fee);

			let millau_call = millau_runtime::Call::BridgeRialtoMessageLane(
				millau_runtime::MessageLaneCall::send_message(lane, payload, fee),
			);

			let signed_millau_call = Millau::sign_transaction(
				&millau_client,
				&millau_sign.signer,
				millau_client
					.next_account_index(millau_sign.signer.public().clone().into())
					.await?,
				millau_call,
			);

			millau_client
				.submit_extrinsic(Bytes(signed_millau_call.encode()))
				.await?;
		}
		cli::Command::RialtoMessagesToMillau {
			rialto,
			rialto_sign,
			millau,
			millau_sign,
			prometheus_params,
			lane,
		} => {
			let rialto_client = RialtoClient::new(ConnectionParams {
				host: rialto.rialto_host,
				port: rialto.rialto_port,
			})
			.await?;
			let rialto_sign = RialtoSigningParams::from_suri(
				&rialto_sign.rialto_signer,
				rialto_sign.rialto_signer_password.as_deref(),
			)
			.map_err(|e| format!("Failed to parse rialto-signer: {:?}", e))?;
			let millau_client = MillauClient::new(ConnectionParams {
				host: millau.millau_host,
				port: millau.millau_port,
			})
			.await?;
			let millau_sign = MillauSigningParams::from_suri(
				&millau_sign.millau_signer,
				millau_sign.millau_signer_password.as_deref(),
			)
			.map_err(|e| format!("Failed to parse millau-signer: {:?}", e))?;

			rialto_messages_to_millau::run(
				rialto_client,
				rialto_sign,
				millau_client,
				millau_sign,
				lane.into(),
				prometheus_params.into(),
			);
		}
		cli::Command::SubmitRialtoToMillauMessage {
			rialto,
			rialto_sign,
			millau_sign,
			lane,
			message,
			fee,
			origin,
			..
		} => {
			let rialto_client = RialtoClient::new(ConnectionParams {
				host: rialto.rialto_host,
				port: rialto.rialto_port,
			})
			.await?;
			let rialto_sign = RialtoSigningParams::from_suri(
				&rialto_sign.rialto_signer,
				rialto_sign.rialto_signer_password.as_deref(),
			)
			.map_err(|e| format!("Failed to parse rialto-signer: {:?}", e))?;
			let millau_sign = MillauSigningParams::from_suri(
				&millau_sign.millau_signer,
				millau_sign.millau_signer_password.as_deref(),
			)
			.map_err(|e| format!("Failed to parse millau-signer: {:?}", e))?;

			let millau_call = match message {
				cli::ToMillauMessage::Remark => millau_runtime::Call::System(millau_runtime::SystemCall::remark(
					format!(
						"Unix time: {}",
						std::time::SystemTime::now()
							.duration_since(std::time::SystemTime::UNIX_EPOCH)
							.unwrap_or_default()
							.as_secs(),
					)
					.as_bytes()
					.to_vec(),
				)),
				cli::ToMillauMessage::Transfer {
					ref recipient,
					ref amount,
				} => millau_runtime::Call::Balances(millau_runtime::BalancesCall::transfer(recipient.clone(), *amount)),
				cli::ToMillauMessage::MaximalSizeRemark => millau_runtime::SystemCall::remark(vec![
					42u8;
					compute_maximal_size_of_remark(bp_rialto::max_extrinsic_size(), bp_millau::max_extrinsic_size(),)
						as _
				])
				.into(),
				cli::ToMillauMessage::MaximalWeightRemark => {
					millau_runtime::SystemCall::remark(b"MaximalWeightRemark".to_vec()).into()
				}
			};

			let millau_call_weight = match message {
				cli::ToMillauMessage::MaximalWeightRemark => {
					compute_maximal_dispatch_weight(bp_millau::max_extrinsic_weight())
				}
				_ => millau_call.get_dispatch_info().weight,
			};
			let rialto_sender_public: bp_rialto::AccountSigner = rialto_sign.signer.public().clone().into();
			let rialto_account_id: bp_rialto::AccountId = rialto_sender_public.into_account();
			let millau_origin_public = millau_sign.signer.public();

			let payload = match origin {
				cli::Origins::Source => MessagePayload {
					spec_version: millau_runtime::VERSION.spec_version,
					weight: millau_call_weight,
					origin: CallOrigin::SourceAccount(rialto_account_id),
					call: millau_call.encode(),
				},
				cli::Origins::Target => {
					let digest = rialto_runtime::millau_account_ownership_digest(
						&millau_call,
						rialto_account_id.clone(),
						millau_runtime::VERSION.spec_version,
					);

					let digest_signature = millau_sign.signer.sign(&digest);

					MessagePayload {
						spec_version: millau_runtime::VERSION.spec_version,
						weight: millau_call_weight,
						origin: CallOrigin::TargetAccount(
							rialto_account_id,
							millau_origin_public.into(),
							digest_signature.into(),
						),
						call: millau_call.encode(),
					}
				}
			};

			let lane = lane.into();
			let fee = match fee {
				Some(fee) => fee,
				None => match estimate_message_delivery_and_dispatch_fee(
					&rialto_client,
					bp_millau::TO_MILLAU_ESTIMATE_MESSAGE_FEE_METHOD,
					lane,
					payload.clone(),
				)
				.await
				{
					Ok(Some(fee)) => fee,
					Ok(None) => return Err("Failed to estimate message fee. Message is too heavy?".into()),
					Err(error) => return Err(format!("Failed to estimate message fee: {:?}", error)),
				},
			};

			log::info!(target: "bridge", "Sending message to Millau. Fee: {}", fee);

			let rialto_call = rialto_runtime::Call::BridgeMillauMessageLane(
				rialto_runtime::MessageLaneCall::send_message(lane, payload, fee),
			);

			let signed_rialto_call = Rialto::sign_transaction(
				&rialto_client,
				&rialto_sign.signer,
				rialto_client
					.next_account_index(rialto_sign.signer.public().clone().into())
					.await?,
				rialto_call,
			);

			rialto_client
				.submit_extrinsic(Bytes(signed_rialto_call.encode()))
				.await?;
		}
	}

	Ok(())
}

async fn estimate_message_delivery_and_dispatch_fee<Fee: Decode, C: Chain, P: Encode>(
	client: &relay_substrate_client::Client<C>,
	estimate_fee_method: &str,
	lane: bp_message_lane::LaneId,
	payload: P,
) -> Result<Option<Fee>, relay_substrate_client::Error> {
	let encoded_response = client
		.state_call(estimate_fee_method.into(), (lane, payload).encode().into(), None)
		.await?;
	let decoded_response: Option<Fee> =
		Decode::decode(&mut &encoded_response.0[..]).map_err(relay_substrate_client::Error::ResponseParseFailed)?;
	Ok(decoded_response)
}

fn compute_maximal_size_of_remark(maximal_source_extrinsic_size: u32, maximal_target_extrinsic_size: u32) -> u32 {
	// assume that both signed extensions and other arguments fit 1KB
	let service_tx_bytes_on_source_chain = 1024;
	let maximal_source_extrinsic_size = maximal_source_extrinsic_size - service_tx_bytes_on_source_chain;
	let maximal_call_size =
		bridge_runtime_common::messages::target::maximal_incoming_message_size(maximal_target_extrinsic_size);
	let maximal_call_size = if maximal_call_size > maximal_source_extrinsic_size {
		maximal_source_extrinsic_size
	} else {
		maximal_call_size
	};

	// bytes in remark Call encoding that are used to encode everything except arguments
	let service_bytes = 1 + 1 + 4;
	maximal_call_size - service_bytes
}

fn compute_maximal_dispatch_weight(maximal_extrinsic_weight: Weight) -> Weight {
	bridge_runtime_common::messages::target::maximal_incoming_message_dispatch_weight(maximal_extrinsic_weight)
}

#[cfg(test)]
mod tests {
	use super::*;
	use sp_core::Pair;
	use sp_runtime::traits::{IdentifyAccount, Verify};

	#[test]
	fn millau_signature_is_valid_on_rialto() {
		let millau_sign = relay_millau_client::SigningParams::from_suri("//Dave", None).unwrap();

		let call = rialto_runtime::Call::System(rialto_runtime::SystemCall::remark(vec![]));

		let millau_public: bp_millau::AccountSigner = millau_sign.signer.public().clone().into();
		let millau_account_id: bp_millau::AccountId = millau_public.into_account();

		let digest = millau_runtime::rialto_account_ownership_digest(
			&call,
			millau_account_id,
			rialto_runtime::VERSION.spec_version,
		);

		let rialto_signer = relay_rialto_client::SigningParams::from_suri("//Dave", None).unwrap();
		let signature = rialto_signer.signer.sign(&digest);

		assert!(signature.verify(&digest[..], &rialto_signer.signer.public()));
	}

	#[test]
	fn rialto_signature_is_valid_on_millau() {
		let rialto_sign = relay_rialto_client::SigningParams::from_suri("//Dave", None).unwrap();

		let call = millau_runtime::Call::System(millau_runtime::SystemCall::remark(vec![]));

		let rialto_public: bp_rialto::AccountSigner = rialto_sign.signer.public().clone().into();
		let rialto_account_id: bp_rialto::AccountId = rialto_public.into_account();

		let digest = rialto_runtime::millau_account_ownership_digest(
			&call,
			rialto_account_id,
			millau_runtime::VERSION.spec_version,
		);

		let millau_signer = relay_millau_client::SigningParams::from_suri("//Dave", None).unwrap();
		let signature = millau_signer.signer.sign(&digest);

		assert!(signature.verify(&digest[..], &millau_signer.signer.public()));
	}

	#[test]
	fn maximal_size_remark_to_millau_is_generated_correctly() {
		use bp_message_lane::source_chain::TargetHeaderChain;
		use rialto_runtime::millau_messages::Millau;

		let maximal_remark_size =
			compute_maximal_size_of_remark(bp_rialto::max_extrinsic_size(), bp_millau::max_extrinsic_size());

		let call: millau_runtime::Call = millau_runtime::SystemCall::remark(vec![42; maximal_remark_size as _]).into();
		let payload = pallet_bridge_call_dispatch::MessagePayload {
			spec_version: Default::default(),
			weight: call.get_dispatch_info().weight,
			origin: pallet_bridge_call_dispatch::CallOrigin::SourceRoot,
			call: call.encode(),
		};
		assert_eq!(Millau::verify_message(&payload), Ok(()));

		let call: millau_runtime::Call =
			millau_runtime::SystemCall::remark(vec![42; (maximal_remark_size + 1) as _]).into();
		let payload = pallet_bridge_call_dispatch::MessagePayload {
			spec_version: Default::default(),
			weight: call.get_dispatch_info().weight,
			origin: pallet_bridge_call_dispatch::CallOrigin::SourceRoot,
			call: call.encode(),
		};
		assert!(Millau::verify_message(&payload).is_err());
	}

	#[test]
	fn maximal_size_remark_to_rialto_is_generated_correctly() {
		assert!(
			bridge_runtime_common::messages::target::maximal_incoming_message_size(
				bp_rialto::max_extrinsic_size()
			) > bp_millau::max_extrinsic_size(),
			"We can't actually send maximal messages to Rialto from Millau, because Millau extrinsics can't be that large",
		)
	}

	#[test]
	fn maximal_weight_remark_to_millau_is_generated_correctly() {
		use bp_message_lane::source_chain::TargetHeaderChain;
		use rialto_runtime::millau_messages::Millau;

		let maximal_dispatch_weight = compute_maximal_dispatch_weight(bp_millau::max_extrinsic_weight());
		let call: millau_runtime::Call = rialto_runtime::SystemCall::remark(vec![]).into();

		let payload = pallet_bridge_call_dispatch::MessagePayload {
			spec_version: Default::default(),
			weight: maximal_dispatch_weight,
			origin: pallet_bridge_call_dispatch::CallOrigin::SourceRoot,
			call: call.encode(),
		};
		assert_eq!(Millau::verify_message(&payload), Ok(()));

		let payload = pallet_bridge_call_dispatch::MessagePayload {
			spec_version: Default::default(),
			weight: maximal_dispatch_weight + 1,
			origin: pallet_bridge_call_dispatch::CallOrigin::SourceRoot,
			call: call.encode(),
		};
		assert!(Millau::verify_message(&payload).is_err());
	}

	#[test]
	fn maximal_weight_fill_block_to_rialto_is_generated_correctly() {
		use bp_message_lane::source_chain::TargetHeaderChain;
		use millau_runtime::rialto_messages::Rialto;

		let maximal_dispatch_weight = compute_maximal_dispatch_weight(bp_rialto::max_extrinsic_weight());
		let call: rialto_runtime::Call = millau_runtime::SystemCall::remark(vec![]).into();

		let payload = pallet_bridge_call_dispatch::MessagePayload {
			spec_version: Default::default(),
			weight: maximal_dispatch_weight,
			origin: pallet_bridge_call_dispatch::CallOrigin::SourceRoot,
			call: call.encode(),
		};
		assert_eq!(Rialto::verify_message(&payload), Ok(()));

		let payload = pallet_bridge_call_dispatch::MessagePayload {
			spec_version: Default::default(),
			weight: maximal_dispatch_weight + 1,
			origin: pallet_bridge_call_dispatch::CallOrigin::SourceRoot,
			call: call.encode(),
		};
		assert!(Rialto::verify_message(&payload).is_err());
	}
}
