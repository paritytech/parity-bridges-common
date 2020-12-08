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

## Components

### Header Sync
The header sync pallet is an on-chain light client for chains which use Grandpa finality. It is part
of the target chain's runtime, and accepts headers from the source chain. Its main goals are to
accept valid headers, track Grandpa finality set changes, and verify Grandpa finality proofs.

The pallet has a simple interface consisting of two dispatchables. The first dispatchable accepts
headers from the source chain and checks their validity. It performs checks to make sure that the
incoming header doesn't conflict with headers the pallet already knows about (a possible conflict
could be that the incoming header is on different finalized fork).

When importing a header the pallet will also be checking headers for Grandpa authority set changes.
Substrate headers contain logs which signal when the next authority set change is supposed to
occur. As a rule, Grandpa authorities can only finalize blocks up to the authority set change block.

The second dispatachable is used to import a Grandpa justification with the expectation that it can
finalize a header that the pallet had previously imported. When importing a finality proof we
require the hash of a header which the pallet has previously imported through the first dispatchable
we talked about. We then verify the justification. This verification is done using basically a
copy-paste of the Grandpa finality justification code from Substrate.

If we find that the justification given for the current header was indeed valid ....

After verifying that a justification for a given header is valid, we then see if the newly finalized
header enacts an authority set change. A header enacts an authority set change if its block number
equals the one from an authority set change signal log we recieved while importing a header.

### Message Delivery

<TODO>Details of the message lanes delivery protocol</TODO>
- delivery confirmations
- configurability of dispatch mechanism
- inbound/outbound lanes
- races

### Message Dispatch
The message dispatch pallet (`pallet-bridge-call-dispatch`) is used to perform the actions specified
by messages which have come over the bridge. For Substrate-based chains this means interpreting the
source chain's message as a `Call` on the target chain.

An example `Call` on the source chain would look something like this:

```
target_runtime::Call::Balances(target_runtime::pallet_balances::Call::transfer(recipient, amount))
```

When sending a `Call` it must first be SCALE encoded. When a message is recieved the inbound
message lane on the target chain will try and decode the message payload into a `Call`. If succesful
the message is added to a list of messages. Messages from this list will be dispatched after we
check that their fees have been covered.

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

The Source origin represents an account without a private key on the target chain. This is useful
for representing things such as proxies or pallets. This account will be generated/derived using the
account ID of the sender on the source chain.

The Target origin represents an account with a private key on the target chain. The sender on the
source chain needs to prove ownership of this account by using their target chain private key to
sign: `(Call, SourceChainAccountId).encode()`. This will be included in the message payload and
verified by the target chain before dispatch.
