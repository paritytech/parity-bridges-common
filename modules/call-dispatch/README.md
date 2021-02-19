# Call Dispatch Module

The Call Dispatch Module has single entry point for dispatching encoded calls. Every dispacth (successful or not) emits corresponding Module event. The Module has no any Call-related requirements - they may come from the Bridged Chain over some Message Lane, or they may be crafted locally. But here we'll mostly talk about this Module in the context of bridges.

Every message that is being dispatched has three main characteristics:
- `bridge` is the 4-bytes identifier of the bridge where this message comes from. The may be the identifier of the Bridged Chain (like b"rlto" for Rialto), or the identifier of the bridge itself (b"rimi" for Rialto <-> Millau bridge);
- `id` is the unique id of the message within given bridge. For messages coming from the Message Lane, it may wort to use a tuple `(LaneId, MessageNonce)` to identify a message;
- `message` is the Call itself and couple of additional fields, required for the dispatch (see events section below).

The easiest way to understand what is happening when call is being dispatched, is to look at Module events set:

- `MessageRejected` event is emitted if message has been rejected even before reaching the Module. Dispatch may be called just to reflect the fact that message has been received, but we have failed to pre-process it (e.g. because of corrupted encoding);
- `MessageVersionSpecMismatch` event is emitted if current runtime specification version differs from the version that has been used to encode the `Call`. The message payload has the `spec_version`, that is filled by the message submitter. If this value differs from the current runtime version, dispatch mechanism rejects to dispatch the message. Otherwise we may decode the wrong `Call` for example if method arguments were changed;
- `MessageCallDecodeFailed` event is emitted if we have failed to decode `Call` from the payload. This may happen if submitter has provided incorrect value in the `call` field, or if Source Chain storage has been corrupted. The `Call` is decoded after `spec_version` check, so we'll never try to decode `Call` from other runtime version;
- `MessageSignatureMismatch` event is emitted if submitter has chose to dispatch message using specified This Chain account (`pallet_bridge_call_dispatch::CallOrigin::TargetAccount` origin), but he has failed to prove that he owns the private key for this account;
- `MessageCallRejected` event is emitted if Module has been deployed with Call filter and this filter has rejected the Call. In your bridge you may choose to reject all messages except e.g. `pallet_balances::Module::transfer()` calls;
- `MessageWeightMismatch` event is emitted if message submitter has specified invalid Call dispatch weight in the `weight` field of the message payload. The value of this field is compared to pre-dispatch weight of the decoded `Call`. If it is less than pre-dispatch weight, the dispatch is rejected. Keep in mind, that even if post-dispatch weight will be less than specified, the submitter still have to pay the maximal fee;
- `MessageWeightMismatch` event is emitted if message has passed all checks and we have actually dispatched it. The dispatch may still fail, though - that's why are including dispatch result in the event payload.

When we talk about Module in context of bridges, these events are helping in following cases:
1) when message submitter has access to both chains state and wants to monitor what has happened with his message. Then he could use message id, received during his message submit transaction to filter events of Call Dispatch Module at the Target Chain && actually see what has happened with his message;
2) when message submitter only has access to the Source Chain state. In this case, your bridge may have additional mechanism to deliver dispatch proofs (which are this Module events) back to the Source Chain, thus allowing submitter to see what happens with his messages.
