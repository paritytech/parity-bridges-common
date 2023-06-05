use crate::{
	outbound_lane,
	outbound_lane::ReceivalConfirmationError,
	send_message,
	tests::mock::{self, *},
	weights_ext::WeightInfoExt,
	Call, Config, Error, Event, InboundLanes, MaybeOutboundLanesCount, OutboundLanes,
	OutboundMessages, Pallet, PalletOperatingMode, PalletOwner, RuntimeInboundLaneStorage,
	StoredInboundLaneData,
};

use bp_messages::{
	BridgeMessagesCall, DeliveredMessages, InboundLaneData, InboundMessageDetails, MessageKey,
	MessageNonce, MessagesOperatingMode, OutboundLaneData, OutboundMessageDetails,
	UnrewardedRelayer, UnrewardedRelayersState, VerificationError,
};
use bp_runtime::{BasicOperatingMode, PreComputedSize, Size};
use bp_test_utils::generate_owned_bridge_module_tests;
use codec::Encode;
use frame_support::{
	assert_noop, assert_ok,
	dispatch::Pays,
	storage::generator::{StorageMap, StorageValue},
	traits::Hooks,
	weights::Weight,
};
use frame_system::{EventRecord, Pallet as System, Phase};
use sp_core::Get;
use sp_runtime::DispatchError;

fn get_ready_for_events() {
	System::<TestRuntime>::set_block_number(1);
	System::<TestRuntime>::reset_events();
}

fn send_regular_message() {
	get_ready_for_events();

	let message_nonce =
		outbound_lane::<TestRuntime, ()>(TEST_LANE_ID).data().latest_generated_nonce + 1;
	send_message::<TestRuntime, ()>(TEST_LANE_ID, REGULAR_PAYLOAD)
		.expect("send_message has failed");

	// check event with assigned nonce
	assert_eq!(
		System::<TestRuntime>::events(),
		vec![EventRecord {
			phase: Phase::Initialization,
			event: TestEvent::Messages(Event::MessageAccepted {
				lane_id: TEST_LANE_ID,
				nonce: message_nonce
			}),
			topics: vec![],
		}],
	);
}

fn receive_messages_delivery_proof() {
	System::<TestRuntime>::set_block_number(1);
	System::<TestRuntime>::reset_events();

	assert_ok!(Pallet::<TestRuntime>::receive_messages_delivery_proof(
		RuntimeOrigin::signed(1),
		TestMessagesDeliveryProof(Ok((
			TEST_LANE_ID,
			InboundLaneData {
				last_confirmed_nonce: 1,
				relayers: vec![UnrewardedRelayer {
					relayer: 0,
					messages: DeliveredMessages::new(1),
				}]
				.into_iter()
				.collect(),
			},
		))),
		UnrewardedRelayersState {
			unrewarded_relayer_entries: 1,
			messages_in_oldest_entry: 1,
			total_messages: 1,
			last_delivered_nonce: 1,
		},
	));

	assert_eq!(
		System::<TestRuntime>::events(),
		vec![EventRecord {
			phase: Phase::Initialization,
			event: TestEvent::Messages(Event::MessagesDelivered {
				lane_id: TEST_LANE_ID,
				messages: DeliveredMessages::new(1),
			}),
			topics: vec![],
		}],
	);
}

#[test]
fn pallet_rejects_transactions_if_halted() {
	run_test(|| {
		// send message first to be able to check that delivery_proof fails later
		send_regular_message();

		PalletOperatingMode::<TestRuntime, ()>::put(MessagesOperatingMode::Basic(
			BasicOperatingMode::Halted,
		));

		assert_noop!(
			send_message::<TestRuntime, ()>(TEST_LANE_ID, REGULAR_PAYLOAD,),
			Error::<TestRuntime, ()>::NotOperatingNormally,
		);

		assert_noop!(
			Pallet::<TestRuntime>::receive_messages_proof(
				RuntimeOrigin::signed(1),
				TEST_RELAYER_A,
				Ok(vec![message(2, REGULAR_PAYLOAD)]).into(),
				1,
				REGULAR_PAYLOAD.declared_weight,
			),
			Error::<TestRuntime, ()>::BridgeModule(bp_runtime::OwnedBridgeModuleError::Halted),
		);

		assert_noop!(
			Pallet::<TestRuntime>::receive_messages_delivery_proof(
				RuntimeOrigin::signed(1),
				TestMessagesDeliveryProof(Ok((
					TEST_LANE_ID,
					InboundLaneData {
						last_confirmed_nonce: 1,
						relayers: vec![unrewarded_relayer(1, 1, TEST_RELAYER_A)]
							.into_iter()
							.collect(),
					},
				))),
				UnrewardedRelayersState {
					unrewarded_relayer_entries: 1,
					messages_in_oldest_entry: 1,
					total_messages: 1,
					last_delivered_nonce: 1,
				},
			),
			Error::<TestRuntime, ()>::BridgeModule(bp_runtime::OwnedBridgeModuleError::Halted),
		);
	});
}

