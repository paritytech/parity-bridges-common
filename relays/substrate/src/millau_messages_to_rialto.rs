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

//! Millau-to-Rialto messages sync entrypoint.

use crate::messages_source::{SubstrateMessagesSource, SubstrateTransactionMaker as SubstrateSourceTransactionMaker};
use crate::messages_target::{SubstrateMessagesTarget, SubstrateTransactionMaker as SubstrateTargetTransactionMaker};
use crate::{MillauClient, RialtoClient};

use async_trait::async_trait;
use bp_message_lane::{LaneId, MessageNonce};
use bp_runtime::{MILLAU_BRIDGE_INSTANCE, RIALTO_BRIDGE_INSTANCE};
use messages_relay::message_lane::MessageLane;
use relay_millau_client::{HeaderId as MillauHeaderId, Millau, SigningParams as MillauSigningParams};
use relay_rialto_client::{HeaderId as RialtoHeaderId, Rialto, SigningParams as RialtoSigningParams};
use relay_substrate_client::{BlockNumberOf, Error as SubstrateError, HashOf, TransactionSignScheme};
use relay_utils::metrics::MetricsParams;
use sp_core::Bytes;
use std::{ops::RangeInclusive, time::Duration};

/// Millau -> Rialto messages proof.
type FromMillauMessagesProof = (HashOf<Millau>, LaneId, MessageNonce, MessageNonce, Bytes);
/// Rialto -> Millau messages receiving proof.
type FromRialtoMessagesReceivingProof = (HashOf<Rialto>, LaneId, Bytes);

/// Millau-to-Rialto messages pipeline.
#[derive(Debug, Clone, Copy)]
struct MillauMessagesToRialto;

impl MessageLane for MillauMessagesToRialto {
	const SOURCE_NAME: &'static str = "Millau";
	const TARGET_NAME: &'static str = "Rialto";

	type MessageNonce = MessageNonce;
	type MessagesProof = FromMillauMessagesProof;
	type MessagesReceivingProof = FromRialtoMessagesReceivingProof;

	type SourceHeaderNumber = BlockNumberOf<Millau>;
	type SourceHeaderHash = HashOf<Millau>;

	type TargetHeaderNumber = BlockNumberOf<Rialto>;
	type TargetHeaderHash = HashOf<Rialto>;
}

/// Millau node as messages source.
type MillauSourceClient = SubstrateMessagesSource<Millau, MillauMessagesToRialto, MillauTransactionMaker>;

/// Millau transaction maker.
#[derive(Clone)]
struct MillauTransactionMaker {
	client: MillauClient,
	sign: MillauSigningParams,
}

#[async_trait]
impl SubstrateSourceTransactionMaker<Millau, MillauMessagesToRialto> for MillauTransactionMaker {
	type SignedTransaction = <Millau as TransactionSignScheme>::SignedTransaction;

	async fn make_messages_receiving_proof_transaction(
		&self,
		_generated_at_block: RialtoHeaderId,
		_proof: FromRialtoMessagesReceivingProof,
	) -> Result<Self::SignedTransaction, SubstrateError> {
		// TODO:
		// let account_id = self.sign.signer.public().as_array_ref().clone().into();
		// let nonce = self.client.next_account_index(account_id).await?;
		// let call = MessageLaneCall::receive_messages_delivery_proof(proof, dispatch_weight).into();
		// let transaction = Rialto::sign_transaction(&self.client, &self.sign.signer, nonce, call);
		// Ok(transaction)
		unimplemented!("TODO")
	}
}

/// Rialto node as messages target.
type RialtoTargetClient = SubstrateMessagesTarget<Rialto, MillauMessagesToRialto, RialtoTransactionMaker>;

/// Rialto transaction maker.
#[derive(Clone)]
struct RialtoTransactionMaker {
	client: RialtoClient,
	sign: RialtoSigningParams,
}

#[async_trait]
impl SubstrateTargetTransactionMaker<Rialto, MillauMessagesToRialto> for RialtoTransactionMaker {
	type SignedTransaction = <Millau as TransactionSignScheme>::SignedTransaction;

	async fn make_messages_delivery_transaction(
		&self,
		_generated_at_header: MillauHeaderId,
		_nonces: RangeInclusive<MessageNonce>,
		_proof: FromMillauMessagesProof,
	) -> Result<Self::SignedTransaction, SubstrateError> {
		// TODO:
		// let account_id = self.sign.signer.public().as_array_ref().clone().into();
		// let nonce = self.client.next_account_index(account_id).await?;
		// let dispatch_weight = unimplemented!("TODO");
		// let call = MessageLaneCall::receive_messages_proof(proof, dispatch_weight).into();
		// let transaction = Rialto::sign_transaction(&self.client, &self.sign.signer, nonce, call);
		// Ok(transaction)
		unimplemented!("TODO")
	}
}

/// Run Millau-to-Rialto messages sync.
pub fn run(
	millau_client: MillauClient,
	millau_sign: MillauSigningParams,
	rialto_client: RialtoClient,
	rialto_sign: RialtoSigningParams,
	lane: LaneId,
	metrics_params: Option<MetricsParams>,
) {
	let millau_tick = Duration::from_secs(5);
	let rialto_tick = Duration::from_secs(5);
	let reconnect_delay = Duration::from_secs(10);
	let stall_timeout = Duration::from_secs(5 * 60);

	messages_relay::message_lane_loop::run(
		lane,
		MillauSourceClient::new(
			millau_client.clone(),
			MillauTransactionMaker {
				client: millau_client,
				sign: millau_sign,
			},
			lane,
			RIALTO_BRIDGE_INSTANCE,
		),
		millau_tick,
		RialtoTargetClient::new(
			rialto_client.clone(),
			RialtoTransactionMaker {
				client: rialto_client,
				sign: rialto_sign,
			},
			lane,
			MILLAU_BRIDGE_INSTANCE,
		),
		rialto_tick,
		reconnect_delay,
		stall_timeout,
		metrics_params,
		futures::future::pending(),
	);
}
