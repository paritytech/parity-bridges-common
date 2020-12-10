In the scenarios, for simplicity, we call the chains Kusama (KSM token) and Polkadot (DOT token),
but they should be applicable to any other chains.

Notation:
- kX - user X interacting with Kusama chain.
- `k(kX)` - Kusama account id of user kX (native account id; usable on Kusama)
- `p(kX)` - Polkadot account id of user kX (account id derived from `k(kX)` usable on Polkadot)
- [Kusama] ... - Interaction happens on Kusama (i.e. the user interacts with Kusama chain)
- [Polkadot] ... - Interaction happens on Polkadot

Basic Scenarios
===========================

Scenario 1: Kusama's Alice receiving & spending DOTs.
---------------------------

Kusama's Alice (kAlice) receives 5 DOTs from Polkadot's Bob (pBob) and sends half of them to
kCharlie.

1. Generate kAlice's DOT address (`p(kAlice)`)
See: `bp_runtime::derive_account_id(b["pdot"], kAlice)`
See:
```
let hash = bp_polkadot::derive_kusama_account_id(kAlice);
let p_kAlice = bp_polkadot::AccountIdConverter::convert(hash);
```

2. [Polkadot] pBob transfers 5 DOTs to `p(kAlice)`
  1. Creates & Signs a transaction with `Transfer(..)`
  2. It is included in block.
  3. kAlice observers Polkadot chain to see her balance updated.

3. [Kusama] kAlice sends 2.5 DOTs to `p(kCharlie)`
  1. Alice prepars:
    `let call = polkadot::Call::Balances(polkadot::Balances::Transfer(p(kCharlie), 2.5DOT)).encode()`
    `let weight = call.get_dispatch_info().weight;`
  2. Alice prepares Kusama transaction:
    ```
    kusama::Call::MessageLane::<Instance=Polkadot>::send_message(
      lane_id,? // dot-transfer-lane (truncated to 4bytes)
      payload: MessagePayload {
        spec_version, // Get from current polkadot runtime (kind of hardcoded)
        weight: weight // Alice should know the exact dispatch weight of the call on the target
                // source verifies: at least to cover call.length() and below max weight
        call: call, // simply bytes, we don't know anything about that on the source chain
        origin: CallOrigin::SourceAccount(kAlice),
      },
      delivery_and_dispatch_fee: {
        (single_message_delivery_weight
          + convert_target_weight_to_source_weight(weight) // source weight = X * target weight
          + confirmation_transaction_weight
        ) * weight_to_fee + relayers_fee
      }, // ?
    )
    ```
  3. Alice sends Kusama transaction with the above `Call` and pays regular fees.
  4. The transaction is included in block `B1`

  -- Syncing headers
  5. Relayer sees that `B1` has not yet been delivered to the target chain.
    https://github.com/paritytech/parity-bridges-common/blob/8b327a94595c4a6fae6d7866e24ecf2390501e32/relays/headers-relay/src/sync_loop.rs#L199
  6. Relayer prepares transaction which delivers `B1` and with all of the missing ancestors to the
     target chain (one header per transaction).
     TODO add an issue: The relayer could use `utils.batch`
     TODO add an issue: use unsigned transactions to deliver headers.
  7. Polkadot on-chain Kusama LC learns about `B1` block.
     - it's stored in the on-chain storage.

  -- Syncing finality
  8. Relayer is subscribed to finality events on Kusama.
     Relayer get's a finality notification for `B3`
  9. The header sync informs the targe tchain about `B1..B3` blocks (see point 6)
  9. Relayer learns about missing finalization of `B1..B3` on the target chain
    https://github.com/paritytech/parity-bridges-common/blob/8b327a94595c4a6fae6d7866e24ecf2390501e32/relays/substrate/src/headers_maintain.rs#L107
  10. Relayer submits justification for B3 to the target chain (`finalize_header`)
    See #421 for multiple authority set changes support in Relayer (i.e. what block the target chain
    expects, not only what I have).
    Relayer is doing two things:
      - syncing on demand
      - and syncing as notifications come
  11. Polkadot learns about finality of `B1`.

  -- Syncing messages
  12. The relayer checks the on-chain storage (last finalized header on the source, best header on
      the target):
    - Kusama outbound lane
    - Polkadot inbound lane
    Lanes contains `latest_generated_nonce` and `latest_received_nonce` respectively.
    The relayer syncs messages between that range.
  13. The relayer gets a proof for every message in that range (RPC of message lanes module)
  14. Creates message delivery transaction (but it has weight & size limit and count limit)
      - count limit - just to make the loop of delivery code bounded
      ```
      receive_message_proof(
        relayer_id, // account id of the source chain
        proof, // messages + proofs (hash of source block `B1`, nonces, lane_id + storage proof)
        dispatch_weight // relayer declares how much it will take to dispatch all messages in that transaction,
      )
      ```
      The `proof` can also contain an update of outbound lane
      state of source chain, which indicates the delivery
      confirmation of these messages and reward payment,
      so that the target chain can truncate it's unpayed rewards vector.

      The target chain stores `relayer_ids` that delivered messages,
      because the relayer can generate a storage proof, that
      they did deliver those messages.
      The reward is being is being payed on the source chain
      and we inform the target chain about that fact, so
      it can prune these `relayer_ids`.

      It's totally fine if there is no messages, and we only include the reward payment proof
      when calling that function.

      TODO: in case we can't decode the dispatch payload, we don't even post an event, but we do
      in case of `spec_version` mismatch for instance.

      TODO: consider defering `Call::decode` up until we know that the `spec_version`
      is correct
      or fail to decode `MessagePayload` type in case `spec_version` mismatch.

      TODO: Replay protection in case of CallOrigin::TargetAccount.
      Currently the assumption is that `source_account_id` and `target_account_id` is controlled by
      the same person. Target users should NEVER sign anything if asked by source chain users.
    15. 🥳 the message is now delivered & dispatched on the target chain.
    16. The relayer now needs to confirm the delivery to claim her's reward.
    17. The relayer creates a transaction on the source chain with call:
    ```
    receive_messages_delivery_proof(
      proof, // hash of the finalized target chain block, lane_id, storage proof
    )
    ```
    TODO: Check if InboundLaneData needs `latest_*_nonce` - potentially this could be extracted
    from `relayers` vector (min nonce and max nonce from that vector).

    TODO: Re-think relayers strategy of confirmations, to align their incentives (currently they
    confirm whatever they see on-chain, so they might not get any rewards for such delivery,
    however they might be unblocking the lane (i.e. delivering more messages in the future)).

    TODO: Relayer fund account & relayer account that receives the rewards should always have
    balance above Existential Deposit (ED).

    TODO: Releayers could withdraw the acumualted rewards instead of having them transfered right
    away.


  ...

    TODO: RuntimeAPI to help with `delivery_and_dispatch_fee` (there is a function)
    `get_fee(dispatch_weight)`
    See: `runtime_common::messages::estimate_message_dispatch_and_delivery_fee`