#[test]
fn pallet_rejects_new_messages_in_rejecting_outbound_messages_operating_mode() {
	run_test(|| {
		// send message first to be able to check that delivery_proof fails later
		send_regular_message();

		PalletOperatingMode::<TestRuntime, ()>::put(
			MessagesOperatingMode::RejectingOutboundMessages,
		);

		assert_noop!(
			send_message::<TestRuntime, ()>(TEST_LANE_ID, REGULAR_PAYLOAD,),
			Error::<TestRuntime, ()>::NotOperatingNormally,
		);

		assert_ok!(Pallet::<TestRuntime>::receive_messages_proof(
			RuntimeOrigin::signed(1),
			TEST_RELAYER_A,
			Ok(vec![message(1, REGULAR_PAYLOAD)]).into(),
			1,
			REGULAR_PAYLOAD.declared_weight,
		),);

		assert_ok!(Pallet::<TestRuntime>::receive_messages_delivery_proof(
			RuntimeOrigin::signed(1),
			TestMessagesDeliveryProof(Ok((
				TEST_LANE_ID,
				InboundLaneData {
					last_confirmed_nonce: 1,
					relayers: vec![unrewarded_relayer(1, 1, TEST_RELAYER_A)].into_iter().collect(),
				},
			))),
			UnrewardedRelayersState {
				unrewarded_relayer_entries: 1,
				messages_in_oldest_entry: 1,
				total_messages: 1,
				last_delivered_nonce: 1,
			},
		));
	});
}

#[test]
fn send_message_works() {
	run_test(|| {
		send_regular_message();
	});
}

#[test]
fn send_message_rejects_too_large_message() {
	run_test(|| {
		let mut message_payload = message_payload(1, 0);
		// the payload isn't simply extra, so it'll definitely overflow
		// `MAX_OUTBOUND_PAYLOAD_SIZE` if we add `MAX_OUTBOUND_PAYLOAD_SIZE` bytes to extra
		message_payload
			.extra
			.extend_from_slice(&[0u8; MAX_OUTBOUND_PAYLOAD_SIZE as usize]);
		assert_noop!(
			send_message::<TestRuntime, ()>(TEST_LANE_ID, message_payload.clone(),),
			Error::<TestRuntime, ()>::MessageRejectedByPallet(VerificationError::MessageTooLarge),
		);

		// let's check that we're able to send `MAX_OUTBOUND_PAYLOAD_SIZE` messages
		while message_payload.encoded_size() as u32 > MAX_OUTBOUND_PAYLOAD_SIZE {
			message_payload.extra.pop();
		}
		assert_eq!(message_payload.encoded_size() as u32, MAX_OUTBOUND_PAYLOAD_SIZE);
		assert_ok!(send_message::<TestRuntime, ()>(TEST_LANE_ID, message_payload,),);
	})
}

#[test]
fn chain_verifier_rejects_invalid_message_in_send_message() {
	run_test(|| {
		// messages with this payload are rejected by target chain verifier
		assert_noop!(
			send_message::<TestRuntime, ()>(TEST_LANE_ID, PAYLOAD_REJECTED_BY_TARGET_CHAIN,),
			Error::<TestRuntime, ()>::MessageRejectedByChainVerifier(VerificationError::Other(
				mock::TEST_ERROR
			)),
		);
	});
}

#[test]
fn receive_messages_proof_works() {
	run_test(|| {
		assert_ok!(Pallet::<TestRuntime>::receive_messages_proof(
			RuntimeOrigin::signed(1),
			TEST_RELAYER_A,
			Ok(vec![message(1, REGULAR_PAYLOAD)]).into(),
			1,
			REGULAR_PAYLOAD.declared_weight,
		));

		assert_eq!(InboundLanes::<TestRuntime>::get(TEST_LANE_ID).0.last_delivered_nonce(), 1);

		assert!(TestDeliveryPayments::is_reward_paid(1));
	});
}

#[test]
fn receive_messages_proof_updates_confirmed_message_nonce() {
	run_test(|| {
		// say we have received 10 messages && last confirmed message is 8
		InboundLanes::<TestRuntime, ()>::insert(
			TEST_LANE_ID,
			InboundLaneData {
				last_confirmed_nonce: 8,
				relayers: vec![
					unrewarded_relayer(9, 9, TEST_RELAYER_A),
					unrewarded_relayer(10, 10, TEST_RELAYER_B),
				]
				.into_iter()
				.collect(),
			},
		);
		assert_eq!(
			inbound_unrewarded_relayers_state(TEST_LANE_ID),
			UnrewardedRelayersState {
				unrewarded_relayer_entries: 2,
				messages_in_oldest_entry: 1,
				total_messages: 2,
				last_delivered_nonce: 10,
			},
		);

		// message proof includes outbound lane state with latest confirmed message updated to 9
		let mut message_proof: TestMessagesProof = Ok(vec![message(11, REGULAR_PAYLOAD)]).into();
		message_proof.result.as_mut().unwrap()[0].1.lane_state =
			Some(OutboundLaneData { latest_received_nonce: 9, ..Default::default() });

		assert_ok!(Pallet::<TestRuntime>::receive_messages_proof(
			RuntimeOrigin::signed(1),
			TEST_RELAYER_A,
			message_proof,
			1,
			REGULAR_PAYLOAD.declared_weight,
		));

		assert_eq!(
			InboundLanes::<TestRuntime>::get(TEST_LANE_ID).0,
			InboundLaneData {
				last_confirmed_nonce: 9,
				relayers: vec![
					unrewarded_relayer(10, 10, TEST_RELAYER_B),
					unrewarded_relayer(11, 11, TEST_RELAYER_A)
				]
				.into_iter()
				.collect(),
			},
		);
		assert_eq!(
			inbound_unrewarded_relayers_state(TEST_LANE_ID),
			UnrewardedRelayersState {
				unrewarded_relayer_entries: 2,
				messages_in_oldest_entry: 1,
				total_messages: 2,
				last_delivered_nonce: 11,
			},
		);
	});
}

