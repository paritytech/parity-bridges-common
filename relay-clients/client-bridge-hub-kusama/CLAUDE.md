# client-bridge-hub-kusama

Relay client for the **BridgeHub-Kusama** parachain.

## Chain type

Parachain (no GRANDPA). Handles cross-chain message passing for the Kusama side.

## Traits implemented

`Chain`, `ChainWithMessages`, `ChainWithBalances`, `ChainWithTransactions` (sr25519), `ChainWithUtilityPallet`

## Transaction extensions

Uses `BridgeRejectObsoleteHeadersAndMessages`, `RefundBridgedParachainMessagesSchema`, and `CheckMetadataHash`.

## Files

- `lib.rs` — hand-written chain definition and trait implementations
- `codegen_runtime.rs` — **auto-generated from on-chain metadata, never edit manually**. Regenerate with `tools/runtime-codegen/`.
