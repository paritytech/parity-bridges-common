# client-polkadot-bulletin

Relay client for the **Polkadot Bulletin** chain.

## Chain type

Standalone chain with GRANDPA finality (not a parachain, not a standard relay chain). Has message support but no balances.

## Traits implemented

`Chain`, `ChainWithGrandpa`, `ChainWithMessages`, `ChainWithBalances` (stub — no balances on this chain), `ChainWithTransactions` (sr25519)

## Files

- `lib.rs` — hand-written chain definition and trait implementations
- `codegen_runtime.rs` — **auto-generated from on-chain metadata, never edit manually**. Regenerate with `tools/runtime-codegen/`.