#[test]
fn receive_messages_proof_does_not_accept_message_if_dispatch_weight_is_not_enough() {
	run_test(|| {
		let mut declared_weight = REGULAR_PAYLOAD.declared_weight;
		*declared_weight.ref_time_mut() -= 1;
		assert_noop!(
			Pallet::<TestRuntime>::receive_messages_proof(
				RuntimeOrigin::signed(1),
				TEST_RELAYER_A,
				Ok(vec![message(1, REGULAR_PAYLOAD)]).into(),
				1,
				declared_weight,
			),
			Error::<TestRuntime, ()>::InsufficientDispatchWeight
		);
		assert_eq!(InboundLanes::<TestRuntime>::get(TEST_LANE_ID).last_delivered_nonce(), 0);
	});
}

#[test]
fn receive_messages_proof_rejects_invalid_proof() {
	run_test(|| {
		assert_noop!(
			Pallet::<TestRuntime, ()>::receive_messages_proof(
				RuntimeOrigin::signed(1),
				TEST_RELAYER_A,
				Err(()).into(),
				1,
				Weight::zero(),
			),
			Error::<TestRuntime, ()>::InvalidMessagesProof,
		);
	});
}

#[test]
fn receive_messages_proof_rejects_proof_with_too_many_messages() {
	run_test(|| {
		assert_noop!(
			Pallet::<TestRuntime, ()>::receive_messages_proof(
				RuntimeOrigin::signed(1),
				TEST_RELAYER_A,
				Ok(vec![message(1, REGULAR_PAYLOAD)]).into(),
				u32::MAX,
				Weight::zero(),
			),
			Error::<TestRuntime, ()>::TooManyMessagesInTheProof,
		);
	});
}

#[test]
fn receive_messages_delivery_proof_works() {
	run_test(|| {
		send_regular_message();
		receive_messages_delivery_proof();

		assert_eq!(OutboundLanes::<TestRuntime, ()>::get(TEST_LANE_ID).latest_received_nonce, 1,);
	});
}

#[test]
fn receive_messages_delivery_proof_rewards_relayers() {
	run_test(|| {
		assert_ok!(send_message::<TestRuntime, ()>(TEST_LANE_ID, REGULAR_PAYLOAD,));
		assert_ok!(send_message::<TestRuntime, ()>(TEST_LANE_ID, REGULAR_PAYLOAD,));

		// this reports delivery of message 1 => reward is paid to TEST_RELAYER_A
		let single_message_delivery_proof = TestMessagesDeliveryProof(Ok((
			TEST_LANE_ID,
			InboundLaneData {
				relayers: vec![unrewarded_relayer(1, 1, TEST_RELAYER_A)].into_iter().collect(),
				..Default::default()
			},
		)));
		let single_message_delivery_proof_size = single_message_delivery_proof.size();
		let result = Pallet::<TestRuntime>::receive_messages_delivery_proof(
			RuntimeOrigin::signed(1),
			single_message_delivery_proof,
			UnrewardedRelayersState {
				unrewarded_relayer_entries: 1,
				messages_in_oldest_entry: 1,
				total_messages: 1,
				last_delivered_nonce: 1,
			},
		);
		assert_ok!(result);
		assert_eq!(
			result.unwrap().actual_weight.unwrap(),
			TestWeightInfo::receive_messages_delivery_proof_weight(
				&PreComputedSize(single_message_delivery_proof_size as _),
				&UnrewardedRelayersState {
					unrewarded_relayer_entries: 1,
					total_messages: 1,
					..Default::default()
				},
			)
		);
		assert!(TestDeliveryConfirmationPayments::is_reward_paid(TEST_RELAYER_A, 1));
		assert!(!TestDeliveryConfirmationPayments::is_reward_paid(TEST_RELAYER_B, 1));

		// this reports delivery of both message 1 and message 2 => reward is paid only to
		// TEST_RELAYER_B
		let two_messages_delivery_proof = TestMessagesDeliveryProof(Ok((
			TEST_LANE_ID,
			InboundLaneData {
				relayers: vec![
					unrewarded_relayer(1, 1, TEST_RELAYER_A),
					unrewarded_relayer(2, 2, TEST_RELAYER_B),
				]
				.into_iter()
				.collect(),
				..Default::default()
			},
		)));
		let two_messages_delivery_proof_size = two_messages_delivery_proof.size();
		let result = Pallet::<TestRuntime>::receive_messages_delivery_proof(
			RuntimeOrigin::signed(1),
			two_messages_delivery_proof,
			UnrewardedRelayersState {
				unrewarded_relayer_entries: 2,
				messages_in_oldest_entry: 1,
				total_messages: 2,
				last_delivered_nonce: 2,
			},
		);
		assert_ok!(result);
		// even though the pre-dispatch weight was for two messages, the actual weight is
		// for single message only
		assert_eq!(
			result.unwrap().actual_weight.unwrap(),
			TestWeightInfo::receive_messages_delivery_proof_weight(
				&PreComputedSize(two_messages_delivery_proof_size as _),
				&UnrewardedRelayersState {
					unrewarded_relayer_entries: 1,
					total_messages: 1,
					..Default::default()
				},
			)
		);
		assert!(!TestDeliveryConfirmationPayments::is_reward_paid(TEST_RELAYER_A, 1));
		assert!(TestDeliveryConfirmationPayments::is_reward_paid(TEST_RELAYER_B, 1));
	});
}

