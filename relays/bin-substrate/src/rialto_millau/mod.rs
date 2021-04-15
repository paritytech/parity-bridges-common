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

//! Rialto <> Millau Bridge commands.

pub mod millau_headers_to_rialto;
pub mod millau_messages_to_rialto;
pub mod rialto_headers_to_millau;
pub mod rialto_messages_to_millau;
pub mod rococo_headers_to_westend;
pub mod westend_headers_to_millau;
pub mod westend_headers_to_rococo;

use crate::cli::{
	bridge,
	encode_call::{self, Call, CliEncodeCall},
	encode_message, send_message, CliChain,
};
use codec::Decode;
use frame_support::weights::{GetDispatchInfo, Weight};
use pallet_bridge_dispatch::{CallOrigin, MessagePayload};
use relay_millau_client::Millau;
use relay_rialto_client::Rialto;
use relay_rococo_client::Rococo;
use relay_utils::metrics::{FloatJsonValueMetric, MetricsParams};
use relay_westend_client::Westend;
use sp_version::RuntimeVersion;

pub(crate) fn add_polkadot_kusama_price_metrics<T: finality_relay::FinalitySyncPipeline>(
	params: MetricsParams,
) -> anyhow::Result<MetricsParams> {
	Ok(
		relay_utils::relay_metrics(Some(finality_relay::metrics_prefix::<T>()), params)
			// Polkadot/Kusama prices are added as metrics here, because atm we don't have Polkadot <-> Kusama
			// relays, but we want to test metrics/dashboards in advance
			.standalone_metric(|registry, prefix| {
				FloatJsonValueMetric::new(
					registry,
					prefix,
					"https://api.coingecko.com/api/v3/simple/price?ids=Polkadot&vs_currencies=usd".into(),
					"$.polkadot.usd".into(),
					"polkadot_price".into(),
					"Polkadot price in USD".into(),
				)
			})
			.map_err(|e| anyhow::format_err!("{}", e))?
			.standalone_metric(|registry, prefix| {
				FloatJsonValueMetric::new(
					registry,
					prefix,
					"https://api.coingecko.com/api/v3/simple/price?ids=Kusama&vs_currencies=usd".into(),
					"$.kusama.usd".into(),
					"kusama_price".into(),
					"Kusama price in USD".into(),
				)
			})
			.map_err(|e| anyhow::format_err!("{}", e))?
			.into_params(),
	)
}

impl CliEncodeCall for Millau {
	fn max_extrinsic_size() -> u32 {
		bp_millau::max_extrinsic_size()
	}

	fn encode_call(call: &Call) -> anyhow::Result<Self::Call> {
		Ok(match call {
			Call::Raw { data } => Decode::decode(&mut &*data.0)?,
			Call::Remark { remark_payload, .. } => millau_runtime::Call::System(millau_runtime::SystemCall::remark(
				remark_payload.as_ref().map(|x| x.0.clone()).unwrap_or_default(),
			)),
			Call::Transfer { recipient, amount } => millau_runtime::Call::Balances(
				millau_runtime::BalancesCall::transfer(recipient.raw_id(), amount.cast()),
			),
			Call::BridgeSendMessage {
				lane,
				payload,
				fee,
				bridge_instance_index,
			} => match *bridge_instance_index {
				bridge::MILLAU_TO_RIALTO_INDEX => {
					let payload = Decode::decode(&mut &*payload.0)?;
					millau_runtime::Call::BridgeRialtoMessages(millau_runtime::MessagesCall::send_message(
						lane.0,
						payload,
						fee.cast(),
					))
				}
				_ => anyhow::bail!(
					"Unsupported target bridge pallet with instance index: {}",
					bridge_instance_index
				),
			},
		})
	}
}

impl CliChain for Millau {
	const RUNTIME_VERSION: RuntimeVersion = millau_runtime::VERSION;

	type KeyPair = sp_core::sr25519::Pair;
	type MessagePayload = MessagePayload<bp_millau::AccountId, bp_rialto::AccountSigner, bp_rialto::Signature, Vec<u8>>;

	fn ss58_format() -> u16 {
		millau_runtime::SS58Prefix::get() as u16
	}

	fn max_extrinsic_weight() -> Weight {
		bp_millau::max_extrinsic_weight()
	}

