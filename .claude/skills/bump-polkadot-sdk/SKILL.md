---
name: bump-polkadot-sdk
description: Bump polkadot-sdk master hash & regenerate BridgeHub Westend/Rococo codegen
allowed-tools: Bash Read Edit Write Grep Glob WebFetch
---

# Bump polkadot-sdk & Regenerate BridgeHub Codegen

Bump the polkadot-sdk master dependency hash in this repo and regenerate the BridgeHubWestend/BridgeHubRococo codegen runtime files.

## Prerequisites

- A local checkout of `polkadot-sdk` exists
- Rust nightly toolchain installed (for `cargo +nightly fmt`)

## Steps

### Step 0: Ask for polkadot-sdk location

Ask the user for the local polkadot-sdk repo path. Default to `../polkadot-sdk` (relative to this project root) if not specified.
Store this as `POLKADOT_SDK_PATH`.

Then checkout master and pull latest:

```bash
cd <POLKADOT_SDK_PATH>
git checkout master && git pull
```

### Step 1: Get latest polkadot-sdk master hash

Get the latest commit hash from the local polkadot-sdk master branch:

```bash
cd <POLKADOT_SDK_PATH>
git rev-parse HEAD
```

Store this as `NEW_HASH`.

### Step 2: Update Cargo.lock

Find the current polkadot-sdk hash in `Cargo.lock`:

```bash
grep -m1 -oP '(?<=polkadot-sdk\?branch=master#)[a-f0-9]+' Cargo.lock
```

Store this as `OLD_HASH`. Then replace all occurrences:

```bash
sed -i "s/${OLD_HASH}/${NEW_HASH}/g" Cargo.lock
```

Verify the replacement:

```bash
grep -c "${NEW_HASH}" Cargo.lock
```

### Step 3: Check spec_version updates

Fetch the `spec_version` from the polkadot-sdk repo for both runtimes and compare with local values.

#### BridgeHub Westend

Fetch upstream spec_version from:
`https://raw.githubusercontent.com/paritytech/polkadot-sdk/master/cumulus/parachains/runtimes/bridge-hubs/bridge-hub-westend/src/lib.rs`

Look for the `spec_version:` field inside the `RuntimeVersion` definition.

Compare with the local value in `relay-clients/client-bridge-hub-westend/src/lib.rs` in the `ChainWithRuntimeVersion` impl:
```rust
Some(SimpleRuntimeVersion { spec_version: XXXXXXX, transaction_version: Y })
```

If upstream has a newer version, update the local file.

#### BridgeHub Rococo

Fetch upstream spec_version from:
`https://raw.githubusercontent.com/paritytech/polkadot-sdk/master/cumulus/parachains/runtimes/bridge-hubs/bridge-hub-rococo/src/lib.rs`

Compare with local value in `relay-clients/client-bridge-hub-rococo/src/lib.rs`.

If upstream has a newer version, update the local file.

### Step 4: Build WASM runtimes from local polkadot-sdk

Using the `POLKADOT_SDK_PATH` from Step 0:

```bash
cd <POLKADOT_SDK_PATH>
cargo build --release -p bridge-hub-rococo-runtime
cargo build --release -p bridge-hub-westend-runtime
```

Once built, copy the WASM files to this project:

```bash
cp <POLKADOT_SDK_PATH>/target/release/wbuild/bridge-hub-rococo-runtime/bridge_hub_rococo_runtime.compact.compressed.wasm tools/runtime-codegen/wbuild/
cp <POLKADOT_SDK_PATH>/target/release/wbuild/bridge-hub-westend-runtime/bridge_hub_westend_runtime.compact.compressed.wasm tools/runtime-codegen/wbuild/
```

### Step 5: Regenerate codegen_runtime.rs

From the `tools/runtime-codegen/` directory. Capture stderr separately to check for warnings:

```bash
cd tools/runtime-codegen
cargo run --bin runtime-codegen --release -- --from-wasm-file wbuild/bridge_hub_rococo_runtime.compact.compressed.wasm 1> ../../relay-clients/client-bridge-hub-rococo/src/codegen_runtime.rs 2> /tmp/codegen-rococo-stderr.log
cargo run --bin runtime-codegen --release -- --from-wasm-file wbuild/bridge_hub_westend_runtime.compact.compressed.wasm 1> ../../relay-clients/client-bridge-hub-westend/src/codegen_runtime.rs 2> /tmp/codegen-westend-stderr.log
```

After codegen, check stderr logs for warnings and report them to the user:

```bash
grep -E "warning:|future-incompat" /tmp/codegen-rococo-stderr.log /tmp/codegen-westend-stderr.log
```