#[test]
fn receive_messages_delivery_proof_rejects_invalid_proof() {
	run_test(|| {
		assert_noop!(
			Pallet::<TestRuntime>::receive_messages_delivery_proof(
				RuntimeOrigin::signed(1),
				TestMessagesDeliveryProof(Err(())),
				Default::default(),
			),
			Error::<TestRuntime, ()>::InvalidMessagesDeliveryProof,
		);
	});
}

#[test]
fn receive_messages_delivery_proof_rejects_proof_if_declared_relayers_state_is_invalid() {
	run_test(|| {
		// when number of relayers entries is invalid
		assert_noop!(
			Pallet::<TestRuntime>::receive_messages_delivery_proof(
				RuntimeOrigin::signed(1),
				TestMessagesDeliveryProof(Ok((
					TEST_LANE_ID,
					InboundLaneData {
						relayers: vec![
							unrewarded_relayer(1, 1, TEST_RELAYER_A),
							unrewarded_relayer(2, 2, TEST_RELAYER_B)
						]
						.into_iter()
						.collect(),
						..Default::default()
					}
				))),
				UnrewardedRelayersState {
					unrewarded_relayer_entries: 1,
					total_messages: 2,
					last_delivered_nonce: 2,
					..Default::default()
				},
			),
			Error::<TestRuntime, ()>::InvalidUnrewardedRelayersState,
		);

		// when number of messages is invalid
		assert_noop!(
			Pallet::<TestRuntime>::receive_messages_delivery_proof(
				RuntimeOrigin::signed(1),
				TestMessagesDeliveryProof(Ok((
					TEST_LANE_ID,
					InboundLaneData {
						relayers: vec![
							unrewarded_relayer(1, 1, TEST_RELAYER_A),
							unrewarded_relayer(2, 2, TEST_RELAYER_B)
						]
						.into_iter()
						.collect(),
						..Default::default()
					}
				))),
				UnrewardedRelayersState {
					unrewarded_relayer_entries: 2,
					total_messages: 1,
					last_delivered_nonce: 2,
					..Default::default()
				},
			),
			Error::<TestRuntime, ()>::InvalidUnrewardedRelayersState,
		);

		// when last delivered nonce is invalid
		assert_noop!(
			Pallet::<TestRuntime>::receive_messages_delivery_proof(
				RuntimeOrigin::signed(1),
				TestMessagesDeliveryProof(Ok((
					TEST_LANE_ID,
					InboundLaneData {
						relayers: vec![
							unrewarded_relayer(1, 1, TEST_RELAYER_A),
							unrewarded_relayer(2, 2, TEST_RELAYER_B)
						]
						.into_iter()
						.collect(),
						..Default::default()
					}
				))),
				UnrewardedRelayersState {
					unrewarded_relayer_entries: 2,
					total_messages: 2,
					last_delivered_nonce: 8,
					..Default::default()
				},
			),
			Error::<TestRuntime, ()>::InvalidUnrewardedRelayersState,
		);
	});
}

#[test]
fn receive_messages_accepts_single_message_with_invalid_payload() {
	run_test(|| {
		let mut invalid_message = message(1, REGULAR_PAYLOAD);
		invalid_message.payload = Vec::new();

		assert_ok!(Pallet::<TestRuntime, ()>::receive_messages_proof(
			RuntimeOrigin::signed(1),
			TEST_RELAYER_A,
			Ok(vec![invalid_message]).into(),
			1,
			Weight::zero(), /* weight may be zero in this case (all messages are
			                 * improperly encoded) */
		),);

		assert_eq!(InboundLanes::<TestRuntime>::get(TEST_LANE_ID).last_delivered_nonce(), 1,);
	});
}

#[test]
fn receive_messages_accepts_batch_with_message_with_invalid_payload() {
	run_test(|| {
		let mut invalid_message = message(2, REGULAR_PAYLOAD);
		invalid_message.payload = Vec::new();

		assert_ok!(Pallet::<TestRuntime, ()>::receive_messages_proof(
			RuntimeOrigin::signed(1),
			TEST_RELAYER_A,
			Ok(vec![message(1, REGULAR_PAYLOAD), invalid_message, message(3, REGULAR_PAYLOAD),])
				.into(),
			3,
			REGULAR_PAYLOAD.declared_weight + REGULAR_PAYLOAD.declared_weight,
		),);

		assert_eq!(InboundLanes::<TestRuntime>::get(TEST_LANE_ID).last_delivered_nonce(), 3,);
	});
}

