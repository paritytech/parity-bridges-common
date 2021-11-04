use async_trait::async_trait;

use bp_messages::{MessageNonce, Weight};
use bp_runtime::messages::DispatchFeePayment;

use crate::{
	message_lane::MessageLane,
	message_lane_loop::{
		RelayerMode, SourceClient as MessageLaneSourceClient,
		TargetClient as MessageLaneTargetClient,
	},
	message_race_loop::NoncesRange,
	relay_strategy::{RelayReference, RelayStrategy},
};

/// Do hard check and run soft check strategy
#[derive(Clone)]
pub struct EnforcementStrategy<Strategy: RelayStrategy> {
	strategy: Strategy,
}

impl<Strategy: RelayStrategy> EnforcementStrategy<Strategy> {
	pub fn new(strategy: Strategy) -> Self {
		Self { strategy }
	}
}

#[async_trait]
impl<Strategy: RelayStrategy> RelayStrategy for EnforcementStrategy<Strategy> {
	async fn decide<
		P: MessageLane,
		SourceClient: MessageLaneSourceClient<P>,
		TargetClient: MessageLaneTargetClient<P>,
	>(
		&self,
		reference: RelayReference<P, SourceClient, TargetClient>,
	) -> Option<MessageNonce> {
		let mut hard_selected_count = 0u64;

		let mut selected_weight: Weight = 0;
		let mut selected_size: u32 = 0;
		let mut selected_count: MessageNonce = 0;

		let hard_selected_begin_nonce =
			reference.nonces_queue[reference.nonces_queue_range.start].1.begin();

		let all_ready_nonces = reference
			.nonces_queue
			.range(reference.nonces_queue_range.clone())
			.flat_map(|(_, ready_nonces)| ready_nonces.iter())
			.enumerate();
		for (index, (_nonce, details)) in all_ready_nonces {
			// Since we (hopefully) have some reserves in `max_messages_weight_in_single_batch`
			// and `max_messages_size_in_single_batch`, we may still try to submit transaction
			// with single message if message overflows these limits. The worst case would be if
			// transaction will be rejected by the target runtime, but at least we have tried.

			// limit messages in the batch by weight
			let new_selected_weight = match selected_weight.checked_add(details.dispatch_weight) {
				Some(new_selected_weight)
					if new_selected_weight <= reference.max_messages_weight_in_single_batch =>
					new_selected_weight,
				new_selected_weight if selected_count == 0 => {
					log::warn!(
						target: "bridge",
						"Going to submit message delivery transaction with declared dispatch \
						weight {:?} that overflows maximal configured weight {}",
						new_selected_weight,
						reference.max_messages_weight_in_single_batch,
					);
					new_selected_weight.unwrap_or(Weight::MAX)
				},
				_ => break,
			};

			// limit messages in the batch by size
			let new_selected_size = match selected_size.checked_add(details.size) {
				Some(new_selected_size)
					if new_selected_size <= reference.max_messages_size_in_single_batch =>
					new_selected_size,
				new_selected_size if selected_count == 0 => {
					log::warn!(
						target: "bridge",
						"Going to submit message delivery transaction with message \
						size {:?} that overflows maximal configured size {}",
						new_selected_size,
						reference.max_messages_size_in_single_batch,
					);
					new_selected_size.unwrap_or(u32::MAX)
				},
				_ => break,
			};

			// limit number of messages in the batch
			let new_selected_count = selected_count + 1;
			if new_selected_count > reference.max_messages_in_this_batch {
				break
			}

			hard_selected_count = index + 1;
			selected_weight = new_selected_weight;
			selected_size = new_selected_size;
			selected_count = new_selected_count;
		}

		// soft check

		let soft_selected_count = self.strategy.decide(reference).await.unwrap_or(0);

		if hard_selected_count != soft_selected_count {
			let hard_selected_end_nonce =
				hard_selected_begin_nonce + hard_selected_count as MessageNonce - 1;
			let soft_selected_begin_nonce = hard_selected_begin_nonce;
			let soft_selected_end_nonce =
				soft_selected_begin_nonce + soft_selected_count as MessageNonce - 1;
			log::warn!(
				target: "bridge",
				"Relayer may deliver nonces [{:?}; {:?}], but because of its strategy it has selected \
				nonces [{:?}; {:?}].",
				hard_selected_begin_nonce,
				hard_selected_end_nonce,
				soft_selected_begin_nonce,
				soft_selected_end_nonce,
			);
			hard_selected_count = soft_selected_count;
		}

		if hard_selected_count == 0 {
			if soft_selected_count != 0 {
				log::warn!(target: "bridge", "Failed hard check");
			}
			return None
		}

		log::trace!(
			target: "bridge",
			"Delivering nonces [{:?}; {:?}]",
			hard_selected_begin_nonce,
			hard_selected_begin_nonce + hard_selected_count as MessageNonce - 1,
		);

		Some(hard_selected_begin_nonce + hard_selected_count as MessageNonce - 1)
	}
}
