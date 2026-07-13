# Bridges Tests for Local Rococo <> Westend and Kusama <> Polkadot Bridges

[`zombienet-sdk`](https://github.com/paritytech/zombienet-sdk) based integration tests for the local
Rococo <> Westend and Kusama <> Polkadot bridges. They spawn both relay-chain networks together with
their Bridge Hub and Asset Hub parachains, drive the `substrate-relay` binary as subprocesses, and
assert on-chain state via `subxt`.

Tests:

- `rococo_westend::asset_transfer` — transfers assets across the bridge (both directions, native and
  wrapped) and checks they arrive while the free-header relayers' balances stay constant;
- `rococo_westend::free_headers` — checks that free relay-chain and parachain headers are synced to
  the remote Bridge Hub while the relayer is otherwise idle;
- `kusama_polkadot::asset_transfer` — the Kusama <> Polkadot equivalent of the asset-transfer test,
  migrated from the manual shell test in `polkadot-fellows-runtimes`
  (`integration-tests/bridges/tests/0001-polkadot-kusama-asset-transfer`).

The code is organized so a bridge pair is a self-contained sibling module on top of shared, generic
infrastructure:

- `tests/common/` — reusable, bridge-pair-agnostic infrastructure: generic `subxt` helpers
  (`utils.rs`), the per-runtime typed-operation macros (`ops.rs`), the `substrate-relay` subprocess
  driver (`relayer.rs`) and the default node images (`images.rs`);
- `tests/rococo_westend/` — everything specific to the Rococo <> Westend pair: shared constants and
  bridge-relayer reward queries (`mod.rs`), the network spawning + bridge bootstrap environment
  (`environment.rs`) and the tests themselves (`asset_transfer.rs`, `free_headers.rs`);
- `tests/kusama_polkadot/` — the Kusama <> Polkadot pair, built on the same shared infrastructure.

The per-runtime typed-operation macros (`asset_hub_ops!` / `bridge_hub_ops!`) are shared: each pair
invokes them from its `mod.rs`, passing the remote consensus `NetworkId` (Rococo/Westend identify
each other by `NetworkId::ByGenesis(..)`, Polkadot/Kusama by the named `NetworkId::Polkadot` /
`NetworkId::Kusama` variants).

## Rococo <> Westend vs Kusama <> Polkadot

The two pairs differ because the Polkadot/Kusama runtimes live in
[`polkadot-fellows-runtimes`](https://github.com/polkadot-fellows/runtimes), not in polkadot-sdk, and
have **no `sudo` pallet**:

- **Chain specs / binaries.** Rococo/Westend run from polkadot-sdk (docker images or `PATH`
  binaries). Kusama/Polkadot are spawned with the **native** provider only, using the fellows
  `chain-spec-generator` (one per relay) plus the polkadot-sdk `polkadot` / `polkadot-parachain`
  binaries.
- **Bridge bootstrap.** Rococo/Westend configure the bridge via `sudo`-signed governance. Kusama/
  Polkadot configure it **entirely from genesis** (HRMP channels, remote XCM version, bridge-GRANDPA
  owner `//Alice`, and funded sovereign/reward accounts), leaving only the non-`sudo`, `//Bob`-signed
  asset-conversion pools to be created after spawn.

## Prerequisites

- Build `polkadot` and `polkadot-parachain` from a [`polkadot-sdk`](https://github.com/paritytech/polkadot-sdk)
  checkout **at the revision pinned in this repo's `Cargo.lock`**, and put them on your `PATH`:

  ```bash
  # The `polkadot` package produces three binaries: `polkadot`, `polkadot-prepare-worker` and
  # `polkadot-execute-worker`. A validator spawns the two PVF workers as child processes and refuses
  # to start without them, so all three must stay together.
  cargo build --release -p polkadot --features fast-runtime
  cargo build --release -p polkadot-parachain-bin
  export PATH="$PWD/target/release:$PATH"
  ```

  > If you instead copy the binaries into a directory (e.g. `~/local_bridge_testing/bin`), copy **all
  > four** — `polkadot`, `polkadot-prepare-worker`, `polkadot-execute-worker` and `polkadot-parachain`.
  > Copying only `polkadot` makes the validator fail at startup (it can't find its workers), surfacing
  > as a zombienet `Timeout … waiting for metric process_start_time_seconds` error.

- Build `substrate-relay` and point `SUBSTRATE_RELAY_BINARY` at it (it defaults to
  `~/local_bridge_testing/bin/substrate-relay`):

  ```bash
  cargo build --release -p substrate-relay
  export SUBSTRATE_RELAY_BINARY="$PWD/target/release/substrate-relay"
  ```

- **Kusama <> Polkadot only:** build the fellows `chain-spec-generator` (one binary per relay) from a
  [`polkadot-fellows-runtimes`](https://github.com/polkadot-fellows/runtimes) checkout and point the
  environment at them (they default to `~/local_bridge_testing/bin/chain-spec-generator-{polkadot,kusama}`):

  ```bash
  # In the fellows checkout, built with the fast-runtime feature for short local sessions:
  cargo build --release -p chain-spec-generator --no-default-features \
    --features fast-runtime,polkadot,kusama,asset-hub-polkadot,asset-hub-kusama,bridge-hub-polkadot,bridge-hub-kusama
  cp target/release/chain-spec-generator ~/local_bridge_testing/bin/chain-spec-generator-polkadot
  cp target/release/chain-spec-generator ~/local_bridge_testing/bin/chain-spec-generator-kusama
  # Override with CHAIN_SPEC_GEN_BINARY_FOR_POLKADOT / CHAIN_SPEC_GEN_BINARY_FOR_KUSAMA and the node
  # binaries with POLKADOT_BINARY / POLKADOT_PARACHAIN_BINARY if not on the default paths.
  ```

## Running

The tests are gated behind the `zombie-ci` feature, so a plain `cargo check`/`cargo build` of the
workspace neither compiles them nor pulls in their (otherwise optional) `zombienet-*` / `subxt`
dependencies.

Pick how nodes are spawned via `ZOMBIE_PROVIDER`:

- `docker` (default): pulls the `paritypr/*-debug` images tagged with the polkadot-sdk revision
  pinned in `Cargo.lock` (the defaults in `tests/common/images.rs`, set by `build.rs`; CI sets the
  same tag — see [`.github/workflows/zombienet.yml`](../../.github/workflows/zombienet.yml)).
  Override with `POLKADOT_IMAGE` / `CUMULUS_IMAGE`.
- `native`: runs the `polkadot` / `polkadot-parachain` binaries from your `PATH` (image vars ignored):

  ```bash
  export ZOMBIE_PROVIDER=native
  ```

The Kusama <> Polkadot tests are **native-only** (they need the fellows `chain-spec-generator`), so
run them with `ZOMBIE_PROVIDER=native`.

Run one test, or all of them:

```bash
cargo test -p bridges-zombienet-sdk-tests --features zombie-ci rococo_westend  -- --nocapture
ZOMBIE_PROVIDER=native \
  cargo test -p bridges-zombienet-sdk-tests --features zombie-ci kusama_polkadot -- --nocapture
```

On success a test exits `0`; on failure it prints the node and relayer logs.

## Runtime modules (`tests/codegen/`)

`tests/codegen/*.rs` are the typed `subxt` clients for the runtimes the tests talk to — Rococo,
Westend, Polkadot, Kusama, and their Asset Hub (`asset-hub-*-local`) and Bridge Hub
(`bridge-hub-*-local`) system parachains. `tests/lib.rs` loads each with `#[path]` and re-exports its
`api` module under the chain name, so call sites use `crate::<chain>::{tx, storage, runtime_types, ..}`.

At runtime `subxt` validates the generated calls against each node's metadata, so these modules must
match the runtimes in the `polkadot` / `polkadot-parachain` binaries you run — i.e. the polkadot-sdk
revision pinned in `Cargo.lock`. A mismatch aborts the test with
`Metadata error: The generated code is not compatible with the node`.

## Maintenance

- **Regenerate the codegen when the pinned polkadot-sdk revision changes** (a new commit hash for
  `git+https://github.com/paritytech/polkadot-sdk` in `Cargo.lock`). `scripts/generate-codegen.sh`
  builds the six runtimes from a matching polkadot-sdk checkout and rewrites `tests/codegen/*.rs`
  (see its header for all options):

  ```bash
  scripts/generate-codegen.sh --target zombienet                       # clone @ pinned rev + generate
  scripts/generate-codegen.sh --target zombienet --polkadot-sdk <path> # reuse an existing checkout
  scripts/generate-codegen.sh --target zombienet --polkadot-sdk-hash <sha> # build a specific commit
  ```

  Commit the updated `tests/codegen/*.rs`.

- **Regenerate the Kusama <> Polkadot codegen from the fellows runtime WASM.** These six runtimes are
  not in polkadot-sdk, so build their WASM from a `polkadot-fellows-runtimes` checkout and feed the
  blobs to the same script via `--wasm-dir` (the `staging-kusama-runtime` blob must be renamed to
  `kusama_runtime*.wasm` so the script's per-chain matcher finds it):

  ```bash
  # In the fellows checkout:
  cargo build --release -p polkadot-runtime -p staging-kusama-runtime \
    -p asset-hub-polkadot-runtime -p asset-hub-kusama-runtime \
    -p bridge-hub-polkadot-runtime -p bridge-hub-kusama-runtime
  mkdir -p /tmp/pk-wasm && for c in polkadot asset-hub-polkadot asset-hub-kusama \
    bridge-hub-polkadot bridge-hub-kusama; do
    cp target/release/wbuild/$c-runtime/${c//-/_}_runtime.compact.compressed.wasm /tmp/pk-wasm/
  done
  cp target/release/wbuild/staging-kusama-runtime/staging_kusama_runtime.compact.compressed.wasm \
    /tmp/pk-wasm/kusama_runtime.compact.compressed.wasm

  # Back in this repo:
  scripts/generate-codegen.sh --target zombienet --wasm-dir /tmp/pk-wasm \
    --chains "polkadot kusama asset-hub-polkadot asset-hub-kusama bridge-hub-polkadot bridge-hub-kusama"
  ```

  Commit the updated `tests/codegen/*.rs`.

- **`subxt`/`subxt-signer` are pinned to the version `zombienet-sdk` uses** (workspace `Cargo.toml`),
  so `node.wait_client()` returns a client of the type the tests use. When bumping `zombienet-sdk`,
  realign the `subxt` version to match.