#[test]
fn actual_dispatch_weight_does_not_overlow() {
	run_test(|| {
		let message1 = message(1, message_payload(0, u64::MAX / 2));
		let message2 = message(2, message_payload(0, u64::MAX / 2));
		let message3 = message(3, message_payload(0, u64::MAX / 2));

		assert_noop!(
			Pallet::<TestRuntime, ()>::receive_messages_proof(
				RuntimeOrigin::signed(1),
				TEST_RELAYER_A,
				// this may cause overflow if source chain storage is invalid
				Ok(vec![message1, message2, message3]).into(),
				3,
				Weight::MAX,
			),
			Error::<TestRuntime, ()>::InsufficientDispatchWeight
		);
		assert_eq!(InboundLanes::<TestRuntime>::get(TEST_LANE_ID).last_delivered_nonce(), 0);
	});
}

#[test]
fn ref_time_refund_from_receive_messages_proof_works() {
	run_test(|| {
		fn submit_with_unspent_weight(
			nonce: MessageNonce,
			unspent_weight: u64,
		) -> (Weight, Weight) {
			let mut payload = REGULAR_PAYLOAD;
			*payload.dispatch_result.unspent_weight.ref_time_mut() = unspent_weight;
			let proof = Ok(vec![message(nonce, payload)]).into();
			let messages_count = 1;
			let pre_dispatch_weight =
				<TestRuntime as Config>::WeightInfo::receive_messages_proof_weight(
					&proof,
					messages_count,
					REGULAR_PAYLOAD.declared_weight,
				);
			let result = Pallet::<TestRuntime>::receive_messages_proof(
				RuntimeOrigin::signed(1),
				TEST_RELAYER_A,
				proof,
				messages_count,
				REGULAR_PAYLOAD.declared_weight,
			)
			.expect("delivery has failed");
			let post_dispatch_weight =
				result.actual_weight.expect("receive_messages_proof always returns Some");

			// message delivery transactions are never free
			assert_eq!(result.pays_fee, Pays::Yes);

			(pre_dispatch_weight, post_dispatch_weight)
		}

		// when dispatch is returning `unspent_weight < declared_weight`
		let (pre, post) = submit_with_unspent_weight(1, 1);
		assert_eq!(post.ref_time(), pre.ref_time() - 1);

		// when dispatch is returning `unspent_weight = declared_weight`
		let (pre, post) = submit_with_unspent_weight(2, REGULAR_PAYLOAD.declared_weight.ref_time());
		assert_eq!(post.ref_time(), pre.ref_time() - REGULAR_PAYLOAD.declared_weight.ref_time());

		// when dispatch is returning `unspent_weight > declared_weight`
		let (pre, post) =
			submit_with_unspent_weight(3, REGULAR_PAYLOAD.declared_weight.ref_time() + 1);
		assert_eq!(post.ref_time(), pre.ref_time() - REGULAR_PAYLOAD.declared_weight.ref_time());

		// when there's no unspent weight
		let (pre, post) = submit_with_unspent_weight(4, 0);
		assert_eq!(post.ref_time(), pre.ref_time());

		// when dispatch is returning `unspent_weight < declared_weight`
		let (pre, post) = submit_with_unspent_weight(5, 1);
		assert_eq!(post.ref_time(), pre.ref_time() - 1);
	});
}

#[test]
fn proof_size_refund_from_receive_messages_proof_works() {
	run_test(|| {
		let max_entries = mock::MaxUnrewardedRelayerEntriesAtInboundLane::get() as usize;

		// if there's maximal number of unrewarded relayer entries at the inbound lane, then
		// `proof_size` is unchanged in post-dispatch weight
		let proof: TestMessagesProof = Ok(vec![message(101, REGULAR_PAYLOAD)]).into();
		let messages_count = 1;
		let pre_dispatch_weight =
			<TestRuntime as Config>::WeightInfo::receive_messages_proof_weight(
				&proof,
				messages_count,
				REGULAR_PAYLOAD.declared_weight,
			);
		InboundLanes::<TestRuntime>::insert(
			TEST_LANE_ID,
			StoredInboundLaneData(InboundLaneData {
				relayers: vec![
					UnrewardedRelayer {
						relayer: 42,
						messages: DeliveredMessages { begin: 0, end: 100 }
					};
					max_entries
				]
				.into_iter()
				.collect(),
				last_confirmed_nonce: 0,
			}),
		);
		let post_dispatch_weight = Pallet::<TestRuntime>::receive_messages_proof(
			RuntimeOrigin::signed(1),
			TEST_RELAYER_A,
			proof.clone(),
			messages_count,
			REGULAR_PAYLOAD.declared_weight,
		)
		.unwrap()
		.actual_weight
		.unwrap();
		assert_eq!(post_dispatch_weight.proof_size(), pre_dispatch_weight.proof_size());

		// if count of unrewarded relayer entries is less than maximal, then some `proof_size`
		// must be refunded
		InboundLanes::<TestRuntime>::insert(
			TEST_LANE_ID,
			StoredInboundLaneData(InboundLaneData {
				relayers: vec![
					UnrewardedRelayer {
						relayer: 42,
						messages: DeliveredMessages { begin: 0, end: 100 }
					};
					max_entries - 1
				]
				.into_iter()
				.collect(),
				last_confirmed_nonce: 0,
			}),
		);
		let post_dispatch_weight = Pallet::<TestRuntime>::receive_messages_proof(
			RuntimeOrigin::signed(1),
			TEST_RELAYER_A,
			proof,
			messages_count,
			REGULAR_PAYLOAD.declared_weight,
		)
		.unwrap()
		.actual_weight
		.unwrap();
		assert!(
			post_dispatch_weight.proof_size() < pre_dispatch_weight.proof_size(),
			"Expected post-dispatch PoV {} to be less than pre-dispatch PoV {}",
			post_dispatch_weight.proof_size(),
			pre_dispatch_weight.proof_size(),
		);
	});
}