	// TODO [#854|#843] support multiple bridges?
	fn encode_message(message: encode_message::MessagePayload) -> Result<Self::MessagePayload, String> {
		match message {
			encode_message::MessagePayload::Raw { data } => MessagePayload::decode(&mut &*data.0)
				.map_err(|e| format!("Failed to decode Millau's MessagePayload: {:?}", e)),
			encode_message::MessagePayload::Call { mut call, mut sender } => {
				type Source = Millau;
				type Target = Rialto;

				sender.enforce_chain::<Source>();
				let spec_version = Target::RUNTIME_VERSION.spec_version;
				let origin = CallOrigin::SourceAccount(sender.raw_id());
				encode_call::preprocess_call::<Source, Target>(&mut call, bridge::MILLAU_TO_RIALTO_INDEX);
				let call = Target::encode_call(&call).map_err(|e| e.to_string())?;
				let weight = call.get_dispatch_info().weight;

				Ok(send_message::message_payload(spec_version, weight, origin, &call))
			}
		}
	}
}

impl CliEncodeCall for Rialto {
	fn max_extrinsic_size() -> u32 {
		bp_rialto::max_extrinsic_size()
	}

	fn encode_call(call: &Call) -> anyhow::Result<Self::Call> {
		Ok(match call {
			Call::Raw { data } => Decode::decode(&mut &*data.0)?,
			Call::Remark { remark_payload, .. } => rialto_runtime::Call::System(rialto_runtime::SystemCall::remark(
				remark_payload.as_ref().map(|x| x.0.clone()).unwrap_or_default(),
			)),
			Call::Transfer { recipient, amount } => {
				rialto_runtime::Call::Balances(rialto_runtime::BalancesCall::transfer(recipient.raw_id(), amount.0))
			}
			Call::BridgeSendMessage {
				lane,
				payload,
				fee,
				bridge_instance_index,
			} => match *bridge_instance_index {
				bridge::RIALTO_TO_MILLAU_INDEX => {
					let payload = Decode::decode(&mut &*payload.0)?;
					rialto_runtime::Call::BridgeMillauMessages(rialto_runtime::MessagesCall::send_message(
						lane.0, payload, fee.0,
					))
				}
				_ => anyhow::bail!(
					"Unsupported target bridge pallet with instance index: {}",
					bridge_instance_index
				),
			},
		})
	}
}

impl CliChain for Rialto {
	const RUNTIME_VERSION: RuntimeVersion = rialto_runtime::VERSION;

	type KeyPair = sp_core::sr25519::Pair;
	type MessagePayload = MessagePayload<bp_rialto::AccountId, bp_millau::AccountSigner, bp_millau::Signature, Vec<u8>>;

	fn ss58_format() -> u16 {
		rialto_runtime::SS58Prefix::get() as u16
	}

	fn max_extrinsic_weight() -> Weight {
		bp_rialto::max_extrinsic_weight()
	}

	fn encode_message(message: encode_message::MessagePayload) -> Result<Self::MessagePayload, String> {
		match message {
			encode_message::MessagePayload::Raw { data } => MessagePayload::decode(&mut &*data.0)
				.map_err(|e| format!("Failed to decode Rialto's MessagePayload: {:?}", e)),
			encode_message::MessagePayload::Call { mut call, mut sender } => {
				type Source = Rialto;
				type Target = Millau;

				sender.enforce_chain::<Source>();
				let spec_version = Target::RUNTIME_VERSION.spec_version;
				let origin = CallOrigin::SourceAccount(sender.raw_id());
				encode_call::preprocess_call::<Source, Target>(&mut call, bridge::RIALTO_TO_MILLAU_INDEX);
				let call = Target::encode_call(&call).map_err(|e| e.to_string())?;
				let weight = call.get_dispatch_info().weight;

				Ok(send_message::message_payload(spec_version, weight, origin, &call))
			}
		}
	}
}

impl CliChain for Westend {
	const RUNTIME_VERSION: RuntimeVersion = bp_westend::VERSION;

	type KeyPair = sp_core::sr25519::Pair;
	type MessagePayload = ();

	fn ss58_format() -> u16 {
		42
	}

	fn max_extrinsic_weight() -> Weight {
		0
	}

	fn encode_message(_message: encode_message::MessagePayload) -> Result<Self::MessagePayload, String> {
		Err("Sending messages from Westend is not yet supported.".into())
	}
}

impl CliChain for Rococo {
	const RUNTIME_VERSION: RuntimeVersion = bp_rococo::VERSION;

	type KeyPair = sp_core::sr25519::Pair;
	type MessagePayload = ();

	fn ss58_format() -> u16 {
		42
	}

	fn max_extrinsic_weight() -> Weight {
		0
	}

