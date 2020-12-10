# High-Level Bridge Documentation

## Purpose
Trustless connecting between two Substrate-based chains using GRANDPA finality.

## Overview
Even though we support two-way bridging, the documentation will generally talk about a one-sided
interaction. That's to say, we will only talk about syncing headers and messages from a _source_
chain to a _target_ chain. This is because the two-sided interaction is really just the one-sided
interaction with the source and target chains switched.

The bridge is built from various components. Here is a quick overview of the important ones.

### Header Sync
A light client of the source chain built into the target chain's runtime. It is a single FRAME
pallet. It provides a "source of truth" about the source chain headers which have been finalized.
This is useful for higher level applications.

### Headers Relayer
A standalone application connected to both chains. It submits every source chain header it sees to
the target chain through RPC.

### Message Delivery
A FRAME pallet built on top of the header sync pallet. It allows users to submit messages to the
source chain, which are to be delivered to the target chain. The delivery protocol doesn't care
about the payload more than it has to. Handles replay protection and message ordering.

### Message Dispatch
A FRAME pallet responsible for interpreting the payload of delivered messages.

### Message Relayer
A standalone application handling delivery of the messages from source chain to the target chain.

## Processes

### Substrate (GRANDPA) Header Sync
The header sync pallet (`pallet-substrate-bridge`) is an on-chain light client for chains which use
GRANDPA finality. It is part of the target chain's runtime, and accepts headers from the source
chain. Its main goals are to accept valid headers, track GRANDPA finality set changes, and verify
GRANDPA finality proofs (a.k.a justifications).

The pallet does not care about what block production mechanism is used for the source chain
(e.g Aura or BABE) as long as it uses the GRANDPA finality gadget. Due to this it is possible for
the pallet to import (but not necessarily finalize) headers which are _not_ valid according to the
source chain's block production mechanism.

The pallet has support for tracking forks and uses the longest chain rule to determine what the
canonical chain is. The pallet allows headers to be imported on a different fork from the canonical
one as long as the headers being imported don't conflict with already finalized headers (for
example, it will not allow importing a header at a lower height than the best finalized header).

When tracking authority set changes, the pallet - unlike the full GRANDPA protocol - does not
support tracking multiple authority set changes across forks. Each fork can have at most one pending
authority set change. This is done to prevent DoS attacks if GRANDPA on the source chain were to
stall for a long time (the pallet would have to do a lot of expensive ancestry checks to catch up).

The pallet has a simple interface consisting of two dispatchables:

1. `import_signed_header()`
2. `finalize_header()`

The `import_signed_header()` dispatchable does the following checks:
  - Has the header has already been imported?
  - Does the header extend a known fork?
  - Does the header signal an authority set change?
  - Does the header enact an authority set change?

The `finalize_header()` dispatchable does the following checks:
  - Have we previously imported this header?
  - Is the given justification valid for the given header?
  - Does the given header enact an authority set change?

#### Relayer strategy

TODO
- Weight costs
- Fee payments

### Message Passing


#### Message Lanes Delivery
<TODO>Details of the message lanes delivery protocol</TODO>
- it doesn't care about payload, configurability of dispatch mechanism
- ordered within lane
- lanes independent
- relayers not strictly bound to a lane
- delivery confirmations
- inbound/outbound lanes
- Message Lane strictly require bi-directional header sync (due to confirmations)
- Lanes are like channels
- describe races from relayer
-
- How weight is calculated
- Who is paying fees:
-   transaction execution
-   dispatch
-   delivery cost

#### Dispatching Messages
The message dispatch pallet (`pallet-bridge-call-dispatch`) is used to perform the actions specified
by messages which have come over the bridge. For Substrate-based chains this means interpreting the
source chain's message as a `Call` on the target chain.

An example `Call` of the target chain would look something like this:

```
target_runtime::Call::Balances(target_runtime::pallet_balances::Call::transfer(recipient, amount))
```

When sending a `Call` it must first be SCALE encoded and then sent to the source chain. The `Call`
is then delivered by the message lane delivery mechanism from the source chain to the target chain.
When a message is received the inbound message lane on the target chain will try and decode the
message payload into a `Call` enum. If it's successful it will be dispatched after wecheck that the
weight of the call does not exceed the weight declared by the sender. The relayer pays fees for
executing the transaction on the target chain, but her costs should be covered by the sender on the
source chain.

When dispatching messages there are three Origins which can be used by the target chain:
1. Root Origin
2. Source Origin
3. Target Origin

Senders of a message can indicate which one of the three origins they would like to dispatch their
message with. However, there are restrictions on who/what is allowed to dispatch messages with a
particular origin.

The Root origin represents the source chain's Root account on the target chain. This origin can can
only be dispatched on the target chain if the "send message" request was made by the Root origin of
the source chain - otherwise the message will fail to be dispatched.

The Source origin represents an account without a private key on the target chain. This account will
be generated/derived using the account ID of the sender on the source chain. We don't necessarily
require the source account id to be associated with a private key on the source chain either. This
is useful for representing things such as source chain proxies or pallets.

The Target origin represents an account with a private key on the target chain. The sender on the
source chain needs to prove ownership of this account by using their target chain private key to
sign: `(Call, SourceChainAccountId).encode()`. This will be included in the message payload and
verified by the target chain before dispatch.