#[test]
fn messages_delivered_callbacks_are_called() {
	run_test(|| {
		send_regular_message();
		send_regular_message();
		send_regular_message();

		// messages 1+2 are confirmed in 1 tx, message 3 in a separate tx
		// dispatch of message 2 has failed
		let mut delivered_messages_1_and_2 = DeliveredMessages::new(1);
		delivered_messages_1_and_2.note_dispatched_message();
		let messages_1_and_2_proof = Ok((
			TEST_LANE_ID,
			InboundLaneData {
				last_confirmed_nonce: 0,
				relayers: vec![UnrewardedRelayer {
					relayer: 0,
					messages: delivered_messages_1_and_2.clone(),
				}]
				.into_iter()
				.collect(),
			},
		));
		let delivered_message_3 = DeliveredMessages::new(3);
		let messages_3_proof = Ok((
			TEST_LANE_ID,
			InboundLaneData {
				last_confirmed_nonce: 0,
				relayers: vec![UnrewardedRelayer { relayer: 0, messages: delivered_message_3 }]
					.into_iter()
					.collect(),
			},
		));

		// first tx with messages 1+2
		assert_ok!(Pallet::<TestRuntime>::receive_messages_delivery_proof(
			RuntimeOrigin::signed(1),
			TestMessagesDeliveryProof(messages_1_and_2_proof),
			UnrewardedRelayersState {
				unrewarded_relayer_entries: 1,
				messages_in_oldest_entry: 2,
				total_messages: 2,
				last_delivered_nonce: 2,
			},
		));
		// second tx with message 3
		assert_ok!(Pallet::<TestRuntime>::receive_messages_delivery_proof(
			RuntimeOrigin::signed(1),
			TestMessagesDeliveryProof(messages_3_proof),
			UnrewardedRelayersState {
				unrewarded_relayer_entries: 1,
				messages_in_oldest_entry: 1,
				total_messages: 1,
				last_delivered_nonce: 3,
			},
		));
	});
}

#[test]
fn receive_messages_delivery_proof_rejects_proof_if_trying_to_confirm_more_messages_than_expected()
{
	run_test(|| {
		// send message first to be able to check that delivery_proof fails later
		send_regular_message();

		// 1) InboundLaneData declares that the `last_confirmed_nonce` is 1;
		// 2) InboundLaneData has no entries => `InboundLaneData::last_delivered_nonce()`
		//    returns `last_confirmed_nonce`;
		// 3) it means that we're going to confirm delivery of messages 1..=1;
		// 4) so the number of declared messages (see `UnrewardedRelayersState`) is `0` and
		//    numer of actually confirmed messages is `1`.
		assert_noop!(
			Pallet::<TestRuntime>::receive_messages_delivery_proof(
				RuntimeOrigin::signed(1),
				TestMessagesDeliveryProof(Ok((
					TEST_LANE_ID,
					InboundLaneData { last_confirmed_nonce: 1, relayers: Default::default() },
				))),
				UnrewardedRelayersState { last_delivered_nonce: 1, ..Default::default() },
			),
			Error::<TestRuntime, ()>::ReceivalConfirmation(
				ReceivalConfirmationError::TryingToConfirmMoreMessagesThanExpected
			),
		);
	});
}

#[test]
fn storage_keys_computed_properly() {
	assert_eq!(
		PalletOperatingMode::<TestRuntime>::storage_value_final_key().to_vec(),
		bp_messages::storage_keys::operating_mode_key("Messages").0,
	);

	assert_eq!(
		OutboundMessages::<TestRuntime>::storage_map_final_key(MessageKey {
			lane_id: TEST_LANE_ID,
			nonce: 42
		}),
		bp_messages::storage_keys::message_key("Messages", &TEST_LANE_ID, 42).0,
	);

	assert_eq!(
		OutboundLanes::<TestRuntime>::storage_map_final_key(TEST_LANE_ID),
		bp_messages::storage_keys::outbound_lane_data_key("Messages", &TEST_LANE_ID).0,
	);

	assert_eq!(
		InboundLanes::<TestRuntime>::storage_map_final_key(TEST_LANE_ID),
		bp_messages::storage_keys::inbound_lane_data_key("Messages", &TEST_LANE_ID).0,
	);
}

