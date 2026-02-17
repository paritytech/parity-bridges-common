---
name: rollback
description: Roll back substrate-relay to a previous version
metadata:
  argument-hint: "[version]"
---

# Rollback

Roll back the production substrate-relay deployment to a previous version.

## Usage

```
/rollback              # Interactive: list recent tags and ask which to roll back to
/rollback v1.8.15      # Roll back to a specific version
```

## Prerequisites

- The target version must have a Docker image on Docker Hub
- The `gh` CLI must be authenticated with permission to trigger workflows

## Procedure

### Step 1: Determine Target Version

If `$ARGUMENTS` contains a version (e.g. `v1.8.15`), use it.

Otherwise, list recent tags and ask the user to pick:

```bash
git tag -l 'v*' --sort=-v:refname | head -10
```

Also identify what is currently deployed:

```bash
gh run list --workflow=deploy.yml --status=success --limit=5
```

Present the user with the recent versions and ask which one to roll back to.

### Step 2: Verify Docker Image Exists

Check Docker Hub for the target version's image:

```bash
curl --fail --silent "https://hub.docker.com/v2/repositories/paritytech/substrate-relay/tags/v<TARGET_VERSION>" | head -c 200
```

If the image does not exist, abort and explain that this version cannot be deployed.

### Step 3: Show Rollback Plan

Show what will change:

- **Current version** (from last successful deploy): `v<CURRENT>`
- **Rollback target**: `v<TARGET_VERSION>`
- **Docker image**: `paritytech/substrate-relay:v<TARGET_VERSION>`
- **Environment**: `parity-chains` (production)
- **ArgoCD app**: `bridges-common-relay`

Show the commits between the two versions so the user understands what is being rolled back:

```bash
git log --oneline v<TARGET_VERSION>..v<CURRENT>
```

**Ask user for confirmation before proceeding.** This is a production rollback.

### Step 4: Trigger Deploy Workflow

Use the same deploy workflow with the older version tag:

```bash
gh workflow run deploy.yml --ref v<TARGET_VERSION>
```

### Step 5: Monitor Rollback

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
- Report the failure and suggest next steps

### Step 6: Post-Rollback Verification

After the workflow succeeds, report:
- Deploy workflow run URL
- Rolled back from `v<CURRENT>` to `v<TARGET_VERSION>`
- Environment: `parity-chains`

Suggest next step: run `/check-health all` to verify relay health after rollback.
