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

//! Helper for tracking transaction invalidation events.

use crate::{Chain, HashOf, Subscription, TransactionStatusOf};

use async_trait::async_trait;
use futures::{Stream, StreamExt};
use relay_utils::TrackedTransactionStatus;
use std::time::Duration;

/// Substrate transaction tracker implementation.
///
/// Substrate node provides RPC API to submit and watch for transaction events. This way
/// we may know when transaction is included into block, finalized or rejected. There are
/// some edge cases, when we can't fully trust this mechanism - e.g. transaction may broadcasted
/// and then dropped out of node transaction pool (some other cases are also possible - node
/// restarts, connection lost, ...). Then we can't know for sure - what is currently happening
/// with our transaction. Is the transaction really lost? Is it still alive on the chain network?
///
/// We have several options to handle such cases:
///
/// 1) hope that the transaction is still alive and wait for its mining until it is spoiled;
///
/// 2) assume that the transaction is lost and resubmit another transaction instantly;
///
/// 3) wait for some time (if transaction is mortal - then until block where it dies; if it is
///    immortal - then for some time that we assume is long enough to mine it) and assume that
///    it is lost.
///
/// This struct implements third option as it seems to be the most optimal.
pub struct TransactionTracker<C: Chain> {
	transaction_hash: HashOf<C>,
	stall_timeout: Duration,
	subscription: Subscription<TransactionStatusOf<C>>,
}

impl<C: Chain> TransactionTracker<C> {
	/// Create transaction tracker.
	pub fn new(
		stall_timeout: Duration,
		transaction_hash: HashOf<C>,
		subscription: Subscription<TransactionStatusOf<C>>,
	) -> Self {
		Self { stall_timeout, transaction_hash, subscription }
	}
}

#[async_trait]
impl<C: Chain> relay_utils::TransactionTracker for TransactionTracker<C> {
	async fn wait(self) -> TrackedTransactionStatus {
		let invalidation_status = watch_transaction_status::<C, _>(
			self.transaction_hash,
			self.subscription.into_stream(),
		)
		.await;
		match invalidation_status {
			InvalidationStatus::Finalized => TrackedTransactionStatus::Finalized,
			InvalidationStatus::Invalid => TrackedTransactionStatus::Lost,
			InvalidationStatus::Lost => {
				async_std::task::sleep(self.stall_timeout).await;
				// if someone is still watching for our transaction, then we're reporting
				// an error here (which is treated as "transaction lost")
				log::trace!(
					target: "bridge",
					"{} transaction {:?} is considered lost after timeout",
					C::NAME,
					self.transaction_hash,
				);

				TrackedTransactionStatus::Lost
			},
		}
	}
}

/// Transaction invalidation status.
///
/// Note that in places where the `TransactionTracker` is used, the finalization event will be
/// ignored - relay loops are detecting the mining/finalization using their own
/// techniques. That's why we're using `InvalidationStatus` here.
#[derive(Debug, PartialEq)]
enum InvalidationStatus {
	/// Transaction has been included into block and finalized.
	Finalized,
	/// Transaction has been invalidated.
	Invalid,
	/// We have lost track of transaction status.
	Lost,
}

