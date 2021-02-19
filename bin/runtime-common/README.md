# Helpers for Message Lane Module integration

The `messages` module of this crate contains a bunch of helpers for integrating Message Lane Module into your runtime. Basic pre-requisites of these helpers are:
- we're going to bridge Substrate-based chain with Substrate-based chain;
- both chains have Message Lane Module, Substrate Headers Module and Call Dispatch Module;
- all Message Lanes are identical and may be used to transfer the same messages;
- the messages sent over the bridge are dispatched using Call Dispatch Module;
- the messages are `pallet_bridge_call_dispatch::MessagePayload` structures, where call is encoded `Call` of the Target chain. So the `Call` is opaque to the Message Lane Module instance at the Source Chain. It is pre-encoded by the message submitter;
- all proofs in the Message Lane Module transactions are based on the storage proofs from the Bridged Chain: storage proof of the outbound message (value from the `pallet_message_lane::Store::MessagePayload` map), storage proof of the outbound lane state (value from the `pallet_message_lane::Store::OutboundLanes` map) and storage proof of the inbound lane state (value from the `pallet_message_lane::Store::InboundLanes` map);
- storage proofs are built at the finalized headers of the corresponding chain, so all Message Lane transactions that are accepting proofs are verifying header proofs using finalized Bridge Chain headers from Substrate Headers Module;

## `MessageBridge` trait

The essence of your integration will be a struct that implements a `MessageBridge` trait. Let's review every method and its possible implementation here:

- `MessageBridge::maximal_extrinsic_size_on_target_chain`: you will need to return maximal extrinsic size of the target chain from this function. This may be the constant that is updated when your runtime is upgraded, or you may use the Message Lane Parameters functionality to allow pallet owner to update this value often that upgrades happen.

- `MessageBridge::weight_limits_of_message_on_bridged_chain`: you'll need to return range of dispatch weights that the outbound message may take at the target chain. Please keep in mind that our helpers assume that the message is an encoded call of the target chain. But we never decode this call at the source chain. So you can't simply get dispatch weight from pre-dispatch information. Instead there are two options to prepare this range: if you know which calls are to be sent over your bridge, then you may just return weight ranges for these particular calls. Otherwise, if you're going to accept all kind of calls, you may just return range `[0; maximal incoming message dispatch weight]`. If you choose the latter, then you shall remember that the delivery transaction itself has some weight, so you can't accept messages with weight equal to maximal weight of extrinsic at the target chain. In our test chains, we reject all messages that have declared dispatch weight larger than 50% of the maximal bridged extrinsic weight.

- `MessageBridge::weight_of_delivery_transaction`: you will need to return maximal weight of delivery transaction that delivers given payload to the Target Chain. Please keep in mind that this is not dispatch weight, but the weight of transaction itself. There are three main things to notice:
1) the weight, returned from this function is then used to compute fee that the message sender needs to pay for delivery transaction. So it shall not be simple dispatch weight of delivery call - it should be the "weight" of transaction itself, including per-byte "weight", "weight" of signed extras and etc.
2) the delivery transaction brings storage proof of the message, not the message itself. So your transaction will include extra bytes. We suggest to compute size of single empty value storage proof at the Target Chain, increase this value a bit and hardcode it in the Source Chain runtime code. This size then must be added to the size of payload and included in the weight computation;
3) before implementing this function, please take a look at the weight formula of delivery transaction. It adds some extra weight for every additional byte of the proof (everything above `pallet_message_lane::EXPECTED_DEFAULT_MESSAGE_LENGTH`), so it's not trivial. Even better, please refer to our implementation for test chains for details.

- `MessageBridge::weight_of_delivery_confirmation_transaction_on_this_chain`: you'll need to return maximal weight of single message delivery confirmation transaction on This chain. All points from the previous paragraph are also relevant here.

- `MessageBridge::this_weight_to_this_balance`: this function needs to convert weight units into fee units on This Chain. Most probably this can be done by calling	`pallet_transaction_payment::Config::WeightToFee::calc()` for passed weight.

- `MessageBridge::bridged_weight_to_bridged_balance`: this function needs to convert weight units into fee units on Target Chain. The best case is when you have the same conversion formula on both chains - then you may just call the same formula from the previous paragraph. Otherwise, you'll need to hardcode this formula into your runtime.

- `MessageBridge::bridged_balance_to_this_balance`: this may be the easiest method to implement and the hardest to maintain at the same time. If you don't have any automatic methods to determine conversion rate, then you'll probably need to maintain it by yourself (by updating conversion rate, stored in runtime storage). This means that if you're too late with update, then you risk to accept messages with lower-than-expected fee. So it may be wise to has some reserve in this conversion rate, even if that means larger delivery and dispatch fees.

