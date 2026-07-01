#!/usr/bin/env bash
#
# (Re)generate the typed `subxt` runtime clients in this directory (`*.rs`) that the bridge
# zombienet-sdk tests consume as `crate::<chain>::{tx, storage, runtime_types, ..}`.
#
# subxt validates every generated call/storage access against the node's live metadata at test
# runtime, so these modules MUST match the runtimes embedded in the `polkadot` / `polkadot-parachain`
# binaries the tests run -- i.e. the polkadot-sdk revision this repo pins in `Cargo.lock`. A mismatch
# aborts the test with `Metadata error: The generated code is not compatible with the node`. So:
# regenerate whenever that pinned revision changes and commit the updated `*.rs`.
#
# What it does: read the pinned commit from `Cargo.lock`, shallow-checkout polkadot-sdk at exactly
# that commit, build the six `*-runtime` WASM blobs, build this repo's `runtime-codegen` tool, then
# run `runtime-codegen --full --from-wasm-file <wasm>` per runtime. `--full` emits the complete subxt
# client (`tx()`/`storage()`/`apis()`/`runtime_types`), unlike the relay-clients' `runtime_types`-only
# codegen; generating straight from the runtime WASM needs no extra crate injected into the checkout.
#
# Usage (run from anywhere):
#   generate.sh                       # clone @ pinned rev into ./.polkadot-sdk (kept for reuse) + generate
#   generate.sh --polkadot-sdk <path> # reuse an existing checkout (fast if its runtime WASM is built)
#   generate.sh --cleanup             # also remove the cloned ./.polkadot-sdk when done
#
# Requirements: git and the Rust toolchain pinned in RUST_TOOLCHAIN below, with its
# wasm32-unknown-unknown target (needed to build the runtimes).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../../.." && pwd)"
CODEGEN_OUT="${SCRIPT_DIR}"
SDK_REMOTE="https://github.com/paritytech/polkadot-sdk"
DEFAULT_SDK="${SCRIPT_DIR}/.polkadot-sdk"

usage() { sed -n '18,21p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; }

SDK_DIR=""
CLEANUP=0
while [ $# -gt 0 ]; do
	case "$1" in
		--polkadot-sdk) SDK_DIR="$2"; shift 2 ;;
		--cleanup) CLEANUP=1; shift ;;
		-h|--help) usage; exit 0 ;;
		*) echo "unknown argument: $1" >&2; usage; exit 1 ;;
	esac
done

# ============================================================================
# Rust toolchain used to build the polkadot-sdk runtimes.
#
# polkadot-sdk ships no rust-toolchain.toml, so we pin the build toolchain here. A too-new rustc
# fails to compile the runtimes (e.g. `#[no_mangle] cannot be used on internal language items` in
# sp-io).
#
# >>> UPDATE THIS whenever the pinned polkadot-sdk revision in Cargo.lock changes (see REV below),
#     to a rustc version that revision builds with. <<<
# ============================================================================
RUST_TOOLCHAIN="1.84.1"

# Chains to generate. Each maps to a `<chain>-runtime` cargo package and a
# `<chain_with_underscores>.rs` module in this directory (the names `tests/lib.rs` includes).
CHAINS=(rococo westend asset-hub-rococo asset-hub-westend bridge-hub-rococo bridge-hub-westend)

# 1. Pinned polkadot-sdk commit from Cargo.lock (shared with the CI workflow, see script header).
REV="$("${REPO_ROOT}/scripts/polkadot-sdk-rev.sh")"
echo ">> polkadot-sdk revision (from Cargo.lock): ${REV}"

# 2. Obtain a checkout at that exact commit.
CLONED=0
if [ -z "${SDK_DIR}" ]; then
	SDK_DIR="${DEFAULT_SDK}"
	mkdir -p "${SDK_DIR}"
	if [ ! -d "${SDK_DIR}/.git" ]; then
		echo ">> cloning ${SDK_REMOTE} @ ${REV} into ${SDK_DIR}"
		git -C "${SDK_DIR}" init -q
		git -C "${SDK_DIR}" remote add origin "${SDK_REMOTE}"
		CLONED=1
	fi
	echo ">> fetching ${REV} (shallow)"
	git -C "${SDK_DIR}" fetch --depth 1 origin "${REV}"
	git -C "${SDK_DIR}" checkout -q FETCH_HEAD
