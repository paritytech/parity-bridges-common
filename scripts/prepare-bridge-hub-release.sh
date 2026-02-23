#!/usr/bin/env bash
#
# Prepare a BridgeHub runtime upgrade for substrate-relay.
#
# Downloads the new WASM runtime, updates spec_version, regenerates codegen,
# applies the BlakeTwo256 workaround, and bumps the relay version.
#
# Usage: ./scripts/prepare-bridge-hub-release.sh <BridgeHubKusama|BridgeHubPolkadot> <runtime-version>
# Example: ./scripts/prepare-bridge-hub-release.sh BridgeHubKusama v1.1.6
#
# Does NOT: run cargo check, commit, create PR, or touch git.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
MATRIX_FILE="$SCRIPT_DIR/runtime-matrix.json"

usage() {
    echo "Usage: $0 <BridgeHubKusama|BridgeHubPolkadot> <runtime-version>"
    echo ""
    echo "  runtime-version: Fellows runtimes release tag, e.g. v1.1.6"
    echo ""
    echo "Example: $0 BridgeHubKusama v1.1.6"
    exit 1
}

die() {
    echo "ERROR: $*" >&2
    exit 1
}

# --- Validate inputs ---

if [[ $# -ne 2 ]]; then
    usage
fi

CHAIN_NAME="$1"
RUNTIME_VERSION="$2"

if ! command -v jq &>/dev/null; then
    die "jq is required but not found. Install it with: brew install jq"
fi

if [[ ! -f "$MATRIX_FILE" ]]; then
    die "Runtime matrix file not found: $MATRIX_FILE"
fi

# Check chain name exists in matrix
CHAIN_ENTRY=$(jq -r --arg name "$CHAIN_NAME" '.[] | select(.name == $name)' "$MATRIX_FILE")
if [[ -z "$CHAIN_ENTRY" ]]; then
    VALID_NAMES=$(jq -r '.[].name' "$MATRIX_FILE" | tr '\n' ', ' | sed 's/,$//')
    die "Unknown chain: $CHAIN_NAME. Valid chains: $VALID_NAMES"
fi

# Validate version format: vX.Y.Z
if [[ ! "$RUNTIME_VERSION" =~ ^v([0-9]+)\.([0-9]+)\.([0-9]+)$ ]]; then
    die "Invalid version format: $RUNTIME_VERSION. Expected vX.Y.Z (e.g. v1.1.6)"
fi

MAJOR="${BASH_REMATCH[1]}"
MINOR="${BASH_REMATCH[2]}"
PATCH="${BASH_REMATCH[3]}"

# --- Parse version ---

# v1.1.6 → RELEASE_VERSION=v1001006, SPEC_VERSION=1_001_006
RELEASE_VERSION="v$(printf '%d%03d%03d' "$MAJOR" "$MINOR" "$PATCH")"
SPEC_VERSION="$(printf '%d_%03d_%03d' "$MAJOR" "$MINOR" "$PATCH")"

echo "=== Preparing $CHAIN_NAME release ==="
echo "  Runtime version: $RUNTIME_VERSION"
echo "  Release version: $RELEASE_VERSION"
echo "  Spec version:    $SPEC_VERSION"

# --- Read matrix entry ---

RUNTIME_URI_TEMPLATE=$(echo "$CHAIN_ENTRY" | jq -r '.runtime_uri')
RUNTIME_FILE_TEMPLATE=$(echo "$CHAIN_ENTRY" | jq -r '.runtime_file')
TARGET_DIR=$(echo "$CHAIN_ENTRY" | jq -r '.target_dir')

# Substitute placeholders
RUNTIME_URI="${RUNTIME_URI_TEMPLATE//\$\{RUNTIME_VERSION\}/$RUNTIME_VERSION}"
RUNTIME_URI="${RUNTIME_URI//\$\{RELEASE_VERSION\}/$RELEASE_VERSION}"
RUNTIME_FILE="${RUNTIME_FILE_TEMPLATE//\$\{RUNTIME_VERSION\}/$RUNTIME_VERSION}"
RUNTIME_FILE="${RUNTIME_FILE//\$\{RELEASE_VERSION\}/$RELEASE_VERSION}"

echo "  Download URL:    $RUNTIME_URI"
echo "  Target dir:      $TARGET_DIR"

# --- Download WASM ---

TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

WASM_PATH="$TMPDIR/$RUNTIME_FILE"

echo ""
echo "--- Downloading WASM runtime ---"
if ! curl -sL --fail -o "$WASM_PATH" "$RUNTIME_URI"; then
    die "Failed to download WASM from: $RUNTIME_URI"
fi

WASM_SIZE=$(wc -c < "$WASM_PATH" | tr -d ' ')
echo "  Downloaded: $RUNTIME_FILE ($WASM_SIZE bytes)"

# --- Update spec_version ---

LIB_RS="$REPO_ROOT/$TARGET_DIR/lib.rs"
if [[ ! -f "$LIB_RS" ]]; then
    die "lib.rs not found: $LIB_RS"
fi

echo ""
echo "--- Updating spec_version in $TARGET_DIR/lib.rs ---"

OLD_SPEC=$(grep 'spec_version:' "$LIB_RS" | sed 's/.*spec_version: *\([0-9_]*\).*/\1/' || true)
if [[ -z "$OLD_SPEC" ]]; then
    die "Could not find spec_version in $LIB_RS"
fi

echo "  Old spec_version: $OLD_SPEC"
echo "  New spec_version: $SPEC_VERSION"

# Replace spec_version value (preserves surrounding whitespace and format)
sed -i'' -e "s/spec_version: ${OLD_SPEC}/spec_version: ${SPEC_VERSION}/" "$LIB_RS"

# Verify the replacement happened
if ! grep -q "spec_version: ${SPEC_VERSION}" "$LIB_RS"; then
    die "Failed to update spec_version in $LIB_RS"
fi

# --- Regenerate codegen ---

echo ""
echo "--- Regenerating codegen_runtime.rs ---"

CODEGEN_OUTPUT="$REPO_ROOT/$TARGET_DIR/codegen_runtime.rs"

(
    cd "$REPO_ROOT/tools/runtime-codegen"
    cargo run --bin runtime-codegen --release -- --from-wasm-file "$WASM_PATH"
) > "$CODEGEN_OUTPUT"

echo "  Generated: $TARGET_DIR/codegen_runtime.rs"

# --- Apply BlakeTwo256 fix ---

echo ""
echo "--- Applying BlakeTwo256 workaround ---"

(cd "$REPO_ROOT" && cargo +nightly fmt --all)

find "$REPO_ROOT" -name codegen_runtime.rs -exec \
    sed -i'' -e 's/::sp_runtime::generic::Header<::core::primitive::u32>/::sp_runtime::generic::Header<::core::primitive::u32, ::sp_runtime::traits::BlakeTwo256>/g' {} +

(cd "$REPO_ROOT" && cargo +nightly fmt --all)

echo "  BlakeTwo256 fix applied and formatted"

# --- Bump substrate-relay version ---

echo ""
echo "--- Bumping substrate-relay patch version ---"

CARGO_TOML="$REPO_ROOT/substrate-relay/Cargo.toml"
OLD_RELAY_VERSION=$(grep '^version' "$CARGO_TOML" | head -1 | sed 's/.*"\(.*\)".*/\1/')

if [[ ! "$OLD_RELAY_VERSION" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)$ ]]; then
    die "Could not parse relay version: $OLD_RELAY_VERSION"
fi

RELAY_MAJOR="${BASH_REMATCH[1]}"
RELAY_MINOR="${BASH_REMATCH[2]}"
RELAY_PATCH="${BASH_REMATCH[3]}"
NEW_RELAY_VERSION="${RELAY_MAJOR}.${RELAY_MINOR}.$((RELAY_PATCH + 1))"

sed -i'' -e "s/^version = \"${OLD_RELAY_VERSION}\"/version = \"${NEW_RELAY_VERSION}\"/" "$CARGO_TOML"

echo "  Old relay version: $OLD_RELAY_VERSION"
echo "  New relay version: $NEW_RELAY_VERSION"

# --- Summary ---

echo ""
echo "=== Release preparation complete ==="
echo ""
echo "  Chain:           $CHAIN_NAME"
echo "  Runtime version: $RUNTIME_VERSION"
echo "  Spec version:    $OLD_SPEC → $SPEC_VERSION"
echo "  Relay version:   $OLD_RELAY_VERSION → $NEW_RELAY_VERSION"
echo ""
echo "Modified files:"
echo "  - $TARGET_DIR/lib.rs"
echo "  - $TARGET_DIR/codegen_runtime.rs"
echo "  - substrate-relay/Cargo.toml"
echo ""
echo "Next steps:"
echo "  1. Run: cargo check --workspace"
echo "  2. Run: cargo test --workspace"
echo "  3. Commit and create a release PR"
