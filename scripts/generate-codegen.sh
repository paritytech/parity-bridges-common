#!/usr/bin/env bash
#
# (Re)generate subxt runtime codegens for this repo from runtime WASM, for either target:
#
#   --target zombienet      testing/zombienet-sdk-tests/tests/codegen/<chain>.rs   (subxt `--full`)
#   --target relay-clients  relay-clients/client-<chain>/src/codegen_runtime.rs    (`runtime_types`-only)
#
# `--full` emits the complete subxt client (tx()/storage()/apis()); the relay-clients default emits
# `runtime_types`-only with the custom derives and `bp_*`/`sp_*` type substitutes the relay machinery
# relies on (and the `Header<u32>` -> `Header<u32, BlakeTwo256>` fixup for a known subxt bug). The
# target picks a sensible default mode; override with --full / --types-only.
#
# The runtime WASM comes from one of:
#   * a polkadot-sdk checkout at a pinned commit, built here (default; commit from Cargo.lock);
#   * --wasm-dir <dir>  : a directory of prebuilt blobs, matched per chain by `<chain>_runtime*.wasm`
#                         (e.g. Fellows release runtimes: `bridge-hub-polkadot_runtime-vX.compact...`);
#   * --wasm-file <path>: one explicit blob (requires a single --chains entry; used by release.yml).
#
# subxt validates generated calls against the node's live metadata at runtime, so a codegen must
# match the runtime it is used against: the zombienet nodes run the polkadot-sdk revision pinned in
# Cargo.lock, and the relayer talks to production chains. Regenerate + commit when those move.
#
# Usage:
#   scripts/generate-codegen.sh --target zombienet
#   scripts/generate-codegen.sh --target relay-clients                 # rococo/westend family, types-only
#   scripts/generate-codegen.sh --target zombienet --polkadot-sdk-hash <sha>
#   scripts/generate-codegen.sh --target zombienet --polkadot-sdk <path/to/checkout>
#   scripts/generate-codegen.sh --target relay-clients --chains "rococo westend"
#   scripts/generate-codegen.sh --target zombienet --wasm-dir <dir-of-*.wasm>          # skip clone+build
#   scripts/generate-codegen.sh --target relay-clients --chains bridge-hub-polkadot \
#                               --wasm-file ./tools/runtime-codegen/wbuild/bridge-hub-polkadot_runtime-vX.compact.compressed.wasm
#
# Options:
#   --target <zombienet|relay-clients>   (required) where to write and default mode/chains
#   --full | --types-only                override the target's default subxt mode
#   --chains "<a b c>"                   override the chain list (space-separated, dashed names)
#   --polkadot-sdk-hash <sha>            build WASM from this polkadot-sdk commit (default: from Cargo.lock)
#   --polkadot-sdk <path>                reuse an existing polkadot-sdk checkout instead of cloning
#   --wasm-dir <dir>                     use prebuilt `<chain>_runtime*.wasm` from <dir>; skip clone+build
#   --wasm-file <path>                   use one explicit blob (single --chains entry); skip clone+build
#   --cleanup                            remove the cloned checkout when done
#
# Requirements: git and the Rust toolchain pinned in RUST_TOOLCHAIN below with its
# wasm32-unknown-unknown target (only when building WASM, i.e. without --wasm-dir / --wasm-file).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
SDK_REMOTE="https://github.com/paritytech/polkadot-sdk"
DEFAULT_SDK="${REPO_ROOT}/target/polkadot-sdk-codegen"

# The rococo/westend chain family both targets cover by default.
ROCOCO_WESTEND_CHAINS="rococo westend asset-hub-rococo asset-hub-westend bridge-hub-rococo bridge-hub-westend"

# ============================================================================
# Rust toolchain used to build the polkadot-sdk runtimes.
#
# polkadot-sdk ships no rust-toolchain.toml, so we pin the build toolchain here. A too-new rustc
# fails to compile the runtimes (e.g. `#[no_mangle] cannot be used on internal language items` in
# sp-io).
#
# >>> UPDATE THIS whenever the pinned polkadot-sdk revision changes, to a rustc that builds it. <<<
# ============================================================================
RUST_TOOLCHAIN="1.84.1"

