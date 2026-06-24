# Bridges Tests for Local Rococo <> Westend Bridge

This crate contains [`zombienet-sdk`](https://github.com/paritytech/zombienet-sdk) based integration
tests for both onchain and offchain bridges code, for the local Rococo <> Westend bridge (mirroring
`polkadot/zombienet-sdk-tests`). They spawn both relay-chain networks together with their Bridge Hub
and Asset Hub parachains, drive the `substrate-relay` binary as subprocesses and assert on-chain state
via `subxt`.

The available tests are:

- `asset_transfer` — transfers assets across the bridge (both directions, native and wrapped) and
  checks they arrive while the free-header relayers' balances stay constant;
- `free_headers` — checks that free relay-chain and parachain headers are synced to the remote Bridge
  Hub while the relayer is otherwise idle.

The shared environment (network spawning, bridge initialization, the `substrate-relay` driver and the
`subxt` query/extrinsic helpers) lives in `tests/environment.rs`.

## Prerequisites for running the tests locally

- build the `polkadot` and `polkadot-parachain` binaries by running the following commands in a
  [`polkadot-sdk`](https://github.com/paritytech/polkadot-sdk) repository clone, and put them on your
  `PATH`:

  ```bash
  # The `polkadot` package produces THREE binaries: `polkadot`, `polkadot-prepare-worker` and
  # `polkadot-execute-worker`. A `polkadot` validator spawns the two PVF workers as child processes
  # and refuses to start without them, so all three must stay together.
  cargo build --release -p polkadot --features fast-runtime
  cargo build --release -p polkadot-parachain-bin
  export PATH="$PWD/target/release:$PATH"
  ```

  > If you copy the binaries into a separate directory (e.g. `~/local_bridge_testing/bin`) instead of
  > adding `target/release` to your `PATH`, copy **all four** — `polkadot`, `polkadot-prepare-worker`,
  > `polkadot-execute-worker` and `polkadot-parachain`. Copying only `polkadot` makes the validator
  > fail at startup (it can't find its workers), which surfaces as a zombienet
  > `Timeout … waiting for metric process_start_time_seconds` error.

- build the `substrate-relay` binary by running `cargo build --release -p substrate-relay` in this
  repository, and point `SUBSTRATE_RELAY_BINARY` at it (it defaults to
  `~/local_bridge_testing/bin/substrate-relay`):

  ```bash
  cargo build --release -p substrate-relay
  export SUBSTRATE_RELAY_BINARY="$PWD/target/release/substrate-relay"
  ```

## Running

The tests are gated behind the `zombie-ci` feature so that a plain `cargo check`/`cargo test` of the
workspace stays cheap. The committed `metadata-files/*.scale` blobs are used by the `subxt` codegen,
so the tests run without any extra setup.

Select the **native** zombienet provider so the tests spawn the local `polkadot` / `polkadot-parachain`
binaries on your `PATH` directly, instead of containers (Docker/Podman) or Kubernetes:

```bash
export ZOMBIE_PROVIDER=native
```

> `ZOMBIE_PROVIDER` defaults to `docker` when unset, which spawns the nodes in containers (Podman/Docker)
> and pulls `docker.io/parity/*` images. Set it to `native` to use your local binaries. (`k8s` is the
> other option.)

Then run a single test with:

```bash
cargo test -p bridges-zombienet-sdk-tests --features zombie-ci asset_transfer -- --nocapture
cargo test -p bridges-zombienet-sdk-tests --features zombie-ci free_headers -- --nocapture
```

or all of them with:

```bash
cargo test -p bridges-zombienet-sdk-tests --features zombie-ci -- --nocapture
```

On success the test exits `0`; on failure it prints the relay/parachain node and relayer logs, which
can be used to track the state of all spawned nodes.

## Metadata files

`metadata-files/*.scale` are the SCALE-encoded `subxt` metadata for the six runtimes involved — Rococo,
Westend, and their Asset Hub (`asset-hub-*-local`) and Bridge Hub (`bridge-hub-*-local`) system
parachains. The `#[subxt::subxt(...)]` codegen in `tests/lib.rs` reads them at compile time, and at
runtime `subxt` validates the generated calls against each node's live runtime metadata. They must
therefore **match the runtimes embedded in the `polkadot` / `polkadot-parachain` binaries you run** —
otherwise the tests abort with `Metadata error: The generated code is not compatible with the node`.

### How to (re)generate them

This repository builds no runtimes of its own, so the metadata is extracted from a `polkadot-sdk`
checkout. The [`testing/metadata-gen`](../metadata-gen) helper makes this a single command — it reads
the pinned polkadot-sdk revision from this repo's `Cargo.lock`, checks polkadot-sdk out at exactly
that commit, builds the six `*-runtime` WASM blobs, extracts their metadata, copies the `.scale` files
here, and cleans up. Run it from the repo root:

```bash
# clone polkadot-sdk @ the pinned revision, generate, then remove the clone (slow on first run —
# it builds six runtime WASM blobs from scratch):
testing/metadata-gen/generate.sh

# faster: reuse an existing polkadot-sdk checkout (nothing extra is built if its runtime WASM already
# exists; the checkout is left untouched and is not deleted):
testing/metadata-gen/generate.sh --polkadot-sdk /path/to/polkadot-sdk

# keep the cloned checkout under testing/metadata-gen/polkadot-sdk for next time:
testing/metadata-gen/generate.sh --keep
```

Regenerate (and commit the updated `.scale` files) whenever this repo's pinned polkadot-sdk revision
changes. See [`testing/metadata-gen/README.md`](../metadata-gen/README.md) for how it works.
