# client-westend

Relay client for the **Westend** testnet relay chain.

## Chain type

Relay chain with GRANDPA finality.

## Traits implemented

`Chain`, `ChainWithGrandpa`, `ChainWithBalances`, `ChainWithTransactions` (sr25519), `RelayChain`

## Files

- `lib.rs` — hand-written chain definition and trait implementations
- `codegen_runtime.rs` — **auto-generated from on-chain metadata, never edit manually**. Regenerate with `tools/runtime-codegen/`.