If any warnings are found, print them for the user and let them decide on further action.

### Step 6: Fix codegen and format

```bash
# First format pass
cargo +nightly fmt --all

# Fix the Header type in generated files (missing BlakeTwo256 generic)
find . -name codegen_runtime.rs -exec \
    sed -i 's/::sp_runtime::generic::Header<::core::primitive::u32>/::sp_runtime::generic::Header<::core::primitive::u32, ::sp_runtime::traits::BlakeTwo256>/g' {} +

# Second format pass
cargo +nightly fmt --all
```

### Step 7: Check, clippy, and resolve dependency mismatches

```bash
SKIP_WASM_BUILD=1 cargo check --workspace
SKIP_WASM_BUILD=1 cargo clippy --workspace --all-targets --all-features
```

If there are build errors due to dependency version mismatches in Cargo.lock, resolve them by aligning with polkadot-sdk's Cargo.lock:

1. Identify the failing crate and version from the build error.
2. Look up that crate's version in `<POLKADOT_SDK_PATH>/Cargo.lock` to find what version polkadot-sdk uses.
3. Compare with the version in this project's `Cargo.lock`. If they differ, update this project's `Cargo.toml` (root workspace or member crate) to match polkadot-sdk's version.
4. Run `cargo update -p <crate_name>` if needed, or manually edit `Cargo.lock` to align the version.
5. Re-run `SKIP_WASM_BUILD=1 cargo check --workspace` after each fix.

Common issues:
- Third-party crate version conflicts (e.g. `clap`, `syn`, `tokio`, `tempfile`) — match the version polkadot-sdk uses
- New/removed crate features — check polkadot-sdk's `Cargo.toml` for feature flag changes
- API changes in polkadot-sdk dependencies — may require code changes in relay crates
- Duplicate crate versions — ensure only one version is resolved by aligning with polkadot-sdk

### Step 8: Report results

Report to the user:
- Old hash vs new hash
- Whether spec_versions were updated (and from/to values)
- Build/clippy status
- Any errors that need manual intervention

---

## Bridges Tests: Local Rococo <-> Westend Bridge

Run the `0001-asset-transfer` bridge test locally using zombienet to verify the bump works end-to-end.

Reference: https://github.com/paritytech/polkadot-sdk/blob/master/bridges/testing/README.md

### Step 9: Check prerequisites

#### Node.js

Check if `node` is installed:

```bash
node --version
```