	fn encode_message(_message: encode_message::MessagePayload) -> Result<Self::MessagePayload, String> {
		Err("Sending messages from Rococo is not yet supported.".into())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bp_messages::source_chain::TargetHeaderChain;
	use codec::Encode;
	use relay_substrate_client::TransactionSignScheme;
	use sp_core::Pair;
	use sp_runtime::traits::{IdentifyAccount, Verify};

	#[test]
	fn millau_signature_is_valid_on_rialto() {
		let millau_sign = relay_millau_client::SigningParams::from_string("//Dave", None).unwrap();

		let call = rialto_runtime::Call::System(rialto_runtime::SystemCall::remark(vec![]));

		let millau_public: bp_millau::AccountSigner = millau_sign.public().into();
		let millau_account_id: bp_millau::AccountId = millau_public.into_account();

		let digest = millau_runtime::rialto_account_ownership_digest(
			&call,
			millau_account_id,
			rialto_runtime::VERSION.spec_version,
		);

		let rialto_signer = relay_rialto_client::SigningParams::from_string("//Dave", None).unwrap();
		let signature = rialto_signer.sign(&digest);

		assert!(signature.verify(&digest[..], &rialto_signer.public()));
	}

	#[test]
	fn rialto_signature_is_valid_on_millau() {
		let rialto_sign = relay_rialto_client::SigningParams::from_string("//Dave", None).unwrap();

		let call = millau_runtime::Call::System(millau_runtime::SystemCall::remark(vec![]));

		let rialto_public: bp_rialto::AccountSigner = rialto_sign.public().into();
		let rialto_account_id: bp_rialto::AccountId = rialto_public.into_account();

		let digest = rialto_runtime::millau_account_ownership_digest(
			&call,
			rialto_account_id,
			millau_runtime::VERSION.spec_version,
		);

		let millau_signer = relay_millau_client::SigningParams::from_string("//Dave", None).unwrap();
		let signature = millau_signer.sign(&digest);

		assert!(signature.verify(&digest[..], &millau_signer.public()));
	}

	#[test]
	fn maximal_rialto_to_millau_message_arguments_size_is_computed_correctly() {
		use rialto_runtime::millau_messages::Millau;

		let maximal_remark_size = encode_call::compute_maximal_message_arguments_size(
			bp_rialto::max_extrinsic_size(),
			bp_millau::max_extrinsic_size(),
		);

		let call: millau_runtime::Call = millau_runtime::SystemCall::remark(vec![42; maximal_remark_size as _]).into();
		let payload = send_message::message_payload(
			Default::default(),
			call.get_dispatch_info().weight,
			pallet_bridge_dispatch::CallOrigin::SourceRoot,
			&call,
		);
		assert_eq!(Millau::verify_message(&payload), Ok(()));

		let call: millau_runtime::Call =
			millau_runtime::SystemCall::remark(vec![42; (maximal_remark_size + 1) as _]).into();
		let payload = send_message::message_payload(
			Default::default(),
			call.get_dispatch_info().weight,
			pallet_bridge_dispatch::CallOrigin::SourceRoot,
			&call,
		);
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
	fn maximal_rialto_to_millau_message_dispatch_weight_is_computed_correctly() {
		use rialto_runtime::millau_messages::Millau;

		let maximal_dispatch_weight =
			send_message::compute_maximal_message_dispatch_weight(bp_millau::max_extrinsic_weight());
		let call: millau_runtime::Call = rialto_runtime::SystemCall::remark(vec![]).into();

		let payload = send_message::message_payload(
			Default::default(),
			maximal_dispatch_weight,
			pallet_bridge_dispatch::CallOrigin::SourceRoot,
			&call,
		);
		assert_eq!(Millau::verify_message(&payload), Ok(()));

		let payload = send_message::message_payload(
			Default::default(),
			maximal_dispatch_weight + 1,
			pallet_bridge_dispatch::CallOrigin::SourceRoot,
			&call,
		);
		assert!(Millau::verify_message(&payload).is_err());
	}

	#[test]
	fn maximal_weight_fill_block_to_rialto_is_generated_correctly() {
		use millau_runtime::rialto_messages::Rialto;

		let maximal_dispatch_weight =
			send_message::compute_maximal_message_dispatch_weight(bp_rialto::max_extrinsic_weight());
		let call: rialto_runtime::Call = millau_runtime::SystemCall::remark(vec![]).into();

		let payload = send_message::message_payload(
			Default::default(),
			maximal_dispatch_weight,
			pallet_bridge_dispatch::CallOrigin::SourceRoot,
			&call,
		);
		assert_eq!(Rialto::verify_message(&payload), Ok(()));

		let payload = send_message::message_payload(
			Default::default(),
			maximal_dispatch_weight + 1,
			pallet_bridge_dispatch::CallOrigin::SourceRoot,
			&call,
		);
		assert!(Rialto::verify_message(&payload).is_err());
	}

	#[test]
	fn rialto_tx_extra_bytes_constant_is_correct() {
		let rialto_call = rialto_runtime::Call::System(rialto_runtime::SystemCall::remark(vec![]));
		let rialto_tx = Rialto::sign_transaction(
			Default::default(),
			&sp_keyring::AccountKeyring::Alice.pair(),
			0,
			rialto_call.clone(),
		);
		let extra_bytes_in_transaction = rialto_tx.encode().len() - rialto_call.encode().len();
		assert!(
			bp_rialto::TX_EXTRA_BYTES as usize >= extra_bytes_in_transaction,
			"Hardcoded number of extra bytes in Rialto transaction {} is lower than actual value: {}",
			bp_rialto::TX_EXTRA_BYTES,
			extra_bytes_in_transaction,
		);
	}

	#[test]
	fn millau_tx_extra_bytes_constant_is_correct() {
		let millau_call = millau_runtime::Call::System(millau_runtime::SystemCall::remark(vec![]));
		let millau_tx = Millau::sign_transaction(
			Default::default(),
			&sp_keyring::AccountKeyring::Alice.pair(),
			0,
			millau_call.clone(),
		);
		let extra_bytes_in_transaction = millau_tx.encode().len() - millau_call.encode().len();
		assert!(
			bp_millau::TX_EXTRA_BYTES as usize >= extra_bytes_in_transaction,
			"Hardcoded number of extra bytes in Millau transaction {} is lower than actual value: {}",
			bp_millau::TX_EXTRA_BYTES,
			extra_bytes_in_transaction,
		);
	}
}

#[cfg(test)]
mod rococo_tests {
	use bp_header_chain::justification::GrandpaJustification;
	use codec::Encode;

	#[test]
	fn scale_compatibility_of_bridges_call() {
		// given
		let header = sp_runtime::generic::Header {
			parent_hash: Default::default(),
			number: Default::default(),
			state_root: Default::default(),
			extrinsics_root: Default::default(),
			digest: sp_runtime::generic::Digest { logs: vec![] },
		};
		let justification = GrandpaJustification {
			round: 0,
			commit: finality_grandpa::Commit {
				target_hash: Default::default(),
				target_number: Default::default(),
				precommits: vec![],
			},
			votes_ancestries: vec![],
		};
		let actual = bp_rococo::BridgeGrandpaWestendCall::submit_finality_proof(header.clone(), justification.clone());
		let expected = millau_runtime::BridgeGrandpaRialtoCall::<millau_runtime::Runtime>::submit_finality_proof(
			header,
			justification.clone(),
		);

		// when
		let actual_encoded = actual.encode();
		let expected_encoded = expected.encode();

		// then
		assert_eq!(
			actual_encoded, expected_encoded,
			"Encoding difference. Raw: {:?} vs {:?}",
			actual, expected
		);
	}
}

#[cfg(test)]
mod westend_tests {
	use bp_header_chain::justification::GrandpaJustification;
	use codec::Encode;

	#[test]
	fn scale_compatibility_of_bridges_call() {
		// given
		let header = sp_runtime::generic::Header {
			parent_hash: Default::default(),
			number: Default::default(),
			state_root: Default::default(),
			extrinsics_root: Default::default(),
			digest: sp_runtime::generic::Digest { logs: vec![] },
		};
		let justification = GrandpaJustification {
			round: 0,
			commit: finality_grandpa::Commit {
				target_hash: Default::default(),
				target_number: Default::default(),
				precommits: vec![],
			},
			votes_ancestries: vec![],
		};
		let actual = bp_westend::BridgeGrandpaRococoCall::submit_finality_proof(header.clone(), justification.clone());
		let expected = millau_runtime::BridgeGrandpaRialtoCall::<millau_runtime::Runtime>::submit_finality_proof(
			header,
			justification.clone(),
		);

		// when
		let actual_encoded = actual.encode();
		let expected_encoded = expected.encode();

		// then
		assert_eq!(
			actual_encoded, expected_encoded,
			"Encoding difference. Raw: {:?} vs {:?}",
			actual, expected
		);
	}
}
