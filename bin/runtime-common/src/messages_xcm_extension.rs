// Copyright 2023 Parity Technologies (UK) Ltd.
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

//! Module provides utilities for easier XCM handling, e.g:
//! `XcmExecutor` -> `MessageSender` -> `OutboundMessageQueue`
//!                                             |
//!                                          `Relayer`
//!                                             |
//! `XcmRouter` <- `MessageDispatch` <- `InboundMessageQueue`

use bp_messages::{
	source_chain::MessagesBridge,
	target_chain::{DispatchMessage, MessageDispatch},
	LaneId, MessageNonce,
};
use bp_runtime::{messages::MessageDispatchResult, Chain};
use bp_xcm_bridge_hub::LocalXcmChannelManager;
use codec::{Decode, Encode};
use frame_support::{dispatch::Weight, CloneNoBound, EqNoBound, PartialEqNoBound};
use pallet_bridge_messages::WeightInfoExt as MessagesPalletWeights;
use scale_info::TypeInfo;
use sp_core::Get;
use sp_runtime::SaturatedConversion;
use sp_std::marker::PhantomData;
use xcm::prelude::*;
use xcm_builder::{DispatchBlob, DispatchBlobError, HaulBlob, HaulBlobError};

/// Plain "XCM" payload, which we transfer through bridge
pub type XcmAsPlainPayload = sp_std::prelude::Vec<u8>;

/// Make LaneId from chain identifiers of two bridge endpoints.
// TODO: https://github.com/paritytech/parity-bridges-common/issues/1666: this function
// is a temporary solution, because `ChainId` and will be removed soon.
pub struct LaneIdFromChainId<R, I>(PhantomData<(R, I)>);

impl<R, I> Get<LaneId> for LaneIdFromChainId<R, I>
where
	R: pallet_bridge_messages::Config<I>,
	I: 'static,
{
	fn get() -> LaneId {
		LaneId::new(
			pallet_bridge_messages::ThisChainOf::<R, I>::ID,
			pallet_bridge_messages::BridgedChainOf::<R, I>::ID,
		)
	}
}

/// Message dispatch result type for single message
#[derive(CloneNoBound, EqNoBound, PartialEqNoBound, Encode, Decode, Debug, TypeInfo)]
pub enum XcmBlobMessageDispatchResult {
	InvalidPayload,
	Dispatched,
	NotDispatched(#[codec(skip)] Option<DispatchBlobError>),
}

/// [`XcmBlobMessageDispatch`] is responsible for dispatching received messages
pub struct XcmBlobMessageDispatch<DispatchBlob, Weights> {
	_marker: sp_std::marker::PhantomData<(DispatchBlob, Weights)>,
}

impl<BlobDispatcher: DispatchBlob, Weights: MessagesPalletWeights> MessageDispatch
	for XcmBlobMessageDispatch<BlobDispatcher, Weights>
{
	type DispatchPayload = XcmAsPlainPayload;
	type DispatchLevelResult = XcmBlobMessageDispatchResult;

	fn is_active() -> bool {
		// TODO: extend blob dispatcher with some queue-related methods + emulate queue
		// at Rialto/Millau + proper implementation for HRMP/UMP queues
		true
	}

	fn dispatch_weight(message: &mut DispatchMessage<Self::DispatchPayload>) -> Weight {
		match message.data.payload {
			Ok(ref payload) => {
				let payload_size = payload.encoded_size().saturated_into();
				Weights::message_dispatch_weight(payload_size)
			},
			Err(_) => Weight::zero(),
		}
	}

	fn dispatch(
		message: DispatchMessage<Self::DispatchPayload>,
	) -> MessageDispatchResult<Self::DispatchLevelResult> {
		let payload = match message.data.payload {
			Ok(payload) => payload,
			Err(e) => {
				log::error!(
					target: crate::LOG_TARGET_BRIDGE_DISPATCH,
					"[XcmBlobMessageDispatch] payload error: {:?} - message_nonce: {:?}",
					e,
					message.key.nonce
				);
				return MessageDispatchResult {
					unspent_weight: Weight::zero(),
					dispatch_level_result: XcmBlobMessageDispatchResult::InvalidPayload,
				}
			},
		};
		let dispatch_level_result = match BlobDispatcher::dispatch_blob(payload) {
			Ok(_) => {
				log::debug!(
					target: crate::LOG_TARGET_BRIDGE_DISPATCH,
					"[XcmBlobMessageDispatch] DispatchBlob::dispatch_blob was ok - message_nonce: {:?}",
					message.key.nonce
				);
				XcmBlobMessageDispatchResult::Dispatched
			},
			Err(e) => {
				log::error!(
					target: crate::LOG_TARGET_BRIDGE_DISPATCH,
					"[XcmBlobMessageDispatch] DispatchBlob::dispatch_blob failed, error: {:?} - message_nonce: {:?}",
					e, message.key.nonce
				);
				XcmBlobMessageDispatchResult::NotDispatched(Some(e))
			},
		};
		MessageDispatchResult { unspent_weight: Weight::zero(), dispatch_level_result }
	}
}

/// [`XcmBlobHauler`] is responsible for sending messages to the bridge "point-to-point link" from
/// one side, where on the other it can be dispatched by [`XcmBlobMessageDispatch`].
pub trait XcmBlobHauler {
	/// Runtime message sender adapter.
	type MessageSender: MessagesBridge<XcmAsPlainPayload>;

