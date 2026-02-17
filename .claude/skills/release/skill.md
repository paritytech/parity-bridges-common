---
name: release
description: Guide the substrate-relay release lifecycle
metadata:
  argument-hint: "[BridgeHubKusama|BridgeHubPolkadot] [runtime-version]"
---

# substrate-relay Release

Orchestrate the full release lifecycle for substrate-relay. This skill chains the sub-skills `/release-prepare`, `/release-pr`, and `/release-finalize` into a single flow.

Reference `RELEASE.md` for upstream documentation.

## Usage

```
/release                                    # Interactive: detect or ask which chain
/release BridgeHubKusama v1.1.6            # Direct: specify chain and runtime version
```

If `$ARGUMENTS` contains a chain name and version, pass them through. Otherwise, detect the need or ask the user.

## Procedure

### Step 1: Verify Release Need

Determine why a release is needed:

1. Check if version guard alerts are firing (suggests chain runtime upgrade):
   - Look for `"Aborting relay"` in recent logs
2. Check recent commits for chain version bumps: `git log --oneline -20`
3. Check if any relay-client `SimpleRuntimeVersion` constants are outdated

If `$ARGUMENTS` provides a chain and version, skip detection and proceed to Step 2.

If no arguments and no clear signal, ask the user which chain needs updating and what Fellows runtime version to use. Valid chains: `BridgeHubKusama`, `BridgeHubPolkadot`.

### Step 2: Prepare Release

Run the `/release-prepare` skill logic:

1. Execute `./scripts/prepare-bridge-hub-release.sh <CHAIN> <VERSION>`
2. Run `cargo check --workspace` to verify
3. Report: old spec_version → new spec_version, old relay version → new relay version

If this step fails, report errors and stop.

### Step 3: Create Release PR

Run the `/release-pr` skill logic:

1. Run final checks (`cargo check`, `cargo clippy`)
2. Create branch `release-v<RELAY_VERSION>`
3. Stage relevant files, commit, push
4. Create PR with `A-release` label via `gh pr create`
5. Output the PR URL

### Step 4: Wait for Merge

Print:
> PR created. After the PR is reviewed and merged, run `/release-finalize` to tag, publish the Docker image, and create a GitHub Release.

Stop here. The remaining steps happen after merge via `/release-finalize`.

### Step 5: Post-Merge (via /release-finalize)

After merge, `/release-finalize` handles:
- Creating annotated git tag `v<VERSION>` (with user confirmation)
- Pushing tag to trigger `build-tag.yml`
- Waiting for Docker build to complete
- Creating GitHub Release with bundled chain versions

### Step 6: Deploy

After `/release-finalize` completes, run `/deploy v<VERSION>` to deploy to production via ArgoCD.

### Step 7: Post-Deploy Verification

After deployment, run `/check-health all` to verify:
- Finality sync resumes
- Message delivery resumes
- Version guard alerts clear
- Relay balances stable