#[test]
fn inbound_message_details_works() {
	run_test(|| {
		assert_eq!(
			Pallet::<TestRuntime>::inbound_message_data(
				TEST_LANE_ID,
				REGULAR_PAYLOAD.encode(),
				OutboundMessageDetails { nonce: 0, dispatch_weight: Weight::zero(), size: 0 },
			),
			InboundMessageDetails { dispatch_weight: REGULAR_PAYLOAD.declared_weight },
		);
	});
}

#[test]
fn on_idle_callback_respects_remaining_weight() {
	run_test(|| {
		send_regular_message();
		send_regular_message();
		send_regular_message();
		send_regular_message();

		assert_ok!(Pallet::<TestRuntime>::receive_messages_delivery_proof(
			RuntimeOrigin::signed(1),
			TestMessagesDeliveryProof(Ok((
				TEST_LANE_ID,
				InboundLaneData {
					last_confirmed_nonce: 4,
					relayers: vec![unrewarded_relayer(1, 4, TEST_RELAYER_A)].into_iter().collect(),
				},
			))),
			UnrewardedRelayersState {
				unrewarded_relayer_entries: 1,
				messages_in_oldest_entry: 4,
				total_messages: 4,
				last_delivered_nonce: 4,
			},
		));

		// all 4 messages may be pruned now
		assert_eq!(outbound_lane::<TestRuntime, ()>(TEST_LANE_ID).data().latest_received_nonce, 4);
		assert_eq!(outbound_lane::<TestRuntime, ()>(TEST_LANE_ID).data().oldest_unpruned_nonce, 1);
		System::<TestRuntime>::set_block_number(2);

		// if passed wight is too low to do anything
		let dbw = DbWeight::get();
		assert_eq!(Pallet::<TestRuntime, ()>::on_idle(0, dbw.reads_writes(1, 1)), Weight::zero(),);
		assert_eq!(outbound_lane::<TestRuntime, ()>(TEST_LANE_ID).data().oldest_unpruned_nonce, 1);

		// if passed wight is enough to prune single message
		assert_eq!(
			Pallet::<TestRuntime, ()>::on_idle(0, dbw.reads_writes(1, 2)),
			dbw.reads_writes(1, 2),
		);
		assert_eq!(outbound_lane::<TestRuntime, ()>(TEST_LANE_ID).data().oldest_unpruned_nonce, 2);

		// if passed wight is enough to prune two more messages
		assert_eq!(
			Pallet::<TestRuntime, ()>::on_idle(0, dbw.reads_writes(1, 3)),
			dbw.reads_writes(1, 3),
		);
		assert_eq!(outbound_lane::<TestRuntime, ()>(TEST_LANE_ID).data().oldest_unpruned_nonce, 4);

		// if passed wight is enough to prune many messages
		assert_eq!(
			Pallet::<TestRuntime, ()>::on_idle(0, dbw.reads_writes(100, 100)),
			dbw.reads_writes(1, 2),
		);
		assert_eq!(outbound_lane::<TestRuntime, ()>(TEST_LANE_ID).data().oldest_unpruned_nonce, 5);
	});
}

#[test]
fn on_idle_callback_is_rotating_lanes_to_prune() {
	run_test(|| {
		// send + receive confirmation for lane 1
		send_regular_message();
		receive_messages_delivery_proof();
		// send + receive confirmation for lane 2
		assert_ok!(send_message::<TestRuntime, ()>(TEST_LANE_ID_2, REGULAR_PAYLOAD,));
		assert_ok!(Pallet::<TestRuntime>::receive_messages_delivery_proof(
			RuntimeOrigin::signed(1),
			TestMessagesDeliveryProof(Ok((
				TEST_LANE_ID_2,
				InboundLaneData {
					last_confirmed_nonce: 1,
					relayers: vec![unrewarded_relayer(1, 1, TEST_RELAYER_A)].into_iter().collect(),
				},
			))),
			UnrewardedRelayersState {
				unrewarded_relayer_entries: 1,
				messages_in_oldest_entry: 1,
				total_messages: 1,
				last_delivered_nonce: 1,
			},
		));

		// nothing is pruned yet
		assert_eq!(outbound_lane::<TestRuntime, ()>(TEST_LANE_ID).data().latest_received_nonce, 1);
		assert_eq!(outbound_lane::<TestRuntime, ()>(TEST_LANE_ID).data().oldest_unpruned_nonce, 1);
		assert_eq!(
			outbound_lane::<TestRuntime, ()>(TEST_LANE_ID_2).data().latest_received_nonce,
			1
		);
		assert_eq!(
			outbound_lane::<TestRuntime, ()>(TEST_LANE_ID_2).data().oldest_unpruned_nonce,
			1
		);

		// in block#2.on_idle lane messages of lane 1 are pruned
		let dbw = DbWeight::get();
		System::<TestRuntime>::set_block_number(2);
		assert_eq!(
			Pallet::<TestRuntime, ()>::on_idle(0, dbw.reads_writes(100, 100)),
			dbw.reads_writes(1, 2),
		);
		assert_eq!(outbound_lane::<TestRuntime, ()>(TEST_LANE_ID).data().oldest_unpruned_nonce, 2);
		assert_eq!(
			outbound_lane::<TestRuntime, ()>(TEST_LANE_ID_2).data().oldest_unpruned_nonce,
			1
		);

		// in block#3.on_idle lane messages of lane 2 are pruned
		System::<TestRuntime>::set_block_number(3);

		assert_eq!(
			Pallet::<TestRuntime, ()>::on_idle(0, dbw.reads_writes(100, 100)),
			dbw.reads_writes(1, 2),
		);
		assert_eq!(outbound_lane::<TestRuntime, ()>(TEST_LANE_ID).data().oldest_unpruned_nonce, 2);
		assert_eq!(
			outbound_lane::<TestRuntime, ()>(TEST_LANE_ID_2).data().oldest_unpruned_nonce,
			2
		);
	});
}