	/// Returns the relative XCM location of the message sending chain.
	fn sending_chain_location() -> MultiLocation;
	/// Return message lane (as "point-to-point link") used to deliver XCM messages.
	fn xcm_lane() -> LaneId;
}

/// XCM bridge adapter which connects [`XcmBlobHauler`] with [`XcmBlobHauler::MessageSender`] and
/// makes sure that XCM blob is sent to the [`pallet_bridge_messages`] queue to be relayed.
/// When the [`XcmBlobHauler::MessageSender`] is in the overloaded state, the adapter will
/// suspend the inbound XCM channel with [`XcmBlobHauler::MessageSender::sending_chain_location`]
/// using the [`XcmChannelManager`].
pub struct XcmBlobHaulerAdapter<XcmBlobHauler, XcmChannelManager, OverloadedLimit>(
	PhantomData<(XcmBlobHauler, XcmChannelManager, OverloadedLimit)>,
);

impl<H, C, L> HaulBlob for XcmBlobHaulerAdapter<H, C, L>
where
	H: XcmBlobHauler,
	C: LocalXcmChannelManager,
	L: Get<MessageNonce>,
{
	fn haul_blob(blob: sp_std::prelude::Vec<u8>) -> Result<(), HaulBlobError> {
		let lane = H::xcm_lane();
		H::MessageSender::send_message(lane, blob)
			.map(|artifacts| {
				let msg_hash = (lane, artifacts.nonce).using_encoded(sp_io::hashing::blake2_256);
				log::info!(
					target: crate::LOG_TARGET_BRIDGE_DISPATCH,
					"haul_blob result - ok: {:?} on lane: {:?}",
					msg_hash,
					lane
				);

				// suspend the inbound XCM channel with the sender to avoid enqueueing more messages
				// at the outbound bridge queue AND turn on internal backpressure mechanism of the
				// XCM queue
				let is_overloaded = artifacts.enqueued_messages > L::get();
				if is_overloaded {
					// TODO: save the fact that one of `sending_chain_location` bridges has too many
					// enqueued messages - it'll be checked by the `LocalInboundXcmChannelSuspender`

					let sending_chain_location = H::sending_chain_location();
					let suspend_result = C::suspend_inbound_channel(sending_chain_location);
					match suspend_result {
						Ok(_) => log::info!(
							target: crate::LOG_TARGET_BRIDGE_DISPATCH,
							"Suspended inbound XCM channel with {:?} to avoid overloading lane {:?}",
							sending_chain_location,
							lane,
						),
						Err(_) => log::info!(
							target: crate::LOG_TARGET_BRIDGE_DISPATCH,
							"Failed to suspend inbound XCM channel with {:?} to avoid overloading lane {:?}",
							sending_chain_location,
							lane,
						),
					}
				}
			})
			.map_err(|error| {
				log::error!(
					target: crate::LOG_TARGET_BRIDGE_DISPATCH,
					"haul_blob result - error: {:?} on lane: {:?}",
					error,
					lane
				);
				HaulBlobError::Transport("MessageSenderError")
			})
	}
}

// TODO: use following structures in the `pallet-message-queue` configuration

// TODO: it must be a part of `pallet-xcm-bridge-hub`
pub struct LocalInboundXcmChannelSuspender<Origin, Inner>(PhantomData<(Origin, Inner)>);

impl<Origin, Inner> QueuePausedQuery<Origin> for LocalInboundXcmChannelSuspender where
	Origin: Into<MultiLocation>,
	Inner: QueuePausedQuery<Origin>,
{
	fn is_paused(origin: &Origin) -> bool {
		// give priority to inner status
		if Inner::is_paused(origin) {
			return true
		}

		// TODO: if at least one bridge, owner by the `origin` has too many messages, return true

		// TODO: where we should resume the channel? Two options: here OR from the `pallet-xcm-bridge`
		// `on_initialize` (or `on_idle`?)

		false
	}
}

pub struct BridgeMessageProcessor<Origin, Inner>(PhantomData<(Origin, Inner)>);

impl<Origin, Inner> ProcessMessage for BridgeMessageProcessor<Origin, Inner> where
	Origin: Into<MultiLocation>,
	Inner: ProcessMessage<Origin = Origin>,
{
	type Origin = Origin;

	fn process_message(
		message: &[u8],
		origin: Self::Origin,
		meter: &mut WeightMeter,
		id: &mut [u8; 32],
	) -> Result<bool, ProcessMessageError> {
		// TODO: if at least one bridge, owner by the `origin` has too many messages, return Err(ProcessMessageError::Yield)

		// else pass message to backed processor
		Inner::process_message(message, origin, meter, id)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::run_test;

	use bp_messages::source_chain::SendMessageArtifacts;
	use frame_support::traits::{ConstU64, Get};

	struct TestMessageSender;

	impl MessagesBridge<Vec<u8>> for TestMessageSender {
		type Error = ();

		fn send_message(
			_lane: LaneId,
			message: Vec<u8>,
		) -> Result<SendMessageArtifacts, Self::Error> {
			let nonce = message[0] as _;
			Ok(SendMessageArtifacts { nonce, enqueued_messages: nonce })
		}
	}

	struct TestBlobHauler;

	impl XcmBlobHauler for TestBlobHauler {
		type MessageSender = TestMessageSender;

		fn sending_chain_location() -> MultiLocation {
			Here.into()
		}

		fn xcm_lane() -> LaneId {
			LaneId::new(0, 0)
		}
	}

	struct TestXcmChannelManager;

	impl TestXcmChannelManager {
		fn is_suspended(owner: MultiLocation) -> bool {
			frame_support::storage::unhashed::take(&owner.encode()) == Some(42)
		}
	}

	impl LocalXcmChannelManager for TestXcmChannelManager {
		fn suspend_inbound_channel(owner: MultiLocation) -> Result<(), ()> {
			frame_support::storage::unhashed::put(&owner.encode(), &42);
			Ok(())
		}

		fn resume_inbound_channel(_owner: MultiLocation) -> Result<(), ()> {
			unreachable!("")
		}
	}

	type OverloadedLimit = ConstU64<8>;
	type TestAdapter = XcmBlobHaulerAdapter<TestBlobHauler, TestXcmChannelManager, OverloadedLimit>;

	#[test]
	fn blob_haulder_adapter_suspends_inbound_xcm_channel_when_bridge_is_overloaded() {
		run_test(|| {
			// first `OverloadedLimit` messages => no suspension
			for nonce in 1..=OverloadedLimit::get() {
				TestAdapter::haul_blob(vec![nonce as u8]).unwrap();
				assert!(!TestXcmChannelManager::is_suspended(
					TestBlobHauler::sending_chain_location()
				));
			}

			// next message => channel is suspended
			let overloaded_nonce = <OverloadedLimit as Get<u64>>::get() + 1;
			TestAdapter::haul_blob(vec![overloaded_nonce as u8]).unwrap();
			assert!(TestXcmChannelManager::is_suspended(TestBlobHauler::sending_chain_location()));
		});
	}
}
