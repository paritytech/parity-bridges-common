# Substrate-to-Substrate Bridge Design

Goals:
- A mechanism for modules running on one Substrate chain to communicate with corresponding modules on another Substrate chain.

Assumptions
- Neither chain finalizes invalid blocks.
- There exist enough relay nodes to relay blocks and messages in a timely manner.
- If governance approves a buggy runtime on either their own chain or a bridged chain, this will be dealt with through governance decisions on both chains.

## Overview

A *bridge* allows two Substrate chains to communicate bidirectionally. On both bridged chains, there is a `bridge` Substrate module which manages the communication. Communication happens by asynchronously sending *messages* between the chains, which are processed on the receiving side by a designated Substrate module (separate from the `bridge` module itself). Over each bridge, there can be many unidirectional *lanes* through which a sequence of messages are transmitted. All messages are sent through a lane and messages within a single lane are processed in order. On one side of the bridge, a lane is called an *outbound lane* and on the other end it is an *inbound lane*. 

The `bridge` module effectively implements a Substrate light client for all bridged chains using the Grandpa finality mechanism. The module tracks recent finalized blocks from the bridged chains and their current validator sets. It uses this information to verify newly finalized blocks. *The bridge module cannot ensure validity of blocks on bridged chains and relies on the incentive mechanisms driving consensus for correct behavior.* Special nodes, called *bridge relay nodes*, are responsible for posting block headers from one chain to the bridged chain along with proof of finality.

This bridge module design separates bridges from lanes where bridges track the finalized state of the bridged chain and lanes manage the delivery of an ordered sequence of messages. The intent is that there may be multiple lanes through a single bridge, even in the same direction, offering the option of both sequential and parallel message delivery. The main benefit in separating the block finality proofs from the message deliveries is that Grandpa proofs are expensive to verify and need only be posted intermittently. This gives us the ability to update the bridges infrequently and still split message processing across blocks on the receiving chain relatively cheaply. Furthermore, there are likely to be multiple bridged applications running on the same two chains. Configuring each with its own lane is important for isolation so that one bridged application cannot delay messages on the other.

### Message delivery

The sending chain can queue messages for an outbound lane using the `bridge` module. Messages in a lane are assigned incrementing indices. At the beginning of each block, a new *message block* is created for each outbound lane. When a new outbound message is queued, it is appended to the lane's message block. The message blocks form a hash chain, so each message block contains a vector of queued messages, the index of its last message, and a hash of the previous message block in the lane. The hash of a message block is a commitment to the sequence of all messages up to its index and is called a *message log commitment*. When a block is closed, any message blocks that are empty are discarded and any that are non-empty are hashed to get the updated message log commitment. The hash chain structure is used so that the bridged chain can receive sparse message blocks and ensure that it receives all messages in order. In order words, the receiving side of the bridge does not need every block header from the sending chain.

Bridge relay nodes are able to easily observe new message blocks in the module state and post the message blocks to the receiving chain with an extrinsic. The message block may only be posted *after* the receiving chain observes finalization of the block containing it. When a message block is posted along with requisite proofs, the messages are processed by the designated *handler module* [**Probably fine, yes, but we should provide a default handler module that just interprets them as `Call`s from one of a set of `AccountId`s controlled by the external chain. - Gav**]. The receiving chain then updates its last received message index for the inbound lane. Finally, the sending side of the chain receives an asynchronous acknowledgement that the other end has received some messages. When blocks are finalized on a bridged chain, the relay nodes post proof of finality and the last received message indices of each inbound lane back to the sending chain. This should *not* be interpreted by the sending chain as an indication of successful processing, simply that the messages have been delivered. This information is still likely useful to gauge the number of queued, undelivered messages and implement conjestion control.

NOTE: Synchronization is required between the bridge relay client of a node and its state pruning. In particular, new states must be pinned until the bridge relay client reads new message blocks out of the block state and persists them independently.

### Rate limiting

One of the biggest remaining difficulties is conjestion control and rate limiting. If there is a difference in block latency or processing capacity between the bridged chains, the sending chain might send messages in an outbound lane faster than the receiving chain can process them. For the MVP, we assume that rate limiting will be implemented at the higher-level of modules communicating across the bridge.

## Preliminaries

There are a few types of proofs that are used through this specification.

### Storage proofs

Given a block header as an anchor, a *storage proof* proves that a given key, value pair is in the storage trie at that header. The proof is a Merkle branch through the Merkle trie.

