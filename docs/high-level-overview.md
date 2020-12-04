= The purpose of the bridge

Trustless connecting between two substrate-based chains using GRANDPA finality.

Repo:
https://github.com/paritytech/parity-bridges-common



We call these two chains:
- The source chain
- The target chain

= The design of the bridge

- Header sync - a light client of the source chain build into target's chain
                runtime - a FRAME pallet.

- Headers Relayer - a standalone application connected to both chains, and
                    submitting each source chain header to the target chain

- Message Delivery - a FRAME pallet, built on top of the header sync -
                     allowing users to submit messages to be delivered to the
                     target chain. The delivery protocol doesn't care about the
                     payload more than it has to. Handles replay protection and
                     message ordering.

- Message Dispatch - a FRAME pallet responsible for interpreting the payload of delivered
                     messages.

- Message Relayer  - a standalone application handling delivery of the messages from source
                     chain to the target chain.

= Header Sync =

<TODO>Details of the header sync pallet </TODO>

= Message Delivery =

<TODO>Details of the message lanes delivery protocol</TODO>
- delivery confirmations

= Message Dispatch =

<TODO>Details of dispatching mechanism - `Call:decode`</TODO>

<TODO>CallOrigin description</TODO>

= The flow =

The user of the source chain is able to trigger action on the


------------------





Readiness of components.
 - call filtering missing
 - weight benchmarks missing