If not found, ask the user to install Node.js (https://nodejs.org/) before continuing.

Also ensure `@polkadot/api-cli` is installed:

```bash
npx --yes @polkadot/api-cli --version || yarn global add @polkadot/api-cli
```

#### Zombienet

Check if zombienet exists at `~/local_bridge_testing/bin/zombienet`:

```bash
ls ~/local_bridge_testing/bin/zombienet
```

If not found, ask the user to download the latest zombienet release from https://github.com/nickvdyck/zombienet-sdk/releases (or the appropriate zombienet releases page) and place the binary at `~/local_bridge_testing/bin/zombienet`:

```bash
mkdir -p ~/local_bridge_testing/bin
# User must download zombienet binary to ~/local_bridge_testing/bin/zombienet
chmod +x ~/local_bridge_testing/bin/zombienet
```

#### jq (required by test scripts)

```bash
jq --version || echo "jq not found — install with: sudo apt install jq (Linux) or brew install jq (macOS)"
```

### Step 10: Build required binaries

From the local polkadot-sdk repo (`<POLKADOT_SDK_PATH>`):

```bash
cd <POLKADOT_SDK_PATH>
cargo build --release -p polkadot --features fast-runtime
cargo build --release -p polkadot-parachain-bin
```

These produce:
- `<POLKADOT_SDK_PATH>/target/release/polkadot`
- `<POLKADOT_SDK_PATH>/target/release/polkadot-parachain`

Build substrate-relay from this project (parity-bridges-common):

```bash
cd <BRIDGES_COMMON_PATH>
cargo build --release -p substrate-relay
```

Copy the relay binary to the expected location:

```bash
mkdir -p ~/local_bridge_testing/bin
cp <BRIDGES_COMMON_PATH>/target/release/substrate-relay ~/local_bridge_testing/bin/substrate-relay
```

### Step 11: Run the asset transfer test

From the polkadot-sdk repo's bridges testing directory:

```bash
cd <POLKADOT_SDK_PATH>/bridges/testing
./run-test.sh 0001-asset-transfer
```

The script auto-detects local mode and expects binaries at:
- `<POLKADOT_SDK_PATH>/target/release/polkadot`
- `<POLKADOT_SDK_PATH>/target/release/polkadot-parachain`
- `~/local_bridge_testing/bin/zombienet`
- `~/local_bridge_testing/bin/substrate-relay`

### Step 12: Report Rococo <-> Westend test results

If the test passes, report success to the user.

If the test fails, check the zombienet logs (paths are printed in the output) and report:
- Which step failed
- Relevant log snippets
- Whether the failure is related to the polkadot-sdk bump or a pre-existing issue

---

## Bridges Tests: Local Polkadot <-> Kusama Bridge

Run the `0001-polkadot-kusama-asset-transfer` bridge test locally to verify the Polkadot <-> Kusama bridge works end-to-end.

Reference: https://github.com/polkadot-fellows/runtimes/blob/main/integration-tests/bridges/README.md

### Prerequisites

This section requires a local clone of the `polkadot-fellows/runtimes` repo. Ask the user for its path (default: `../runtimes` relative to this project root). Store as `FELLOWS_RUNTIMES_PATH`.

The prerequisites from Steps 9-10 (Node.js, zombienet, jq) still apply. Ensure those are satisfied first.

### Step 13: Build additional binaries for Polkadot <-> Kusama

#### Polkadot binaries (without fast-runtime)

From `<POLKADOT_SDK_PATH>`:

```bash
cd <POLKADOT_SDK_PATH>
cargo build -p polkadot --release
cargo build --bin polkadot-prepare-worker --release
cargo build --bin polkadot-execute-worker --release
cargo build -p polkadot-parachain-bin --release
```

Copy all binaries to `~/local_bridge_testing/bin/`:

```bash
mkdir -p ~/local_bridge_testing/bin
cp <POLKADOT_SDK_PATH>/target/release/polkadot ~/local_bridge_testing/bin/polkadot
cp <POLKADOT_SDK_PATH>/target/release/polkadot-prepare-worker ~/local_bridge_testing/bin/polkadot-prepare-worker
cp <POLKADOT_SDK_PATH>/target/release/polkadot-execute-worker ~/local_bridge_testing/bin/polkadot-execute-worker
cp <POLKADOT_SDK_PATH>/target/release/polkadot-parachain ~/local_bridge_testing/bin/polkadot-parachain
```

#### substrate-relay

If not already built in Step 10, build and copy:

```bash
cd <BRIDGES_COMMON_PATH>
cargo build --release -p substrate-relay
cp <BRIDGES_COMMON_PATH>/target/release/substrate-relay ~/local_bridge_testing/bin/substrate-relay
```

#### chain-spec-generator (from polkadot-fellows/runtimes)

First, apply the sudo patch:

```bash
cd <FELLOWS_RUNTIMES_PATH>
git apply ./integration-tests/bridges/sudo-relay.patch
```

Then build the chain-spec-generator with the required features:

```bash
cd <FELLOWS_RUNTIMES_PATH>
cargo build --release -p chain-spec-generator --no-default-features --features fast-runtime,polkadot,kusama,bridge-hub-kusama,bridge-hub-polkadot,asset-hub-kusama,asset-hub-polkadot
```

Copy the binary twice (once for each chain):

```bash
cp <FELLOWS_RUNTIMES_PATH>/target/release/chain-spec-generator ~/local_bridge_testing/bin/chain-spec-generator-polkadot
cp <FELLOWS_RUNTIMES_PATH>/target/release/chain-spec-generator ~/local_bridge_testing/bin/chain-spec-generator-kusama
```

### Step 14: Run the Polkadot <-> Kusama asset transfer test

From the `polkadot-fellows/runtimes` repo's integration tests directory:

```bash
cd <FELLOWS_RUNTIMES_PATH>/integration-tests/bridges
FRAMEWORK_REPO_PATH=<POLKADOT_SDK_PATH> ./run-test.sh 0001-polkadot-kusama-asset-transfer
```

Setting `FRAMEWORK_REPO_PATH` tells the test script to use the local polkadot-sdk checkout instead of cloning it.

The test expects all binaries at `~/local_bridge_testing/bin/`:
- `polkadot`
- `polkadot-prepare-worker`
- `polkadot-execute-worker`
- `polkadot-parachain`
- `zombienet`
- `substrate-relay`
- `chain-spec-generator-polkadot`
- `chain-spec-generator-kusama`

### Step 15: Report Polkadot <-> Kusama test results

If the test passes, report success to the user.

If the test fails, check the zombienet logs (paths are printed in the output) and report:
- Which step failed
- Relevant log snippets
- Whether the failure is related to the polkadot-sdk bump, the fellows runtimes, or a pre-existing issue
