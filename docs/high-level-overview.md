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
A light client of the source chain built into the target chain's runtime. It is a single a FRAME
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

### Message Dispatch

<TODO>Details of dispatching mechanism - `Call:decode`</TODO>

<TODO>CallOrigin description</TODO>


## Application Flow


## Pallets
NOTE: This is from the old README

### Ethereum Bridge Runtime Module
The main job of this runtime module is to keep track of useful information an Ethereum PoA chain
which has been submitted by a bridge relayer. This includes:

  - Ethereum headers and their status (e.g are they the best header, are they finalized, etc.)
  - Current validator set, and upcoming validator sets

This runtime module has more responsibilties than simply storing headers and validator sets. It is
able to perform checks on the incoming headers to verify their general integrity, as well as whether
or not they've been finalized by the authorities on the PoA chain.

This module is laid out as so:

```
├── ethereum
│  └── src
│     ├── error.rs        // Runtime error handling
│     ├── finality.rs     // Manage finality operations
│     ├── import.rs       // Import new Ethereum headers
│     ├── lib.rs          // Store headers and validator set info
│     ├── validators.rs   // Track current and future PoA validator sets
│     └── verification.rs // Verify validity of incoming Ethereum headers
```

### Currency Exchange Runtime Module
The currency exchange module is used to faciliate cross-chain funds transfers. It works by accepting
a transaction which proves that funds were locked on one chain, and releases a corresponding amount
of funds on the recieving chain.

For example: Alice would like to send funds from chain A to chain B. What she would do is send a
transaction to chain A indicating that she would like to send funds to an address on chain B. This
transaction would contain the amount of funds she would like to send, as well as the address of the
recipient on chain B. These funds would now be locked on chain A. Once the block containing this
"locked-funds" transaction is finalized it can be relayed to chain B. Chain B will verify that this
transaction was included in a finalized block on chain A, and if successful deposit funds into the
recipient account on chain B.

Chain B would need a way to convert from a foreign currency to its local currency. How this is done
is left to the runtime developer for chain B.

This module is one example of how an on-chain light client can be used to prove a particular action
was taken on a foreign chain. In particular it enables transfers of the foreign chain's native
currency, but more sophisticated modules such as ERC20 token transfers or arbitrary message transfers
are being worked on as well.

### Substrate Bridge Runtime Module
👷 Under Construction 👷‍♀️


## Ethereum Node
On the Ethereum side of things, we require two things. First, a Solidity smart contract to track the
Substrate headers which have been submitted to the bridge (by the relay), and a built-in contract to
be able to verify that headers have been finalized by the Grandpa finality gadget. Together this
allows the Ethereum PoA chain to verify the integrity and finality of incoming Substrate headers.

The Solidity smart contract is not part of this repo, but can be found
[here](https://github.com/svyatonik/substrate-bridge-sol/blob/master/substrate-bridge.sol) if you're
curious. We have the contract ABI in the `ethereum/relays/res` directory.


## Rialto Runtime
The node runtime consists of several runtime modules, however not all of them are used at the same
time. When running an Ethereum PoA to Substrate bridge the modules required are the Ethereum module
and the currency exchange module. When running a Substrate to Substrate bridge the Substrate and
currency exchange modules are required.

Below is a brief description of each of the runtime modules.


## Bridge Relay
The bridge relay is responsible for syncing the chains which are being bridged, and passing messages
between them. The current implementation of the relay supportings syncing and interacting with
Ethereum PoA and Substrate chains.

The folder structure of the bridge relay is as follows:

```
├── relays
│  ├── ethereum
│  │  ├── res
│  │  │  └── ...
│  │  └── src
│  │     ├── ethereum_client.rs          // Interface for Ethereum RPC
│  │     ├── ethereum_deploy_contract.rs // Utility for deploying bridge contract to Ethereum
│  │     ├── ethereum_exchange.rs        // Relay proof of PoA -> Substrate exchange transactions
│  │     ├── ethereum_sync_loop.rs       // Sync headers from Ethereum, submit to Substrate
│  │     ├── ethereum_types.rs           // Useful Ethereum types
│  │     ├── exchange.rs                 // Relay proof of exchange transactions
│  │     ├── headers.rs                  // Track synced and incoming block headers
│  │     ├── main.rs                     // Entry point to binary
│  │     ├── substrate_client.rs         // Interface for Substrate RPC
│  │     ├── substrate_sync_loop.rs      // Sync headers from Substrate, submit to Ethereum
│  │     ├── substrate_types.rs          // Useful Ethereum types
│  │     ├── sync.rs                     // Sync configuration and helpers
│  │     ├── sync_loop.rs                // Header synchronization between source and target chains
│  │     ├── sync_types.rs               // Useful sync types
│  │     └── utils.rs                    // General utilities
```


------------------





Readiness of components.
 - call filtering missing
 - weight benchmarks missing
 -

