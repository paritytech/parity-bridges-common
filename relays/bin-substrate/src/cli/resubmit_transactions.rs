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

use crate::cli::{TargetConnectionParams, TargetSigningParams};

use codec::{Decode, Encode};
use relay_substrate_client::{Chain, Client, Error as SubstrateError, TransactionSignScheme};
use relay_utils::FailedClient;
use sp_core::Bytes;
use sp_runtime::{traits::{Hash, Header as HeaderT}, transaction_validity::TransactionPriority};
use std::time::Duration;
use structopt::StructOpt;
use strum::{EnumString, EnumVariantNames, VariantNames};

/// Start resubmit transactions process.
#[derive(StructOpt)]
pub struct ResubmitTransactions {
	/// A bridge instance to relay headers for.
	#[structopt(possible_values = RelayChain::VARIANTS, case_insensitive = true)]
	chain: RelayChain,
	#[structopt(flatten)]
	target: TargetConnectionParams,
	#[structopt(flatten)]
	target_sign: TargetSigningParams,
}

/// Chain, which transactions we're going to track && resubmit.
#[derive(Debug, EnumString, EnumVariantNames)]
#[strum(serialize_all = "kebab_case")]
pub enum RelayChain {
	Millau,
}

macro_rules! select_bridge {
	($bridge: expr, $generic: tt) => {
		match $bridge {
			RelayChain::Millau => {
				type Target = relay_millau_client::Millau;
				type TargetSign = relay_millau_client::Millau;

				$generic
			}
		}
	};
}

#[derive(Debug, Default)]
struct Context<C: Chain> {
	/// Hash of the (potentially) stalled transaction.
	pub transaction: Option<C::Hash>,
	/// This transaction is in pool for `stalled_for` wakeup intevals.
	pub stalled_for: C::BlockNumber,
	/// Tip step interval.
	pub tip_step: C::Balance,
	/// Maximal tip.
	pub tip_limit: C::Balance,
}

impl ResubmitTransactions {
	/// Run the command.
	pub async fn run(self) -> anyhow::Result<()> {
		select_bridge!(self.chain, {
			let relay_loop_name = format!("ResubmitTransactions{}", Target::NAME);
			let client = self.target.to_client::<Target>().await?;
			let key_pair = self.target_sign.to_keypair::<Target>()?;

			relay_utils::relay_loop((), client)
				.run(relay_loop_name, |_, target_client, _| async {
					let mut context = Context::<Target>::default();
					loop {
						async_std::task::sleep(Duration::from_secs(Target::AVERAGE_BLOCK_INTERVAL));

						let result = run_loop_iteration::<Target, TargetSign>(&client, &key_pair, context).await;
						context = match result {
							Ok(context) => context,
							Err(error) => {
								// TODO: trace
								return Err(FailedClient::Target);
							},
						};
					}
				})
				.await
		})
	}
}

/// Run single loop iteration.
async fn run_loop_iteration<C: Chain, S: TransactionSignScheme<Chain = C>>(
	client: &Client<C>,
	key_pair: &S::AccountKeyPair,
	context: Context<C>,
) -> Result<Context<C>, SubstrateError> {
	let original_transaction = match lookup_signer_transaction::<C, S>(client, key_pair).await? {
		Some(original_transaction) => original_transaction,
		None => return Ok(context),
	};
	let original_transaction_hash = C::Hasher::hash(&original_transaction.encode());
	let context = context.notice_transaction(original_transaction_hash);

	if !context.is_stalled() {
		return Ok(context);
	}

	let (best_block, target_priority) = match read_previous_best_priority::<C>(client).await? {
		Some((best_block, target_priority)) => (best_block, target_priority),
		None => return Ok(context),
	};

	let updated_transaction = select_transaction_tip::<C, S>(
		client,
		key_pair,
		best_block,
		original_transaction,
		context.tip_step(),
		context.tip_limit(),
		target_priority,
	).await?;

	let updated_transaction_hash = C::Hasher::hash(&original_transaction.encode());
	if original_transaction_hash == updated_transaction_hash {
		return Ok(context);
	}

	client.submit_unsigned_extrinsic(Bytes(updated_transaction.encode())).await?;

	Ok(context.clear())
}

/// Search transaction pool for transaction, signed by given key pair.
async fn lookup_signer_transaction<C: Chain, S: TransactionSignScheme<Chain = C>>(
	client: &Client<C>,
	key_pair: &S::AccountKeyPair,
) -> Result<Option<S::SignedTransaction>, SubstrateError> {
	let pending_transactions = client.pending_extrinsics().await?;
	for pending_transaction in pending_transactions {
		let pending_transaction = S::SignedTransaction::decode(&mut &pending_transaction.0[..])
			.map_err(SubstrateError::ResponseParseFailed)?;
		if !S::is_signed_by(key_pair, &pending_transaction) {
			continue;
		}

		return Ok(Some(pending_transaction));
	}

	Ok(None)
}

/// Read priority of best signed transaction of previous block.
async fn read_previous_best_priority<C: Chain>(
	client: &Client<C>,
) -> Result<Option<(C::Hash, TransactionPriority)>, SubstrateError> {
	let best_header = client.best_header().await?;
	let best_header_hash = best_header.hash();
	let best_block = client.get_block(Some(best_header_hash)).await?;
	let best_transaction = best_block.block.extrinsics.iter()
		.filter(|xt| xt.signature.is_some())
		.next()
		.cloned();
	match best_transaction {
		Some(best_transaction) => Ok(Some((best_header_hash, client.validate_transaction(
			*best_header.parent_hash(),
			best_transaction,
		).await??.priority))),
		None => Ok(None),
	}
}

/// Try to find appropriate tip for transaction so that its priority is larger than given.
async fn select_transaction_tip<C: Chain, S: TransactionSignScheme<Chain = C>>(
	client: &Client<C>,
	key_pair: &S::AccountKeyPair,
	at_block: C::Hash,
	tx: S::SignedTransaction,
	tip_step: C::Balance,
	tip_limit: C::Balance,
	target_priority: TransactionPriority,
) -> Result<S::SignedTransaction, SubstrateError> {
	let stx = format!("{:?}", tx);
	let mut current_priority = client.validate_transaction(at_block, tx.clone()).await??.priority;
	let mut unsigned_tx = S::parse_transaction(tx).ok_or_else(|| SubstrateError::Custom(format!(
		"Failed to parse {} transaction {}", C::NAME, stx,
	)))?;

	while current_priority < target_priority {
		let next_tip = unsigned_tx.tip + tip_step;
		if next_tip > tip_limit {
			break;
		}

		unsigned_tx.tip = next_tip;
		current_priority = client.validate_transaction(
			at_block,
			S::sign_transaction(*client.genesis_hash(), key_pair, unsigned_tx.clone()),
		).await??.priority;
	}

	Ok(S::sign_transaction(*client.genesis_hash(), key_pair, unsigned_tx))
}
