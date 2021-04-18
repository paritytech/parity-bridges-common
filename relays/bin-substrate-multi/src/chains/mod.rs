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

//! Chain-specific relayer configuration.

pub mod circuit_headers_to_gateway;
pub mod circuit_messages_to_gateway;
pub mod gateway_headers_to_circuit;
pub mod gateway_messages_to_circuit;

mod circuit;
mod gateway;

use relay_utils::metrics::{FloatJsonValueMetric, MetricsParams};

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

#[cfg(test)]
mod tests {
	use crate::cli::{encode_call, send_message};
	use bp_messages::source_chain::TargetHeaderChain;
	use codec::Encode;
	use frame_support::dispatch::GetDispatchInfo;
	use relay_circuit_client::Circuit;
	use relay_gateway_client::Gateway;
	use relay_substrate_client::TransactionSignScheme;
	use sp_core::Pair;
	use sp_runtime::traits::{IdentifyAccount, Verify};

	#[test]
	fn circuit_signature_is_valid_on_gateway() {
		let circuit_sign = relay_circuit_client::SigningParams::from_string("//Dave", None).unwrap();

		let call = gateway_runtime::Call::System(gateway_runtime::SystemCall::remark(vec![]));

		let circuit_public: bp_circuit::AccountSigner = circuit_sign.public().into();
		let circuit_account_id: bp_circuit::AccountId = circuit_public.into_account();

		let digest = circuit_runtime::gateway_account_ownership_digest(
			&call,
			circuit_account_id,
			gateway_runtime::VERSION.spec_version,
		);

		let gateway_signer = relay_gateway_client::SigningParams::from_string("//Dave", None).unwrap();
		let signature = gateway_signer.sign(&digest);

		assert!(signature.verify(&digest[..], &gateway_signer.public()));
	}

	#[test]
	fn gateway_signature_is_valid_on_circuit() {
		let gateway_sign = relay_gateway_client::SigningParams::from_string("//Dave", None).unwrap();

		let call = circuit_runtime::Call::System(circuit_runtime::SystemCall::remark(vec![]));

		let gateway_public: bp_gateway::AccountSigner = gateway_sign.public().into();
		let gateway_account_id: bp_gateway::AccountId = gateway_public.into_account();

		let digest = gateway_runtime::circuit_account_ownership_digest(
			&call,
			gateway_account_id,
			circuit_runtime::VERSION.spec_version,
		);

		let circuit_signer = relay_circuit_client::SigningParams::from_string("//Dave", None).unwrap();
		let signature = circuit_signer.sign(&digest);

		assert!(signature.verify(&digest[..], &circuit_signer.public()));
	}

	#[test]
	fn maximal_gateway_to_circuit_message_arguments_size_is_computed_correctly() {
		use gateway_runtime::circuit_messages::Circuit;

		let maximal_remark_size = encode_call::compute_maximal_message_arguments_size(
			bp_gateway::max_extrinsic_size(),
			bp_circuit::max_extrinsic_size(),
		);

		let call: circuit_runtime::Call =
			circuit_runtime::SystemCall::remark(vec![42; maximal_remark_size as _]).into();
		let payload = send_message::message_payload(
			Default::default(),
			call.get_dispatch_info().weight,
			pallet_bridge_dispatch::CallOrigin::SourceRoot,
			&call,
		);
		assert_eq!(Circuit::verify_message(&payload), Ok(()));

		let call: circuit_runtime::Call =
			circuit_runtime::SystemCall::remark(vec![42; (maximal_remark_size + 1) as _]).into();
		let payload = send_message::message_payload(
			Default::default(),
			call.get_dispatch_info().weight,
			pallet_bridge_dispatch::CallOrigin::SourceRoot,
			&call,
		);
		assert!(Circuit::verify_message(&payload).is_err());
	}

	#[test]
	fn maximal_size_remark_to_gateway_is_generated_correctly() {
		assert!(
			bridge_runtime_common::messages::target::maximal_incoming_message_size(bp_gateway::max_extrinsic_size())
				> bp_circuit::max_extrinsic_size(),
			"We can't actually send maximal messages to Gateway from Circuit, because Circuit extrinsics can't be that large",
		)
	}

