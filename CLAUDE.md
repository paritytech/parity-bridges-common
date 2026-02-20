# Parity Bridges Common

Cross-chain bridge relay infrastructure for Substrate-based blockchains. The `substrate-relay` binary relays GRANDPA finality proofs, parachain heads, and messages between chains via BridgeHubs.

Production bridges: Kusama-Polkadot, Polkadot-Bulletin. Testnet bridges: Rococo-Westend, Rococo-Bulletin.

## Commands

```bash
# Check (fast, use during development)
SKIP_WASM_BUILD=1 CFLAGS="-g0" cargo check --locked --workspace

# Test
SKIP_WASM_BUILD=1 CFLAGS="-g0" cargo test --workspace

# Single crate test
SKIP_WASM_BUILD=1 CFLAGS="-g0" cargo test -p substrate-relay

# Format (requires nightly)
cargo +nightly fmt --all

# Clippy
SKIP_WASM_BUILD=1 CFLAGS="-g0" cargo clippy --all-targets --all-features --locked --workspace

# Build release binary
CFLAGS="-g0" cargo build --release -p substrate-relay

# Generate docs
CFLAGS="-g0" cargo doc --no-deps --all --workspace --document-private-items
```

IMPORTANT: Always set `SKIP_WASM_BUILD=1` for check/test/clippy. Without it, the build attempts to compile WASM targets requiring a nightly toolchain with `wasm32-unknown-unknown`.

NOTE: `CFLAGS="-g0"` is needed on macOS to work around a `dsymutil` failure when `libz-sys` compiles its bundled zlib with debug symbols. Without it, all debug-profile builds fail.

## Project structure

```
substrate-relay/           # Main CLI binary — entry point and bridge implementations
  src/bridges/             # Per-bridge-pair relay logic (kusama_polkadot, rococo_westend, etc.)
  src/cli/                 # CLI subcommands (relay-headers-and-messages is the primary mode)
relay-clients/             # RPC client adapters, one crate per chain (11 crates)
  client-*/codegen_runtime.rs  # Auto-generated from on-chain metadata — do NOT edit by hand
chains/                    # Chain type definitions (4 crates: chain-kusama, chain-polkadot, etc.)
deployments/bridges/       # Grafana dashboards and alert rules per bridge pair
tools/runtime-codegen/     # Utility to regenerate codegen_runtime.rs from live chain metadata
```

## Code style

- Hard tabs, 100-char line width (see `rustfmt.toml`)
- Crate-level import granularity (`use crate::{Foo, Bar};` not individual imports)
- Clippy: `correctness` and `complexity` are denied; other lint groups are allowed
- Spellcheck runs via hunspell — custom dictionary at `.config/lingua.dic`

## Architecture

- **Cargo workspace** with member crates. Workspace-level dependency versions in root `Cargo.toml`.
- Heavy dependency on `polkadot-sdk` (git, master branch) for bridge primitives (`bp-*`), relay helpers (`relay-substrate-client`, `substrate-relay-helper`), and Substrate types (`sp-*`).
- Each relay client wraps `subxt` for RPC and uses `codegen_runtime.rs` generated from on-chain metadata. When a chain runtime upgrades, regenerate with the `runtime-codegen` tool.
- Bridge implementations in `substrate-relay/src/bridges/` wire together header, parachain, message, and confirmation relays for each bridge pair.

## Things that will bite you

- `codegen_runtime.rs` files are large auto-generated files — never modify manually. Use `tools/runtime-codegen/`.
- The relay has **version guards** that abort if the target chain runtime version is incompatible. When you see version-guard errors, the relay binary needs to be rebuilt against the new runtime metadata.
- License is GPL-3.0-only with Classpath exception. `cargo deny check licenses` enforces allowed dependency licenses strictly.
- CI runs in `paritytech/ci-unified` container image. The `test` and `build` jobs require large runners (`parity-large`).
- All releases come from `master`. Version is in `substrate-relay/Cargo.toml`. See `RELEASE.md` for the release process.
- Security issues go to https://security-submission.parity.io/ — never to public GitHub issues.
