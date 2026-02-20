# client-bridge-hub-polkadot

Relay client for the **BridgeHub-Polkadot** parachain.

## Chain type

Parachain (no GRANDPA). Handles cross-chain message passing for the Polkadot side. Supports multiple bridges (Kusama and Bulletin).

## Traits implemented

`Chain`, `ChainWithMessages`, `ChainWithBalances`, `ChainWithTransactions` (sr25519), `ChainWithUtilityPallet`

## Transaction extensions

Uses `BridgeRejectObsoleteHeadersAndMessages`, `RefundBridgedParachainMessagesSchema`, and `CheckMetadataHash`.

## Files

- `lib.rs` — hand-written chain definition and trait implementations
- `codegen_runtime.rs` — **auto-generated from on-chain metadata, never edit manually**. Regenerate with `tools/runtime-codegen/`.
