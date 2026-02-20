# client-asset-hub-westend

Relay client for the **AssetHub-Westend** testnet parachain.

## Chain type

Parachain (no GRANDPA). Simpler than BridgeHub — handles asset-related message passing.

## Traits implemented

`Chain`, `ChainWithMessages`, `ChainWithBalances`, `ChainWithTransactions` (sr25519)

Note: does **not** implement `ChainWithUtilityPallet` (unlike BridgeHub crates).

## Files

- `lib.rs` — hand-written chain definition and trait implementations
- `codegen_runtime.rs` — **auto-generated from on-chain metadata, never edit manually**. Regenerate with `tools/runtime-codegen/`.
