#!/usr/bin/env bash
#
# (Re)generate the bridges zombienet-sdk subxt metadata (`.scale`) files so they match the
# polkadot-sdk revision this repo pins in `Cargo.lock`.
#
# Steps:
#   1. derive the pinned polkadot-sdk commit from this repo's Cargo.lock,
#   2. shallow-checkout polkadot-sdk at that exact commit (into ./polkadot-sdk, gitignored),
#   3. inject the `runner/` crate into that checkout and build it with `--features zombie-metadata`,
#      whose build.rs builds the six `*-runtime` WASM blobs (as needed) and extracts their metadata,
#   4. copy the freshly generated `.scale` files into `testing/zombienet-sdk-tests/metadata-files/`,
#   5. clean up (revert the checkout's Cargo.toml, remove the injected crate, and remove the
#      checkout unless it was supplied via --polkadot-sdk or --keep is given).
#
# Usage:
#   testing/metadata-gen/generate.sh                       # clone + build + cleanup
#   testing/metadata-gen/generate.sh --polkadot-sdk <path> # reuse an existing checkout (no cleanup of it)
#   testing/metadata-gen/generate.sh --keep                # keep the cloned ./polkadot-sdk for reuse
#
# Requirements: git, python3, and the Rust toolchain polkadot-sdk pins (rust-toolchain.toml in the
# checkout drives rustup; building runtimes needs the wasm32 target, which wasm-builder installs).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
RUNNER_SRC="${SCRIPT_DIR}/runner"
META_OUT="${REPO_ROOT}/testing/zombienet-sdk-tests/metadata-files"
DEFAULT_SDK="${SCRIPT_DIR}/polkadot-sdk"
SDK_REMOTE="https://github.com/paritytech/polkadot-sdk"

SDK_DIR=""
KEEP=0
while [ $# -gt 0 ]; do
	case "$1" in
		--polkadot-sdk) SDK_DIR="$2"; shift 2 ;;
		--keep) KEEP=1; shift ;;
		-h|--help) sed -n '2,30p' "${BASH_SOURCE[0]}"; exit 0 ;;
		*) echo "unknown argument: $1" >&2; exit 1 ;;
	esac
done

# 1. Pinned polkadot-sdk commit from Cargo.lock. All polkadot-sdk crates share one git source, so
# `sort -u` collapses to a single revision (and avoids a `| head` that would SIGPIPE under pipefail).
REV="$(grep -oE 'polkadot-sdk\?branch=master#[0-9a-f]+' "${REPO_ROOT}/Cargo.lock" | cut -d'#' -f2 | sort -u)"
REV="${REV%%$'\n'*}" # first line, in case multiple sources are ever pinned
[ -n "${REV}" ] || { echo "ERROR: could not find the polkadot-sdk revision in ${REPO_ROOT}/Cargo.lock" >&2; exit 1; }
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

# 3. Inject the runner crate + register it as a workspace member.
RUNNER_DST="${SDK_DIR}/bridges/testing/zombienet-metadata-gen"
rm -rf "${RUNNER_DST}"
mkdir -p "${RUNNER_DST}/metadata-files"
cp -r "${RUNNER_SRC}/." "${RUNNER_DST}/"

python3 - "${SDK_DIR}/Cargo.toml" <<'PY'
import sys
path = sys.argv[1]
member = '"bridges/testing/zombienet-metadata-gen",'
src = open(path).read()
if member not in src:
    src = src.replace("members = [", "members = [\n\t" + member, 1)
    open(path, "w").write(src)
PY

cleanup() {
	# revert the injected workspace member + remove the runner crate
	git -C "${SDK_DIR}" checkout -- Cargo.toml 2>/dev/null || true
	rm -rf "${RUNNER_DST}"
	if [ "${CLONED}" = "1" ] && [ "${KEEP}" = "0" ]; then
		echo ">> removing cloned checkout ${SDK_DIR}"
		rm -rf "${SDK_DIR}"
	fi
}
trap cleanup EXIT

# 4. Build -> build.rs generates the .scale files (forces a fresh build by clearing stale outputs).
rm -f "${RUNNER_DST}/metadata-files/"*.scale
echo ">> building runtimes + extracting metadata (this is slow on a fresh checkout)"
( cd "${SDK_DIR}" && ZOMBIE_METADATA_BUILD_DEBUG=1 CARGO_NET_GIT_FETCH_WITH_CLI=true \
	cargo build -p bridges-zombienet-metadata-gen --features zombie-metadata )

# 5. Copy the generated metadata into the repo.
mkdir -p "${META_OUT}"
cp "${RUNNER_DST}/metadata-files/"*.scale "${META_OUT}/"
echo ">> copied .scale files into ${META_OUT}:"
ls -la "${META_OUT}"/*.scale
echo ">> done. Remember to re-run the tests against the regenerated metadata."
