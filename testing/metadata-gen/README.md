# Regenerating the zombienet-sdk metadata files

`testing/zombienet-sdk-tests/metadata-files/*.scale` are the subxt metadata for the six runtimes the
bridge tests talk to (Rococo, Westend, and their Asset Hub / Bridge Hub system parachains). subxt
validates the generated calls against each node's live runtime metadata, so these files **must match
the runtime version embedded in the `polkadot` / `polkadot-parachain` binaries you run** — which is
the polkadot-sdk revision this repo pins in `Cargo.lock`.

This repo builds no runtimes of its own, so the files are generated from a polkadot-sdk checkout.
`generate.sh` makes that self-contained: it reads the pinned commit from `Cargo.lock`, checks out
polkadot-sdk at exactly that commit, builds the six runtimes, extracts their metadata, copies the
`.scale` files into `testing/zombienet-sdk-tests/metadata-files/`, and cleans up.

## Usage

From the repo root:

```bash
# clone polkadot-sdk @ pinned-rev into testing/metadata-gen/polkadot-sdk, generate, then remove it:
testing/metadata-gen/generate.sh

# reuse an existing polkadot-sdk checkout (faster; not modified permanently, not deleted):
testing/metadata-gen/generate.sh --polkadot-sdk /path/to/polkadot-sdk

# keep the cloned checkout for next time (skips re-clone, still rebuilds at the pinned rev):
testing/metadata-gen/generate.sh --keep
```

The first run is **slow** — it builds six runtime WASM blobs from a fresh checkout. `--polkadot-sdk`
pointing at a checkout that already has the runtimes built is much faster.

## How it works

- `runner/` is a tiny crate whose `build.rs` (under the `zombie-metadata` feature) locates each
  `*-runtime` WASM (building it on demand) and extracts metadata via `sc-executor` /
  `sc-runtime-utilities`. Its deps use `workspace = true`, so it only builds **inside** a polkadot-sdk
  checkout — `generate.sh` copies it into the checkout's `bridges/testing/`, registers it as a
  workspace member, builds it, then removes it and reverts the checkout's `Cargo.toml`.
- This injection approach guarantees the extraction crate uses the checkout's own substrate crates at
  the exact pinned revision, avoiding any version skew (and avoiding pulling heavy substrate git deps
  into this repo's own workspace, which would force its shared polkadot-sdk pin onto a newer commit).

## When to regenerate

Whenever this repo's pinned polkadot-sdk revision changes (a different commit hash for
`git+https://github.com/paritytech/polkadot-sdk` in `Cargo.lock`), re-run `generate.sh` and commit the
updated `.scale` files.