usage() { sed -n '/^# Usage:/,/^# Requirements:/p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; }

TARGET=""
MODE=""            # "full" | "types-only" | "" (default per target)
CHAINS_OVERRIDE=""
HASH=""
SDK_DIR=""
WASM_DIR=""
WASM_FILE=""
CLEANUP=0
while [ $# -gt 0 ]; do
	case "$1" in
		--target) TARGET="$2"; shift 2 ;;
		--full) MODE="full"; shift ;;
		--types-only) MODE="types-only"; shift ;;
		--chains) CHAINS_OVERRIDE="$2"; shift 2 ;;
		--polkadot-sdk-hash) HASH="$2"; shift 2 ;;
		--polkadot-sdk) SDK_DIR="$2"; shift 2 ;;
		--wasm-dir) WASM_DIR="$2"; shift 2 ;;
		--wasm-file) WASM_FILE="$2"; shift 2 ;;
		--cleanup) CLEANUP=1; shift ;;
		-h|--help) usage; exit 0 ;;
		*) echo "unknown argument: $1" >&2; usage; exit 1 ;;
	esac
done

# Target-dependent defaults: chain list, subxt mode and output location.
case "${TARGET}" in
	zombienet)     DEFAULT_CHAINS="${ROCOCO_WESTEND_CHAINS}"; DEFAULT_MODE="full" ;;
	relay-clients) DEFAULT_CHAINS="${ROCOCO_WESTEND_CHAINS}"; DEFAULT_MODE="types-only" ;;
	"")            echo "ERROR: --target is required (zombienet | relay-clients)" >&2; usage; exit 1 ;;
	*)             echo "ERROR: unknown --target '${TARGET}' (expected zombienet | relay-clients)" >&2; exit 1 ;;
esac
MODE="${MODE:-${DEFAULT_MODE}}"
read -r -a CHAINS <<<"${CHAINS_OVERRIDE:-${DEFAULT_CHAINS}}"
[ "${MODE}" = "full" ] && FULL_FLAG="--full" || FULL_FLAG=""

if [ -n "${WASM_FILE}" ]; then
	[ -n "${WASM_DIR}" ] && { echo "ERROR: pass only one of --wasm-file / --wasm-dir" >&2; exit 1; }
	[ "${#CHAINS[@]}" -eq 1 ] || { echo "ERROR: --wasm-file needs exactly one --chains entry (got: ${CHAINS[*]})" >&2; exit 1; }
	[ -f "${WASM_FILE}" ] || { echo "ERROR: --wasm-file not found: ${WASM_FILE}" >&2; exit 1; }
fi

# Output path for a given dashed chain name.
out_path() {
	local chain="$1" underscored="${1//-/_}"
	case "${TARGET}" in
		zombienet)     echo "${REPO_ROOT}/testing/zombienet-sdk-tests/tests/codegen/${underscored}.rs" ;;
		relay-clients) echo "${REPO_ROOT}/relay-clients/client-${chain}/src/codegen_runtime.rs" ;;
	esac
}

# Resolve the WASM blob for a given dashed chain name (echoes empty if not found).
resolve_wasm() {
	local chain="$1" underscored="${1//-/_}" pat m
	if [ -n "${WASM_FILE}" ]; then echo "${WASM_FILE}"; return; fi
	if [ -n "${WASM_DIR}" ]; then
		# Fellows blobs are `bridge-hub-polkadot_runtime-vX.wasm`; flat wbuild copies are underscored.
		for pat in "${chain}_runtime"*.wasm "${underscored}_runtime"*.wasm; do
			for m in "${WASM_DIR}"/${pat}; do [ -e "${m}" ] && { echo "${m}"; return; }; done
		done
		echo ""; return
	fi
	echo "${SDK_DIR}/target/release/wbuild/${chain}-runtime/${underscored}_runtime.wasm"
}

echo ">> target=${TARGET} mode=${MODE} chains=(${CHAINS[*]})"

# 1. Obtain runtime WASM: from prebuilt blobs (--wasm-file/--wasm-dir), or by building a checkout.
CLONED=0
if [ -n "${WASM_FILE}" ]; then
	echo ">> using prebuilt WASM file: ${WASM_FILE}"
