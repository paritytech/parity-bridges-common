# Parachains Finality Relay

The parachains finality relay works with two chains - source relay chain and target chain (which may be standalone
chain, relay chain or a parachain). The source chain must have the 
[`paras` pallet](https://github.com/paritytech/polkadot/tree/master/runtime/parachains/src/paras) deployed at its
runtime. The target chain must have the [bridge parachains pallet](../../modules/parachains/) deployed at its runtime.




is able to work with different finality engines. In the modern Substrate world they are GRANDPA
and BEEFY. Let's talk about GRANDPA here, because BEEFY relay and bridge BEEFY pallet are in development.

In general, the relay works as follows: it connects to the source and target chain. The source chain must have the 
[GRANDPA gadget](https://github.com/paritytech/finality-grandpa) running (so it can't be a parachain). The target
chain must have the [bridge GRANDPA pallet](../../modules/grandpa/) deployed at its runtime. The relay subscribes
to the GRANDPA finality notifications at the source chain and when the new justification is received, it is submitted
to the pallet at the target chain.

Apart from that, the relay is watching for every source header that is missing at target. If it finds the missing
mandatory header (header that is changing the current GRANDPA validators set), it submits the justification for
this header. The case when the source node can't return the mandatory justification is considered a fatal error,
because the pallet can't proceed without it.

## How to Use the Finality Relay

The most important trait is the [`FinalitySyncPipeline`](./src/lib.rs), which defines the basic primitives of the
source chain (like block hash and number) and the type of finality proof (GRANDPA jusitfication or MMR proof). Once
that is defined, there are two other traits - [`SourceClient`](./src/finality_loop.rs) and
[`TarggetClient`](./src/finality_loop.rs).

The `SourceClient` represents the Substrate node client that connects to the source chain. The client need to
be able to return the best finalized header number, finalized header and its finality proof and the stream of
finality proofs.

The `TargetClient` implementation must be able to craft finality delivery transaction and submit it to the target
node. The transaction is then tracked by the relay until it is mined and finalized.

The main entrypoint for the crate is the [`run` function](./src/finality_loop.rs), which takes source and target
clients and [`FinalitySyncParams`](./src/finality_loop.rs) parameters. The most imporant parameter is the
`only_mandatory_headers` - it it is set to `true`, the relay will only submit mandatory headers. Since transactions
with mandatory headers are fee-free, the cost of running such relay is zero (in terms of fees).