### Ancestry proofs

Given a block header as an anchor, this proves that another block is an ancestor given its number and hash. The simplest form of an ancestry proof is the chain of intermediate headers since they form a hash chain. There is discussion on efficiently implementing these in Substrate in [#2023](https://github.com/paritytech/substrate/issues/2053) using a sort of structured hash skip-list or a Merkle Mountain Range.

### Grandpa proofs

A Grandpa proof is a proof that a certain block has been finalized by a given validator set. This consists of a multisignature by 2/3 of the validators (by weight) on blocks descended from the finalized block, along with ancestry proofs from the signed blocks to the finalized block.

## Specification

## Bridge

For each bridge, the `bridge` module on each chain stores:

- **Block number.** The number of a recent finalized block on the bridged chain.
- **Block hash.** A hash of the block on the bridged chain with the above number.
- **State root.** The state root of the block.
- **Validator set.** The validator set at the block.

Each bridge is assigned an integer ID on both connected chains. These IDs may differ on the two chains.

### Initialization

A new bridge is initialized through the governance process using the `initialize_bridge` runtime call to the `bridge` module.

`initialize_bridge(header, validator_set, validator_set_proof)`

- Generates a new, unique bridge ID.
- Verifies with the storage proof that the validator set at the header is `validator_set`.
- Sets the tracked block number, hash, state root, and validator set for the new bridge ID.

### Updates

A bridge can be updated chain using the `submit_finalized_header` runtime call on the `bridge` module.

`submit_finalized_header(bridge_id, last_block_hash, header, ancestry_proof, grandpa_proof)`

- Looks up the last finalized block number/hash from the bridge state and checks that the hash is `last_block_hash`.
- Verifies with the ancestry proof that the last header is an ancestor of the new one.
- Verifies with the Grandpa proof that the new header is finalized with the validator set of the old header.
- Updates tracked block number, hash, and state root.
- Update validator set. This only needs to change once per {era, epoch, I don't know the difference}. TODO: Figure out details of this. Look into how validator set changes are materialized as logs/digest items.

## Lanes

For each inbound lane, the `bridge` module tracks:
- Last received message index.
- Last message log commitment over received messages.

For each outbound lane, the `bridge` module tracks:
- Last message index sent.
- Last message log commitment over sent messages.
- Last message index ACKed by the bridged chain.
 
### Message delivery

The message blocks sent from one chain are then relayed to the receiving chain with the `receive_messages` runtime call to the `bridge` module.

`receive_messages(bridge_id, lane_id, last_received_index, finalized_block_hash, header, ancestry_proof, message_blocks, message_log_commitment_proof)`

- Looks up the last finalized block given the bridge ID and check that its hash is `finalized_block_hash`.
- Looks up the last received index given the bridge ID and lane ID and check that it equals `last_received_index`.
- Verifies that `header` is an ancestor of the last finalized block using the ancestry proof.
- Computes the new message log commitment using the last message log commitment in the lane state and the sequence of `message_blocks`.
- Verifies that the new message log commitment is correct with respect to the header state using the message log commitment storage proof.
- Processes all messages in all message blocks by calling into the destination module.
- Updates the last received message index and last message log commitment to the new values.

### Delivery acknowledgement

Once a batch of messages is processed on the receiving chain and included in a finalized block, this information is relayed back to the sending chain with the `ack_outbound_messages` runtime call to the `bridge` module.

`ack_outbound_messages(bridge_id, lane_id, finalized_block_hash, acked_index, acked_index_proof)`

- Looks up the last finalized block given the bridge ID and check that its hash is `finalized_block_hash`.
- Verifies using the storage proof that the last received message index on the bridged chain at the finalized block is `acked_index`.
- Updates the last message index ACKed.

## Bridge Relay Nodes

There are special network clients that relay messages across bridges. They must be able to fetch Grandpa finality proofs, storage proofs, and ancestry proofs. They relay both finalized headers and message blocks over the bridges.

The bridge relay is some kind of service or process (distinction discussed below) connected to a primary chain and many bridged chains. This process effectively reads data from the primary chain and writes data to the bridged chains but submitting extrinsics. It can be seen as relaying data in only one direction -- outbound from the primary chain. To run a bidirectional bridge relay, an operator must run nodes and bridge relays for all communicating chains. This sort of configuration requires only one bridge relay process per chain, as compared to a design scaling linearly in the number of bridges, which could be quadratic in the number of chains.

The following is a pseudo-code specification of the core logic implemented by the bridge relays:

```
State:
    # The current finalized head on the local chain.
    current_finalized_head

    map bridge =>
        # The most recent locally finalized head that has been accepted
        # (but not necessarily finalized) on the bridged chain.
        locally_finalized_head_on_bridged_chain

    map outbound_lane =>
        # The most recent message in this lane that has been accepted
        # (but not necessarily finalized) on the bridged chain.
        last_message_on_bridged_chain
        
        # The most recent message in this lane that has been finalized
        # on the bridged chain.
        last_message_finalized_on_bridged_chain
        
        # The queue of outbound messages containing all messages between the
        # last message finalized on bridged chain and the last message sent
        # in a finalized block on the local chain.
        message_queue
        
    map inbound_lane =>
        # The index of the most recent message received in this lane on
        # by finalized block hash.
        queue<(finalized_block_number, last_message_received)>

reload_state():
    read_state_from_disk()

    let finalized_head = read_current_finalized_head()
    on_new_finalized_head(finalized_head)

    for bridge in bridges:
        let head_on_bridged_chain = read_head_on_bridged_chain()
        on_bridged_chain_new_head(head_on_bridged_chain)

        let finalized_head_on_bridged_chain = read_finalized_head_on_bridged_chain()
        on_bridged_chain_new_finalized_head(finalized_head_on_bridged_chain)

on_new_head(new_head):
    pin_state(new_head)

on_new_finalized_head(new_finalized_head):
    old_finalized_head = current_finalized_head
    current_finalized_head = new_finalized_head

    for bridge in bridges:
        if locally_finalized_head_on_bridged_chain.number < current_finalized_head.number:
            submit_current_finalized_header_to_bridged_chain(bridge)

        for block from old_finalized_head to current_finalized_head:
            for outbound_lane in bridge.outbound_lanes:
                extend_outbound_message_queue(outbound_lane, block)

        for inbound_lane in bridge.inbound_lanes:
            read_last_message_received(inbound_lane, new_finalized_head)

    unpin_state(old_finalized_head)

on_bridged_chain_new_head(bridge, new_head):
    let head_is_updated = 
        update_locally_finalized_head_on_bridged_chain(bridge, new_head)
   
    for outbound_lane in bridge.outbound_lanes:
        let last_message_is_updated =
            update_last_message_on_bridged_chain(outbound_lane, new_head)
        if head_is_updated or last_message_is_updated:
            submit_messages_to_bridged_chain(outbound_lane)

    if head_is_updated:
        for inbound_lane in bridge.inbound_lanes:
            submit_acked_index_to_bridged_chain(inbound_lane)

on_bridged_chain_new_finalized_head(bridge, new_finalized_head):
    for outbound_lane in bridge.outbound_lanes:
        let is_updated = 
            update_last_message_finalized_on_bridged_chain(outbound_lane, new_head)
        if is_updated:
            prune_outbound_message_queue(outbound_lane)

# Construct an submit an extrinsic to the bridged chain making a
# "receive_messages" runtime call with all messages in the queue between
# last_message_on_bridged_chain and the last message in the queue.
submit_messages_to_bridged_chain(outbound_lane)

# Construct and submit an extrinsic to the bridged chain making a
# "submit_finalized_header" runtime call with current_finalized_head.
submit_current_finalized_header_to_bridged_chain(outbound_lane)

# Construct and submit an extrinsic to the bridged chain making a
# "ack_outbound_messages" runtime call with the last_message_received index
# corresponding to the locally_finalized_head_on_bridged_chain. The queue
# of last_message_received indexes is pruned up to this block height.
submit_acked_index_to_bridged_chain(inbound_lane)

# Read the locally finalized head from the module state on the new head
# of the bridged chain. Set the locally_finalized_head_on_bridged_chain to
# this header. Return true if the value changed, otherwise false.
update_locally_finalized_head_on_bridged_chain(bridge, new_head)

# Read index of the last message received from the module state on the new
# head of the bridged chain. Set the last_message_on_bridged_chain
# to this header. Return true if the value changed, otherwise false.
update_last_message_on_bridged_chain(outbound_lane, new_head)

# Read index of the last message received from the module state on the new
# finalized head of the bridged chain. Set the 
# last_message_finalized_on_bridged_chain to this header. Return true if
# the value changed, otherwise false.
update_last_message_finalized_on_bridged_chain(outbound_lane, new_head)

# Pop outbound messages from the front of the queue until the message index
# is greater than the last_message_finalized_on_bridged_chain.
trim_outbound_message_queue(outbound_lane)

# Read any new message blocks from the module state at the given block
# and push any messages to the back of the queue.
extend_outbound_message_queue(outbound_lane, block)

# Read the index of the last message received in an inbound lane from the
# module state at the given block. Store this in the last_message_received
# queue along with the block number.
read_last_message_received(inbound_lane, block)

# Pin the block's state so that it does not get pruned from the state DB
# until it is explicitly unpinned.
pin_state(block)

# Unpin the state of this block and all its ancestors so that they may get
# pruned from the state DB.
unpin_state(block)
```

### In-process service vs separate RPC client process

The bridge relay can either be implemented as a Substrate service that runs in the same process as a node (either full or light) or as a standalone process that connects to the node via RPC. Note that in both cases, the bridge relay will connect to nodes running the bridged chain over RPC, even if connected to the primary chain in-process.

The benefit of having a separate process is simply isolation and modularity. This is potentially useful since each bridge relay must be configured on start-up with the RPC endpoints of all nodes running the bridged chains.

The main downside of this approach is that it makes state pinning much harder. Since messages sent from the bridge module are stored in state for only one block, it is necessary that the chain state not be pruned before the bridge relay service is able to read it and persist the messages in a separate store. If the bridge relay is an RPC client process, the first challenge is that the state pinning API is not currently exposed over RPC. Even if such an API were created, however, it would be difficult to avoid race conditions where the state is pruned before the RPC client ever connects or is pruned if the RPC shuts down unexpectedly.

### Stateless bridge relay

By pinning state more aggressively, it is possible to design an entirely stateless bridge relay, meaning that it requires no separate persistant data store other than the state database. A stateless relay would require access to all chain state after the last message with finalized delivery on each bridged chain. Since there may be multiple bridges, it actually means that the slowest or furthest behind bridge will block state pruning for the entire node. Since bridges may operate with differing levels of reliability from each other and from the chain itself, this seems like a poor tradeoff.

## Other Risks

### Oversized message blocks

In this scenario, the sending chain produces a message block that is too large or expensive to be processed in a single block on the receiving chain. Since a message block is the smallest unit of messages that can be delivered at once, and each message block must be delivered before any succeeding ones, in this case the lane will be blocked until capacity on the receiving chain is increased.

For the MVP, it is the responsibility of the sending chain to ensure this does not occur. This will most likely be implemented by setting a maximum byte size per outbound message block during bridge initialization. This parameter is static and may be updated through a governance mechanism.

Another solution which seems more robust is to have *inbound lane parameters* determined by the receiving chain and automatically replicated to the sending chain over the bridge. One inbound lane parameter is the message block capacity in bytes. All inbound lane parameters would be sent semi-synchronously (within some specified number of blocks) to the sending chain on a per-lane basis. While governance intervention is likely required to update lane parameters for capacity changes, this need only be done on one side of the bridge, not both.

### Long range attacks

In this scenario, a bridged chain for some reason stops processing finalized blocks from the other chain for an extended period of time. This would most likely be due to some bug in the runtime. If the bridge is inactive for so long that the validator set it has stored for the other chain becomes unbonded and is no longer economically incentivized to behave honestly, the inactive validator set might perform a long range attack and submit blocks from a fork with valid Grandpa validity proofs. During regular operation this is not possible because as long as the chain receives relatively up-to-date headers, the validator set will be updated as well.

To mitigate this risk, the bridge is configured with a bonding period on the other chain. The bridge is automatically disabled if it receives no new finalized blocks within this bonding period. If the bridge is disabled for this reason, it must be re-enabled with an up-to-date validator set through governance intervention.

### Substrate runtime upgrades

When a bridge is established between two chains, in theory both sides have some assurance of how messages that are sent will be processed because the runtime version is known. In practice, depending on the complexity of the runtime, it may be difficult to know exactly how messages will be processed. And of course, Substrate chains support runtime upgrades through chain governance, which can modify the message handling logic arbitrarily.

For robustness against changes to the message handling logic, the bridge may designate a [SPREE enclave](https://wiki.polkadot.network/en/latest/polkadot/learn/spree/) to process messages. Enclaves have additional behavioral guarantees beyond regular runtime code.