#[test]
fn outbound_message_from_unconfigured_lane_is_rejected() {
	run_test(|| {
		assert_noop!(
			send_message::<TestRuntime, ()>(TEST_LANE_ID_3, REGULAR_PAYLOAD,),
			Error::<TestRuntime, ()>::InactiveOutboundLane,
		);
	});
}

#[test]
fn test_bridge_messages_call_is_correctly_defined() {
	let account_id = 1;
	let message_proof: TestMessagesProof = Ok(vec![message(1, REGULAR_PAYLOAD)]).into();
	let message_delivery_proof = TestMessagesDeliveryProof(Ok((
		TEST_LANE_ID,
		InboundLaneData {
			last_confirmed_nonce: 1,
			relayers: vec![UnrewardedRelayer { relayer: 0, messages: DeliveredMessages::new(1) }]
				.into_iter()
				.collect(),
		},
	)));
	let unrewarded_relayer_state = UnrewardedRelayersState {
		unrewarded_relayer_entries: 1,
		total_messages: 1,
		last_delivered_nonce: 1,
		..Default::default()
	};

	let direct_receive_messages_proof_call = Call::<TestRuntime>::receive_messages_proof {
		relayer_id_at_bridged_chain: account_id,
		proof: message_proof.clone(),
		messages_count: 1,
		dispatch_weight: REGULAR_PAYLOAD.declared_weight,
	};
	let indirect_receive_messages_proof_call = BridgeMessagesCall::<
		AccountId,
		TestMessagesProof,
		TestMessagesDeliveryProof,
	>::receive_messages_proof {
		relayer_id_at_bridged_chain: account_id,
		proof: message_proof,
		messages_count: 1,
		dispatch_weight: REGULAR_PAYLOAD.declared_weight,
	};
	assert_eq!(
		direct_receive_messages_proof_call.encode(),
		indirect_receive_messages_proof_call.encode()
	);

	let direct_receive_messages_delivery_proof_call =
		Call::<TestRuntime>::receive_messages_delivery_proof {
			proof: message_delivery_proof.clone(),
			relayers_state: unrewarded_relayer_state.clone(),
		};
	let indirect_receive_messages_delivery_proof_call = BridgeMessagesCall::<
		AccountId,
		TestMessagesProof,
		TestMessagesDeliveryProof,
	>::receive_messages_delivery_proof {
		proof: message_delivery_proof,
		relayers_state: unrewarded_relayer_state,
	};
	assert_eq!(
		direct_receive_messages_delivery_proof_call.encode(),
		indirect_receive_messages_delivery_proof_call.encode()
	);
}

generate_owned_bridge_module_tests!(
	MessagesOperatingMode::Basic(BasicOperatingMode::Normal),
	MessagesOperatingMode::Basic(BasicOperatingMode::Halted)
);

#[test]
fn inbound_storage_extra_proof_size_bytes_works() {
	fn relayer_entry() -> UnrewardedRelayer<TestRelayer> {
		UnrewardedRelayer { relayer: 42u64, messages: DeliveredMessages { begin: 0, end: 100 } }
	}

	fn storage(relayer_entries: usize) -> RuntimeInboundLaneStorage<TestRuntime, ()> {
		RuntimeInboundLaneStorage {
			lane_id: Default::default(),
			cached_data: Some(InboundLaneData {
				relayers: vec![relayer_entry(); relayer_entries].into_iter().collect(),
				last_confirmed_nonce: 0,
			}),
			_phantom: Default::default(),
		}
	}

	let max_entries = mock::MaxUnrewardedRelayerEntriesAtInboundLane::get() as usize;

	// when we have exactly `MaxUnrewardedRelayerEntriesAtInboundLane` unrewarded relayers
	assert_eq!(storage(max_entries).extra_proof_size_bytes(), 0);

	// when we have less than `MaxUnrewardedRelayerEntriesAtInboundLane` unrewarded relayers
	assert_eq!(
		storage(max_entries - 1).extra_proof_size_bytes(),
		relayer_entry().encode().len() as u64
	);
	assert_eq!(
		storage(max_entries - 2).extra_proof_size_bytes(),
		2 * relayer_entry().encode().len() as u64
	);

	// when we have more than `MaxUnrewardedRelayerEntriesAtInboundLane` unrewarded relayers
	// (shall not happen in practice)
	assert_eq!(storage(max_entries + 1).extra_proof_size_bytes(), 0);
}

#[test]
fn maybe_outbound_lanes_count_returns_correct_value() {
	assert_eq!(
		MaybeOutboundLanesCount::<TestRuntime, ()>::get(),
		Some(mock::ActiveOutboundLanes::get().len() as u32)
	);
}
