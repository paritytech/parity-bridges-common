---
name: release-pr
description: Create a release PR from prepared release changes
metadata:
  argument-hint: ""
---

# Release PR

Create a release pull request from changes prepared by `/release-prepare`.

## Usage

```
/release-pr
```

No arguments required. The skill detects the release details from the current workspace changes.

## Prerequisites

- Changes from `/release-prepare` must be present in the working tree (not yet committed)
- Working tree should be on a clean base (no unrelated uncommitted changes)

## Procedure

### Step 1: Detect Release Details

Read the current state to determine what was changed:

```bash
git diff --name-only
```

From the diff, identify:
- Which chain was updated (check `relay-clients/client-bridge-hub-*/src/lib.rs` for spec_version changes)
- The new relay version from `substrate-relay/Cargo.toml`
- The new spec_version from the modified `lib.rs`

If no release-related changes are detected, inform the user and suggest running `/release-prepare` first.

### Step 2: Final Verification

Run build and lint checks:

```bash
cargo check --workspace
SKIP_WASM_BUILD=1 cargo clippy --all-targets --locked --workspace
```

If either fails, report the errors and stop. Do not create a PR with failing checks.

### Step 3: Read All Bundled Chain Versions

Collect spec_version from every relay-client for the PR body:

```bash
grep 'spec_version' relay-clients/client-*/src/lib.rs
```

### Step 4: Create Branch and Commit

```bash
git checkout -b release-v<RELAY_VERSION>
git add relay-clients/ substrate-relay/Cargo.toml Cargo.lock
git commit -m "Release v<RELAY_VERSION>"
```

Use the actual relay version read from `substrate-relay/Cargo.toml`.

### Step 5: Push and Create PR

```bash
git push -u origin release-v<RELAY_VERSION>
```

Create the PR with the `A-release` label:

```bash
gh pr create --title "Release v<RELAY_VERSION>" --label "A-release" --body "$(cat <<'EOF'
## Summary
- Bump substrate-relay version to <RELAY_VERSION>
- Update <CHAIN_NAME> spec_version: <OLD> → <NEW>
- Regenerate runtime codegen

## Bundled Chain Versions
<table of all chain spec_versions>

Docker reference: `paritytech/substrate-relay:v<RELAY_VERSION>`

## Test plan
- [ ] CI passes (fmt, clippy, check, test, deny)
- [ ] Verify spec_version matches on-chain runtime
- [ ] Verify codegen_runtime.rs regenerated correctly
EOF
)"
```

### Step 6: Report

Output:
- PR URL
- Summary of changes included
- Remind: "After PR is merged, run `/release-finalize` to tag and publish"
