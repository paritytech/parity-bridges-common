---
name: release-prepare
description: Prepare a BridgeHub runtime upgrade for substrate-relay
metadata:
  argument-hint: "<BridgeHubKusama|BridgeHubPolkadot> <runtime-version>"
---

# Release Prepare

Prepare the workspace for a BridgeHub runtime upgrade by running the automated script and verifying the result.

## Usage

```
/release-prepare BridgeHubKusama v1.1.6
/release-prepare BridgeHubPolkadot v1.4.1
```

`$ARGUMENTS` must contain two values: the chain name and the Fellows runtimes release version (vX.Y.Z).

## Procedure

### Step 1: Validate Arguments

Parse `$ARGUMENTS` into `CHAIN` (first word) and `VERSION` (second word).

Valid chains: `BridgeHubKusama`, `BridgeHubPolkadot`.
Version must match `vX.Y.Z`.

If arguments are missing or invalid, explain the usage and ask the user.

### Step 2: Record Current State

Before making changes, read the current values for comparison:

```bash
grep 'spec_version' relay-clients/client-bridge-hub-kusama/src/lib.rs
grep 'spec_version' relay-clients/client-bridge-hub-polkadot/src/lib.rs
grep '^version' substrate-relay/Cargo.toml
```

### Step 3: Run the Release Script

Execute the preparation script:

```bash
./scripts/prepare-bridge-hub-release.sh $CHAIN $VERSION
```

This script will:
1. Download the WASM runtime from polkadot-fellows/runtimes
2. Update `spec_version` in the relay-client `lib.rs`
3. Regenerate `codegen_runtime.rs` using runtime-codegen
4. Apply the BlakeTwo256 workaround and format
5. Bump the substrate-relay patch version

If the script fails, report the error and suggest remediation.

### Step 4: Verify Build

Run workspace checks to confirm the changes compile:

```bash
cargo check --workspace
```

If it fails, inspect the errors. Common issues:
- Missing type imports in codegen (BlakeTwo256 fix may need adjustment)
- Version mismatch between runtime and expected types

### Step 5: Report Results

Summarize the changes:

- **Chain**: which chain was updated
- **Spec version**: old → new
- **Relay version**: old → new
- **Files modified**: list from git diff
- **Build status**: pass/fail

Suggest next step: run `/release-pr` to create the release PR.
