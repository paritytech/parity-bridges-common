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

From the `tools/runtime-codegen/` directory:

```bash
cd tools/runtime-codegen
cargo run --bin runtime-codegen --release -- --from-wasm-file wbuild/bridge_hub_rococo_runtime.compact.compressed.wasm 1> ../../relay-clients/client-bridge-hub-rococo/src/codegen_runtime.rs 2>/dev/null
cargo run --bin runtime-codegen --release -- --from-wasm-file wbuild/bridge_hub_westend_runtime.compact.compressed.wasm 1> ../../relay-clients/client-bridge-hub-westend/src/codegen_runtime.rs 2>/dev/null
```

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
