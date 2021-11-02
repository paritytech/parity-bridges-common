use crate::{message_lane::MessageLane, message_lane_loop::RelayerMode};

use crate::message_lane_loop::{
	SourceClient as MessageLaneSourceClient, TargetClient as MessageLaneTargetClient,
};

pub trait RelayerStrategy {
	fn decide<
		P: MessageLane,
		SourceClient: MessageLaneSourceClient<P>,
		TargetClient: MessageLaneTargetClient<P>,
	>(
		relayer_mode: RelayerReference<P, SourceClient, TargetClient>,
	) -> Option<RelayerDecide<P>>;
}

pub struct RelayerReference<
	P: MessageLane,
	SourceClient: MessageLaneSourceClient<P>,
	TargetClient: MessageLaneTargetClient<P>,
> {
	pub relayer_mode: RelayerMode,
	pub lane_source_client: SourceClient,
	pub lane_target_client: TargetClient,
}

pub struct RelayerDecide<P: MessageLane> {
	participate: bool,
	total_reward: Option<P::SourceChainBalance>,
	total_cost: Option<P::SourceChainBalance>,
}
