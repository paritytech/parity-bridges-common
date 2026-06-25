# Bridges Tests for Local Rococo <> Westend Bridge

[`zombienet-sdk`](https://github.com/paritytech/zombienet-sdk) based integration tests for the local
Rococo <> Westend bridge. They spawn both relay-chain networks together with their Bridge Hub and
Asset Hub parachains, drive the `substrate-relay` binary as subprocesses, and assert on-chain state
via `subxt`.

Tests:

- `asset_transfer` — transfers assets across the bridge (both directions, native and wrapped) and
  checks they arrive while the free-header relayers' balances stay constant;
- `free_headers` — checks that free relay-chain and parachain headers are synced to the remote Bridge
  Hub while the relayer is otherwise idle.

The shared environment (network spawning, bridge initialization, the `substrate-relay` driver and the
`subxt` query/extrinsic helpers) lives in `tests/environment.rs`.

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

## Running

The tests are gated behind the `zombie-ci` feature, so a plain `cargo check`/`cargo build` of the
workspace neither compiles them nor pulls in their (otherwise optional) `zombienet-*` / `subxt`
dependencies.

Pick how nodes are spawned via `ZOMBIE_PROVIDER` (defaults to `docker`, which pulls the
`--features fast-runtime` `docker.io/paritypr/*-debug` images pinned in `tests/environment.rs` —
override with the `POLKADOT_IMAGE` / `CUMULUS_IMAGE` env vars); use `native` to run the local
`polkadot` / `polkadot-parachain` binaries on your `PATH`:

```bash
export ZOMBIE_PROVIDER=native
```

Run one test, or all of them:

```bash
cargo test -p bridges-zombienet-sdk-tests --features zombie-ci asset_transfer -- --nocapture
cargo test -p bridges-zombienet-sdk-tests --features zombie-ci free_headers   -- --nocapture
cargo test -p bridges-zombienet-sdk-tests --features zombie-ci                -- --nocapture
```

On success a test exits `0`; on failure it prints the node and relayer logs.

## Runtime modules (`tests/codegen/`)

`tests/codegen/*.rs` are the typed `subxt` clients for the six runtimes the tests talk to — Rococo,
Westend, and their Asset Hub (`asset-hub-*-local`) and Bridge Hub (`bridge-hub-*-local`) system
parachains. `tests/lib.rs` loads each with `#[path]` and re-exports its `api` module under the chain
name, so call sites use `crate::<chain>::{tx, storage, runtime_types, ..}`.

At runtime `subxt` validates the generated calls against each node's metadata, so these modules must
match the runtimes in the `polkadot` / `polkadot-parachain` binaries you run — i.e. the polkadot-sdk
revision pinned in `Cargo.lock`. A mismatch aborts the test with
`Metadata error: The generated code is not compatible with the node`.

## Maintenance

- **Regenerate the codegen when the pinned polkadot-sdk revision changes** (a new commit hash for
  `git+https://github.com/paritytech/polkadot-sdk` in `Cargo.lock`). One command builds the six
  runtimes from a matching polkadot-sdk checkout and rewrites `tests/codegen/*.rs`:

  ```bash
  testing/metadata-gen/generate.sh                       # clone @ pinned rev (kept for reuse) + generate
  testing/metadata-gen/generate.sh --polkadot-sdk <path> # reuse an existing checkout
  testing/metadata-gen/generate.sh --cleanup             # remove the cloned checkout when done
  ```

  Commit the updated `tests/codegen/*.rs`. See [`testing/metadata-gen/README.md`](../metadata-gen/README.md)
  for how it works.

- **`subxt`/`subxt-signer` are pinned to the version `zombienet-sdk` uses** (workspace `Cargo.toml`),
  so `node.wait_client()` returns a client of the type the tests use. When bumping `zombienet-sdk`,
  realign the `subxt` version to match.