fi
echo ">> using polkadot-sdk checkout at: ${SDK_DIR}"

cleanup() {
	# The cloned polkadot-sdk checkout is kept for reuse by default; remove it only on --cleanup.
	if [ "${CLONED}" = "1" ] && [ "${CLEANUP}" = "1" ]; then
		echo ">> removing cloned checkout ${SDK_DIR}"
		rm -rf "${SDK_DIR}"
	fi
}
trap cleanup EXIT

# Toolchain check. polkadot-sdk ships no rust-toolchain.toml, so pin a compatible toolchain
# explicitly; a too-new rustc fails compiling the runtimes.
echo ">> using rust toolchain: ${RUST_TOOLCHAIN}"
rustup toolchain list | grep -q "^${RUST_TOOLCHAIN}" \
	|| { echo "ERROR: rust toolchain '${RUST_TOOLCHAIN}' not installed (rustup toolchain install ${RUST_TOOLCHAIN})" >&2; exit 1; }
# wasm-builder needs the wasm32 target for this toolchain to build the runtime blobs.
rustup target add --toolchain "${RUST_TOOLCHAIN}" wasm32-unknown-unknown >/dev/null 2>&1 || true

# 3. Build the six runtime WASM blobs (wasm-builder emits them under target/release/wbuild/).
echo ">> building runtime wasm (this is slow on a fresh checkout)"
PKGS=()
for chain in "${CHAINS[@]}"; do PKGS+=(-p "${chain}-runtime"); done
( cd "${SDK_DIR}" && RUSTUP_TOOLCHAIN="${RUST_TOOLCHAIN}" CARGO_NET_GIT_FETCH_WITH_CLI=true \
	cargo build --release "${PKGS[@]}" )

# 4. Build this repo's runtime-codegen tool (uses subxt-codegen to emit the typed client).
echo ">> building runtime-codegen"
cargo build --release --manifest-path "${REPO_ROOT}/tools/runtime-codegen/Cargo.toml"
TOOL="${REPO_ROOT}/tools/runtime-codegen/target/release/runtime-codegen"

# 5. Generate one full subxt module per chain.
mkdir -p "${CODEGEN_OUT}"
for chain in "${CHAINS[@]}"; do
	runtime="${chain}-runtime"
	wasm_name="$(echo "${runtime}" | tr - _).wasm"
	wasm="${SDK_DIR}/target/release/wbuild/${runtime}/${wasm_name}"
	module="$(echo "${chain}" | tr - _)"
	out="${CODEGEN_OUT}/${module}.rs"
	[ -f "${wasm}" ] || { echo "ERROR: runtime wasm not found: ${wasm}" >&2; exit 1; }
	echo ">> generating ${module}.rs from ${wasm_name}"
	"${TOOL}" --full --from-wasm-file "${wasm}" > "${out}"
done

# 6. Format to the repo style (hard tabs). `runtime-codegen` emits prettyplease (4-space) output, and
# `cargo fmt --all` skips these modules because `tests/lib.rs` gates them behind `#[cfg(zombie-ci)]`,
# so run nightly rustfmt directly on the generated files (CI runs `cargo +nightly fmt --all --check`).
echo ">> formatting generated modules (nightly rustfmt)"
rustup run nightly rustfmt --edition 2021 --config-path "${REPO_ROOT}/rustfmt.toml" "${CODEGEN_OUT}"/*.rs

echo ">> done. Generated modules in ${CODEGEN_OUT}:"
ls -la "${CODEGEN_OUT}"/*.rs
echo ">> Remember to re-run the tests (\`--features zombie-ci\`) against the regenerated modules."
