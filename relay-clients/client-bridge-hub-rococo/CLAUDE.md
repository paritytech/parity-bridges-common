# client-bridge-hub-rococo

Relay client for the **BridgeHub-Rococo** testnet parachain.

## Chain type

Parachain (no GRANDPA). Handles cross-chain message passing for the Rococo side. Supports multiple bridges (Westend and Bulletin).

## Traits implemented

`Chain`, `ChainWithMessages`, `ChainWithBalances`, `ChainWithTransactions` (sr25519), `ChainWithUtilityPallet`

## Transaction extensions

Uses `BridgeRejectObsoleteHeadersAndMessages`, `RefundBridgedParachainMessagesSchema`, and `CheckMetadataHash`.

## Files

- `lib.rs` — hand-written chain definition and trait implementations
- `codegen_runtime.rs` — **auto-generated from on-chain metadata, never edit manually**. Regenerate with `tools/runtime-codegen/`.
