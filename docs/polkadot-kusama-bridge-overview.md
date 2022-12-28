# Polkadot <> Kusama Bridge Overview

This document describes how we use all components, described in the [High-Level Bridge Documentation](./high-level-overview.md),
to build the XCM bridge between Kusama and Polkadot. In this case, our components merely work as a XCM transport
(like XCMP/UMP/HRMP), between chains that are not a part of the same consensus system.

The overall architecture may be seen in [this diagram](./polkadot-kusama-bridge.html).

## Bridge Hubs

All operations at relay chain are expensive. Ideally all non-mandatory trasnsactions must happen on parachains.
That's why we are planning to have two parachains - Polkadot Bridge Hub under Polkadot consensus and Kusama
Bridge Hub under Kusama consensus.

The Bridge Hub will have all required bridge pallets in its runtime. We hope that later, other teams will be able to
use our bridge hubs too and have their pallets there.

The Bridge Hub will use the base token of the ecosystem - KSM at Kusama Bridge Hub and DOT at Polkadot Bridge Hub.
The runtime will have minimal set of non-bridge pallets, so there's not much you can do directly on bridge hubs.

## Connecting Parachains

You won't be able to directly use bridge hub transactions to send XCM messages over the bridge. Instead, you'll need
to use other parachains transactions, which will use HRMP to send message to the Bridge Hub. The Bridge Hub will
just queue this messages in its outbound lane, which is dedicated to deliver messages between two parachains.

Our first planned bridge will connect the Polkadot' Statemint and Kusama' Statemine. Bridge between those two
parachains would allow Statemint accounts to hold wrapped KSM tokens and Statemine accounts to hold wrapped DOT
tokens.

For that bridge (pair of parachains under different consensus systems) we'll be using the lane 00000000. Later,
when other parachains will join the bridge, they will be using other lanes for their messages.