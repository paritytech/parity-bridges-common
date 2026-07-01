#!/usr/bin/env bash
#
# Print the polkadot-sdk git revision this repo pins in `Cargo.lock` (the full 40-char commit).
#
# Single source of truth for the pinned revision, reused by:
#   - testing/zombienet-sdk-tests/tests/codegen/generate.sh  (which SDK commit to build codegen from)
#   - .github/workflows/zombienet.yml                        (tag for the polkadot/polkadot-parachain
#                                                             node images; callers take the first 8 chars)
#
# All polkadot-sdk crates share one git source, so `sort -u` collapses to a single revision (and
# avoids a `| head` that would SIGPIPE under `pipefail`).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

REV="$(grep -oE 'polkadot-sdk\?branch=master#[0-9a-f]+' "${REPO_ROOT}/Cargo.lock" | cut -d'#' -f2 | sort -u)"
REV="${REV%%$'\n'*}" # first line, in case multiple sources are ever pinned
[ -n "${REV}" ] || { echo "ERROR: could not find the polkadot-sdk revision in ${REPO_ROOT}/Cargo.lock" >&2; exit 1; }

printf '%s\n' "${REV}"