## `ChainWithMessageLanes` trait

Apart from its methods, `MessageBridge` also has two associated types that need to implement `ChainWithMessageLanes` trait. One is for This Chain and the other is for the Bridged Chain. The trait is quite a simple and can easily be implemented - you just need to specify types used at the corresponding chain. There are two exceptions, though. Both may be changed in the future. Here they are:

- `ChainWithMessageLanes::Call`: it isn't a good idea to reference Bridged Chain runtime from your runtime (cyclic references + maintaining on upgrades). So you can't know the type of Bridged Chain call in your runtime. This type isn't actually used at This Chain, so you may use `()` instead.

- `ChainWithMessageLanes::MessageLaneInstance`: this is used to compute runtime storage keys. There may be several instances of Message Lane pallet, included in the Runtime. Every instance stores messages and these messages stored under different keys. When we are verifying storage proofs from the Bridged Chain, we should know which instance we're talking to. This is fine, but there's significant inconvenience with that - This Chain runtime must have the same Message Lane pallet instance. This not necessarily mean that we should use the same instance on both chains - this instance may be used to bridge with other chain/instance, or may not be used at all.

## Helpers for using at the Source Chain

The helpers for the Source Chain reside in the `source` submodule of the `messages` module. The structs are: `FromThisChainMessagePayload`, `FromBridgedChainMessagesDeliveryProof`, `FromThisChainMessageVerifier`. And the helper functions are: `maximal_message_size`, `verify_chain_message`, `verify_messages_delivery_proof` and `estimate_message_dispatch_and_delivery_fee`.

`FromThisChainMessagePayload` is a message that sender sends through our bridge. It is the `pallet_bridge_call_dispatch::MessagePayload`, where `call` field is encoded Target Chain call. So at This Chain we don't see internals of this call - we jsut know its size.

`FromThisChainMessageVerifier` is an implementation of `bp_message_lane::LaneMessageVerifier`. It has following checks in its `verify_message` method:
1) it'll verify that the used outbound lane is enabled in our runtime;
2) it'll reject message if there are too many undelivered outbound messages at this lane. The sender need to wait while relayers will do their work before sending the message again;
3) it'll reject message if it has wrong dispatch origin declared. Like if submitter is not the root of This Chain, but it tries to dispatch the message at the Target Chain using `pallet_bridge_call_dispatch::CallOrigin::SourceRoot` origin. Or he has provided wrong signature in the `pallet_bridge_call_dispatch::CallOrigin::TargetAccount` origin;
4) it'll reject message if delivery and dispatch fee that the submitter wants to pay is lesser than the fee that is computed using `estimate_message_dispatch_and_delivery_fee` function.

`estimate_message_dispatch_and_delivery_fee` returns minimal fee that the submitter needs to pay for sending given message. The fee includes: payment for the delivery transaction at the Target chain, payment for delivery confirmation transaction on This Chain, payment for Call dispatch at the Target Chain and relayer interest.

`FromBridgedChainMessagesDeliveryProof` holds the lane id and the storage proof of this inbound lane state at the Bridged Chain. This also holds the hash of the Target Chain header, that was used to generate this storage proof. The proof is verified by the `verify_messages_delivery_proof`, which simply checks that the Target Chain header is finalized (using Substrate Headers Pallet) and then reads the inbound lane state from the proof.

`verify_chain_message` function check that the message may be delivered to the Bridged Chain. There are two main checks:
1) that the message size is less than or equal to the 2/3 of maximal extrinsic size at the Target Chain. We leave 1/3 for signed extras and for the storage proof overhead;
2) that the message dispatch weight is less than or equal to the 1/2 of maximal normal extrinsic weight at the Target Chain. We leave 1/2 for for the delivery transaction overhead.

## Helpers for using at the Target Chain

The helpers for the Source Chain reside in the `target` submodule of the `messages` module. The structs are: `FromBridgedChainMessagePayload`, `FromBridgedChainMessagesProof`, `FromBridgedChainMessagesProof`. And the helper functions are: `maximal_incoming_message_dispatch_weight`, `maximal_incoming_message_size` and `verify_messages_proof`.

`FromBridgedChainMessagePayload` corresponds to the `FromThisChainMessagePayload` at the Bridged Chain. We expect that messages with this payload are stored in the `OutboundMessages` storage map of the Message Lane Module. This map is used to build `FromBridgedChainMessagesProof`. The proof holds the lane id, range of Message Nonces included in the proof, storage proof of `OutboundMessages` entries and the hash of Bridged Chain header that has been used to build the proof. Additionally, there's storage proof may contain the proof of outbound lane state. It may be required to prune `relayers` entries at This Chain (see Message Lane Module documentation for details). This proof is verified by the `verify_messages_proof` function.