	#[test]
	fn maximal_gateway_to_circuit_message_dispatch_weight_is_computed_correctly() {
		use gateway_runtime::circuit_messages::Circuit;

		let maximal_dispatch_weight =
			send_message::compute_maximal_message_dispatch_weight(bp_circuit::max_extrinsic_weight());
		let call: circuit_runtime::Call = gateway_runtime::SystemCall::remark(vec![]).into();

		let payload = send_message::message_payload(
			Default::default(),
			maximal_dispatch_weight,
			pallet_bridge_dispatch::CallOrigin::SourceRoot,
			&call,
		);
		assert_eq!(Circuit::verify_message(&payload), Ok(()));

		let payload = send_message::message_payload(
			Default::default(),
			maximal_dispatch_weight + 1,
			pallet_bridge_dispatch::CallOrigin::SourceRoot,
			&call,
		);
		assert!(Circuit::verify_message(&payload).is_err());
	}

	#[test]
	fn maximal_weight_fill_block_to_gateway_is_generated_correctly() {
		use circuit_runtime::gateway_messages::Gateway;

		let maximal_dispatch_weight =
			send_message::compute_maximal_message_dispatch_weight(bp_gateway::max_extrinsic_weight());
		let call: gateway_runtime::Call = circuit_runtime::SystemCall::remark(vec![]).into();

		let payload = send_message::message_payload(
			Default::default(),
			maximal_dispatch_weight,
			pallet_bridge_dispatch::CallOrigin::SourceRoot,
			&call,
		);
		assert_eq!(Gateway::verify_message(&payload), Ok(()));

		let payload = send_message::message_payload(
			Default::default(),
			maximal_dispatch_weight + 1,
			pallet_bridge_dispatch::CallOrigin::SourceRoot,
			&call,
		);
		assert!(Gateway::verify_message(&payload).is_err());
	}

	#[test]
	fn gateway_tx_extra_bytes_constant_is_correct() {
		let gateway_call = gateway_runtime::Call::System(gateway_runtime::SystemCall::remark(vec![]));
		let gateway_tx = Gateway::sign_transaction(
			Default::default(),
			&sp_keyring::AccountKeyring::Alice.pair(),
			0,
			gateway_call.clone(),
		);
		let extra_bytes_in_transaction = gateway_tx.encode().len() - gateway_call.encode().len();
		assert!(
			bp_gateway::TX_EXTRA_BYTES as usize >= extra_bytes_in_transaction,
			"Hardcoded number of extra bytes in Gateway transaction {} is lower than actual value: {}",
			bp_gateway::TX_EXTRA_BYTES,
			extra_bytes_in_transaction,
		);
	}

	#[test]
	fn circuit_tx_extra_bytes_constant_is_correct() {
		let circuit_call = circuit_runtime::Call::System(circuit_runtime::SystemCall::remark(vec![]));
		let circuit_tx = Circuit::sign_transaction(
			Default::default(),
			&sp_keyring::AccountKeyring::Alice.pair(),
			0,
			circuit_call.clone(),
		);
		let extra_bytes_in_transaction = circuit_tx.encode().len() - circuit_call.encode().len();
		assert!(
			bp_circuit::TX_EXTRA_BYTES as usize >= extra_bytes_in_transaction,
			"Hardcoded number of extra bytes in Circuit transaction {} is lower than actual value: {}",
			bp_circuit::TX_EXTRA_BYTES,
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
		let expected = circuit_runtime::BridgeGrandpaGatewayCall::<circuit_runtime::Runtime>::submit_finality_proof(
			header,
			justification,
		);

		// when
		let actual_encoded = actual.encode();
		let expected_encoded = expected.encode();

		// then
		assert_eq!(
			actual_encoded, expected_encoded,
			"\n\nEncoding difference.\nGot {:#?} \nExpected: {:#?}",
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
		let expected = circuit_runtime::BridgeGrandpaGatewayCall::<circuit_runtime::Runtime>::submit_finality_proof(
			header,
			justification,
		);

		// when
		let actual_encoded = actual.encode();
		let expected_encoded = expected.encode();

		// then
		assert_eq!(
			actual_encoded, expected_encoded,
			"\n\nEncoding difference.\nGot {:#?} \nExpected: {:#?}",
			actual, expected
		);
	}
}
