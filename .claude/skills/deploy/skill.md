---
name: deploy
description: Deploy substrate-relay to production via ArgoCD
metadata:
  argument-hint: "[version]"
---

# Deploy

Deploy a substrate-relay version to production via the ArgoCD deploy workflow.

## Usage

```
/deploy              # Auto-detect version from Cargo.toml or latest git tag
/deploy v1.8.16      # Deploy a specific version
```

## Prerequisites

- The version must have a corresponding git tag (created by `/release-finalize`)
- The Docker image `paritytech/substrate-relay:v<VERSION>` must exist on Docker Hub
- The `gh` CLI must be authenticated with permission to trigger workflows

## Procedure

### Step 1: Determine Version

If `$ARGUMENTS` contains a version (e.g. `v1.8.16`), use it. Strip the `v` prefix for comparisons if needed.

Otherwise, auto-detect:

```bash
grep '^version' substrate-relay/Cargo.toml
```

Or use the latest git tag:

```bash
git tag -l 'v*' --sort=-v:refname | head -1
```

Confirm the resolved version with the user.

### Step 2: Verify Docker Image Exists

Check Docker Hub for the image:

```bash
curl --fail --silent "https://hub.docker.com/v2/repositories/paritytech/substrate-relay/tags/v<VERSION>" | head -c 200
```

If the image does not exist, abort and suggest:
- If the tag exists but the image doesn't: check the `build-tag.yml` workflow status
- If the tag doesn't exist: run `/release-finalize` first

### Step 3: Show Deployment Plan

Show the user what will happen:

- **Version**: `v<VERSION>`
- **Docker image**: `paritytech/substrate-relay:v<VERSION>`
- **Environment**: `parity-chains` (production)
- **ArgoCD app**: `bridges-common-relay`
- **Packages**: `headers-a`, `headers-b`, `parachains-a`, `parachains-b`, `messages-a`, `messages-b`

Check what is currently deployed (last successful deploy run):

```bash
gh run list --workflow=deploy.yml --status=success --limit=3
```

**Ask user for confirmation before proceeding.** This is a production deployment.

### Step 4: Trigger Deploy Workflow

The deploy.yml workflow uses `github.ref_name` as the version, so trigger it with the correct ref:

```bash
gh workflow run deploy.yml --ref v<VERSION>
```

### Step 5: Monitor Deployment

Find and watch the triggered run:

```bash
gh run list --workflow=deploy.yml --limit=3
```

Then monitor it:

```bash
gh run watch <RUN_ID>
```

If the run fails:
- Check the logs: `gh run view <RUN_ID> --log-failed`
- Common failures: Docker image not found, ArgoCD auth issues, environment approval pending
- Report the failure and suggest next steps

### Step 6: Post-Deploy

After the workflow succeeds, report:
- Deploy workflow run URL
- Version deployed: `v<VERSION>`
- Environment: `parity-chains`

Suggest next step: run `/check-health all` to verify relay health after deployment.
