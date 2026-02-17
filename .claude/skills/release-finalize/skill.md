---
name: release-finalize
description: Tag, publish, and verify a merged substrate-relay release
metadata:
  argument-hint: ""
---

# Release Finalize

After a release PR has been merged, create the git tag, verify the Docker build, and create a GitHub Release.

## Usage

```
/release-finalize
```

No arguments required. The skill reads the version from `substrate-relay/Cargo.toml` on the current branch.

## Prerequisites

- The release PR must be merged to `master`
- You should be on (or able to switch to) the `master` branch with the merged changes

## Procedure

### Step 1: Verify State

Ensure we're on `master` with the latest changes:

```bash
git checkout master
git pull
```

Read the version:

```bash
grep '^version' substrate-relay/Cargo.toml
```

Check that the version doesn't already have a tag:

```bash
git tag -l "v<VERSION>"
```

If the tag already exists, inform the user and skip to Step 4.

### Step 2: Create Annotated Tag

**Ask user for confirmation before proceeding.**

Show the user what will happen:
- Tag: `v<VERSION>`
- This will trigger the `build-tag.yml` workflow
- Docker images will be pushed to Docker Hub

Create the tag:

```bash
git tag -a v<VERSION> -m "Release v<VERSION>"
```

### Step 3: Push Tag

**Ask user for confirmation before pushing.**

```bash
git push origin v<VERSION>
```

This triggers the `build-tag.yml` pipeline which builds and pushes:
- `docker.io/paritytech/substrate-relay:v<VERSION>`
- `docker.io/paritytech/bridges-common-relay:v<VERSION>`

### Step 4: Wait for Docker Build

Monitor the build-tag pipeline:

```bash
gh run list --workflow=build-tag.yml --limit=5
```

Wait for the run to complete. Check status periodically:

```bash
gh run watch <RUN_ID>
```

### Step 5: Create GitHub Release

Collect bundled chain versions for the release notes:

```bash
grep 'spec_version' relay-clients/client-*/src/lib.rs
```

Create the release:

```bash
gh release create v<VERSION> --title "Release v<VERSION>" --notes "$(cat <<'EOF'
Docker reference: `paritytech/substrate-relay:v<VERSION>`

Bundled Chain Versions:

- Rococo: `<spec_version>`
- Westend: `<spec_version>`
- Kusama: `<spec_version>`
- Polkadot: `<spec_version>`
- Rococo Bridge Hub: `<spec_version>`
- Westend Bridge Hub: `<spec_version>`
- Kusama Bridge Hub: `<spec_version>`
- Polkadot Bridge Hub: `<spec_version>`
- Rococo Bulletin: `None` (must be specified in CLI)
- Polkadot Bulletin: `None` (must be specified in CLI)
EOF
)" --generate-notes
```

### Step 6: Post-Release

Report completion:
- Tag: `v<VERSION>`
- Docker image: `paritytech/substrate-relay:v<VERSION>`
- GitHub Release URL

Suggest next steps:
1. Deploy via ArgoCD: go to [deploy.yml](https://github.com/paritytech/parity-bridges-common/actions/workflows/deploy.yml) and run workflow with the new version tag
2. After deployment: run `/check-health all` to verify relay health