/// Watch for transaction status until transaction is finalized or we lose track of its status.
async fn watch_transaction_status<C: Chain, S: Stream<Item = TransactionStatusOf<C>>>(
	transaction_hash: HashOf<C>,
	subscription: S,
) -> InvalidationStatus {
	futures::pin_mut!(subscription);

	loop {
		match subscription.next().await {
			Some(TransactionStatusOf::<C>::Finalized(block_hash)) => {
				// the only "successful" outcome of this method is when the block with transaction
				// has been finalized
				log::trace!(
					target: "bridge",
					"{} transaction {:?} has been finalized at block: {:?}",
					C::NAME,
					transaction_hash,
					block_hash,
				);
				return InvalidationStatus::Finalized
			},
			Some(TransactionStatusOf::<C>::Invalid) => {
				// if node says that the transaction is invalid, there are still chances that
				// it is not actually invalid - e.g. if the block where transaction has been
				// revalidated is retracted and transaction (at some other node pool) becomes
				// valid again on other fork. But let's assume that the chances of this event
				// are almost zero - there's a lot of things that must happen for this to be the
				// case.
				log::trace!(
					target: "bridge",
					"{} transaction {:?} has been invalidated",
					C::NAME,
					transaction_hash,
				);
				return InvalidationStatus::Invalid
			},
			Some(TransactionStatusOf::<C>::Future) |
			Some(TransactionStatusOf::<C>::Ready) |
			Some(TransactionStatusOf::<C>::Broadcast(_)) => {
				// nothing important (for us) has happened
			},
			Some(TransactionStatusOf::<C>::InBlock(block_hash)) => {
				// TODO: read matching system event (ExtrinsicSuccess or ExtrinsicFailed), log it
				// here and use it later (on finality) for reporting invalid transaction
				// https://github.com/paritytech/parity-bridges-common/issues/1464
				log::trace!(
					target: "bridge",
					"{} transaction {:?} has been included in block: {:?}",
					C::NAME,
					transaction_hash,
					block_hash,
				);
			},
			Some(TransactionStatusOf::<C>::Retracted(block_hash)) => {
				log::trace!(
					target: "bridge",
					"{} transaction {:?} at block {:?} has been retracted",
					C::NAME,
					transaction_hash,
					block_hash,
				);
			},
			Some(TransactionStatusOf::<C>::FinalityTimeout(block_hash)) => {
				// finality is lagging? let's wait a bit more and report a stall
				log::trace!(
					target: "bridge",
					"{} transaction {:?} block {:?} has not been finalized for too long",
					C::NAME,
					transaction_hash,
					block_hash,
				);
				return InvalidationStatus::Lost
			},
			Some(TransactionStatusOf::<C>::Usurped(new_transaction_hash)) => {
				// this may be result of our transaction resubmitter work or some manual
				// intervention. In both cases - let's start stall timeout, because the meaning
				// of transaction may have changed
				log::trace!(
					target: "bridge",
					"{} transaction {:?} has been usurped by new transaction: {:?}",
					C::NAME,
					transaction_hash,
					new_transaction_hash,
				);
				return InvalidationStatus::Lost
			},
			Some(TransactionStatusOf::<C>::Dropped) => {
				// the transaction has been removed from the pool because of its limits. Let's wait
				// a bit and report a stall
				log::trace!(
					target: "bridge",
					"{} transaction {:?} has been dropped from the pool",
					C::NAME,
					transaction_hash,
				);
				return InvalidationStatus::Lost
			},
			None => {
				// the status of transaction is unknown to us (the subscription has been closed?).
				// Let's wait a bit and report a stall
				return InvalidationStatus::Lost
			},
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::test_chain::TestChain;
	use futures::{FutureExt, SinkExt};
	use relay_utils::TransactionTracker as _;
	use sc_transaction_pool_api::TransactionStatus;

	async fn on_transaction_status(
		status: TransactionStatus<HashOf<TestChain>, HashOf<TestChain>>,
	) -> Option<TrackedTransactionStatus> {
		let (mut sender, receiver) = futures::channel::mpsc::channel(1);
		let tx_tracker = TransactionTracker::<TestChain>::new(
			Duration::from_secs(0),
			Default::default(),
			Subscription(async_std::sync::Mutex::new(receiver)),
		);

		sender.send(Some(status)).await.unwrap();
		tx_tracker.wait().now_or_never()
	}

	#[async_std::test]
	async fn returns_finalized_on_finalized() {
		assert_eq!(
			on_transaction_status(TransactionStatus::Finalized(Default::default())).await,
			Some(TrackedTransactionStatus::Finalized),
		);
	}

	#[async_std::test]
	async fn returns_invalid_on_invalid() {
		assert_eq!(
			on_transaction_status(TransactionStatus::Invalid).await,
			Some(TrackedTransactionStatus::Lost),
		);
	}

	#[async_std::test]
	async fn waits_on_future() {
		assert_eq!(on_transaction_status(TransactionStatus::Future).await, None,);
	}

	#[async_std::test]
	async fn waits_on_ready() {
		assert_eq!(on_transaction_status(TransactionStatus::Ready).await, None,);
	}

	#[async_std::test]
	async fn waits_on_broadcast() {
		assert_eq!(
			on_transaction_status(TransactionStatus::Broadcast(Default::default())).await,
			None,
		);
	}

	#[async_std::test]
	async fn waits_on_in_block() {
		assert_eq!(
			on_transaction_status(TransactionStatus::InBlock(Default::default())).await,
			None,
		);
	}

	#[async_std::test]
	async fn waits_on_retracted() {
		assert_eq!(
			on_transaction_status(TransactionStatus::Retracted(Default::default())).await,
			None,
		);
	}

	#[async_std::test]
	async fn lost_on_finality_timeout() {
		assert_eq!(
			on_transaction_status(TransactionStatus::FinalityTimeout(Default::default())).await,
			Some(TrackedTransactionStatus::Lost),
		);
	}

	#[async_std::test]
	async fn lost_on_usurped() {
		assert_eq!(
			on_transaction_status(TransactionStatus::Usurped(Default::default())).await,
			Some(TrackedTransactionStatus::Lost),
		);
	}

	#[async_std::test]
	async fn lost_on_dropped() {
		assert_eq!(
			on_transaction_status(TransactionStatus::Dropped).await,
			Some(TrackedTransactionStatus::Lost),
		);
	}

	#[async_std::test]
	async fn lost_on_subscription_error() {
		assert_eq!(
			watch_transaction_status::<TestChain, _>(Default::default(), futures::stream::iter([]))
				.now_or_never(),
			Some(InvalidationStatus::Lost),
		);
	}
}