UI challenges:
- The UI should warn before (or prevent) sending to `k(kCharlie)`!


Scenario 2: Kusama's Alice nominating validators with her DOTs.
---------------------------

kAlice receives 10 DOTs from pBob and nominates `p(pCharlie)` and `p(pDave)`.

1. Generate kAlice's DOT address (`p(kAlice)`)
2. [Polkadot] pBob transfers 5 DOTs to `p(kAlice)`
3. [Kusama] kAlice sends a batch transaction:
  - `staking::Bond` transaction to create stash account choosing `p(kAlice)` as the controller account.
  - `staking::Nominate(vec![p(pCharlie)])` to nominate pCharlie using the controller account.


Scenario 3: Kusama Treasury receiving & spending DOTs.
---------------------------

pBob sends 15 DOTs to Kusama Treasury which Kusama Governance decides to transfer to kCharlie.

1. Generate source account for the treasury (`kTreasury`).
2. [Polkadot] pBob tarnsfers 15 DOTs to `p(kTreasury)`.
2. [Kusama] Send a governance proposal to send a bridge message which transfers funds to `p(kCharlie)`.
3. [Kusama] Dispatch the governance proposal using `kTreasury` account id.

Extra scenarios
===========================

Scenario 4: Kusama's Alice setting up 1-of-2 multi-sig to spend from either Kusama or Polkadot
---------------------------

Assuming `p(pAlice)` has at least 7 DOTs already.

1. Generate multisig account id: `pMultiSig = multi_account_id(&[p(kAlice), p(pAlice)], 1)`.
2. [Kusama] Transfer 7 DOTs to `pMultiSig` using `TargetAccount` origin of `pAlice`.
3. [Kusama] Transfer 2 DOTs to `p(kAlice)` from the multisig:
   - Send `multisig::as_multi_threshold_1(vec![p(pAlice)], balances::Transfer(p(kAlice), 2))`

Scenario 5: Kusama Treasury staking & nominating validators with DOTs.
---------------------------

Scenario 6: Kusama Treasury voting in Polkadot's democracy proposal.
---------------------------

Potentially interesting scenarios
===========================

Scenario 7: Polkadot's Bob spending his DOTs by using Kusama chain.
---------------------------

We can assume he holds KSM. Problem: he can pay fees, but can't really send (sign) a transaction?
Shall we support some kind of dispatcher?

Scenario 8: Kusama Governance taking over Kusama's Alice DOT holdings.
---------------------------

We use `SourceRoot` call to transfer her's DOTs to Kusama treasury.