elif [ -n "${WASM_DIR}" ]; then
	echo ">> using prebuilt WASM from dir: ${WASM_DIR}"
else
	# Resolve the polkadot-sdk commit: --polkadot-sdk-hash, else the pin from Cargo.lock.
	REV="${HASH:-$("${SCRIPT_DIR}/polkadot-sdk-rev.sh")}"
	echo ">> polkadot-sdk revision: ${REV}${HASH:+ (override)}"

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

	# Toolchain check (polkadot-sdk ships no rust-toolchain.toml, so pin one; too-new rustc fails).
	echo ">> using rust toolchain: ${RUST_TOOLCHAIN}"
	rustup toolchain list | grep -q "^${RUST_TOOLCHAIN}" \
		|| { echo "ERROR: rust toolchain '${RUST_TOOLCHAIN}' not installed (rustup toolchain install ${RUST_TOOLCHAIN})" >&2; exit 1; }
	rustup target add --toolchain "${RUST_TOOLCHAIN}" wasm32-unknown-unknown >/dev/null 2>&1 || true

	# Build the needed `<chain>-runtime` WASM blobs (wasm-builder emits them under target/release/wbuild/).
	echo ">> building runtime wasm (slow on a fresh checkout)"
	PKGS=()
	for chain in "${CHAINS[@]}"; do PKGS+=(-p "${chain}-runtime"); done
	( cd "${SDK_DIR}" && RUSTUP_TOOLCHAIN="${RUST_TOOLCHAIN}" CARGO_NET_GIT_FETCH_WITH_CLI=true \
		cargo build --release "${PKGS[@]}" )
fi

cleanup() {
	# The cloned checkout is kept for reuse by default; remove it only on --cleanup.
	if [ "${CLONED}" = "1" ] && [ "${CLEANUP}" = "1" ]; then
		echo ">> removing cloned checkout ${SDK_DIR}"
		rm -rf "${SDK_DIR}"
	fi
}
trap cleanup EXIT

# 2. Build this repo's runtime-codegen tool (uses subxt-codegen to emit the client).
echo ">> building runtime-codegen"
cargo build --release --manifest-path "${REPO_ROOT}/tools/runtime-codegen/Cargo.toml"
TOOL="${REPO_ROOT}/tools/runtime-codegen/target/release/runtime-codegen"

# 3. Generate one module per chain.
OUTS=()
for chain in "${CHAINS[@]}"; do
	wasm="$(resolve_wasm "${chain}")"
	out="$(out_path "${chain}")"
	[ -n "${wasm}" ] && [ -f "${wasm}" ] || { echo "ERROR: runtime wasm not found for ${chain} (looked for: ${wasm:-<none>})" >&2; exit 1; }
	mkdir -p "$(dirname "${out}")"
	echo ">> generating $(basename "${out}") from $(basename "${wasm}") ${FULL_FLAG}"
	# shellcheck disable=SC2086
	"${TOOL}" ${FULL_FLAG} --from-wasm-file "${wasm}" > "${out}"
	# Fix a subxt bug that drops the `Header` hash param (matches scripts/regenerate_runtimes.sh and
	# the release workflow). Applied unconditionally; the pattern only appears in `runtime_types`-only
	# output, so it is a no-op for `--full`.
	sed -i 's/::sp_runtime::generic::Header<::core::primitive::u32>/::sp_runtime::generic::Header<::core::primitive::u32, ::sp_runtime::traits::BlakeTwo256>/g' "${out}"
	OUTS+=("${out}")
done

# 4. Format to the repo style (hard tabs). runtime-codegen emits prettyplease (4-space) output, and
# the zombienet modules are also skipped by `cargo fmt` (cfg-gated), so format the outputs directly.
echo ">> formatting generated modules (nightly rustfmt)"
rustup run nightly rustfmt --edition 2021 --config-path "${REPO_ROOT}/rustfmt.toml" "${OUTS[@]}"

echo ">> done. Generated:"
for out in "${OUTS[@]}"; do echo "   ${out#${REPO_ROOT}/}"; done
echo ">> Remember to commit the regenerated modules (and re-run the relevant tests)."
