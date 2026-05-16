---
name: pipeline-status
description: Show CI/CD pipeline health at a glance
metadata:
  argument-hint: ""
---

# Pipeline Status

Show the overall health of all CI/CD pipelines, the currently deployed version, and any pending actions.

## Usage

```
/pipeline-status
```

No arguments required. Shows a full overview of all workflows, deployment state, and release readiness.

## Procedure

### Step 1: Gather Workflow Status

Fetch recent runs for all three workflows in parallel:

```bash
gh run list --workflow=ci.yml --limit=5
```

```bash
gh run list --workflow=build-tag.yml --limit=5
```

```bash
gh run list --workflow=deploy.yml --limit=5
```

For each workflow, note:
- Last run status (success, failure, in_progress, cancelled)
- When it ran
- What triggered it (branch, tag, manual)

### Step 2: Identify Current Deployed Version

Find the last successful deploy:

```bash
gh run list --workflow=deploy.yml --status=success --limit=1
```

Extract which version tag was used from the run's head branch/tag.

### Step 3: Identify Latest Release

Check the latest git tag and GitHub Release:

```bash
git tag -l 'v*' --sort=-v:refname | head -3
```

```bash
gh release list --limit=3
```

Compare: is the latest release deployed? If the latest tag is newer than the last deploy, flag it.

### Step 4: Check Current Branch CI

Show CI status for the current branch (if not master):

```bash
gh run list --workflow=ci.yml --branch=$(git branch --show-current) --limit=3
```

### Step 5: Check Docker Image Availability

Verify Docker images exist for the latest tag:

```bash
curl --fail --silent "https://hub.docker.com/v2/repositories/paritytech/substrate-relay/tags/$(git tag -l 'v*' --sort=-v:refname | head -1)" | head -c 200
```

### Step 6: Read Relay Version from Source

```bash
grep '^version' substrate-relay/Cargo.toml
```

Compare the Cargo.toml version against the latest git tag. If the Cargo.toml version is ahead, a release may be in progress.

### Step 7: Output Dashboard

Present a structured dashboard:

```
## Pipeline Status

### Workflows

| Workflow | Last Run | Status | Trigger | When |
|----------|----------|--------|---------|------|
| CI | #<id> | pass/fail | PR #N / push to master | <time> |
| Build Tag | #<id> | pass/fail | tag v<X> | <time> |
| Deploy | #<id> | pass/fail | manual (v<X>) | <time> |

### Versions

| | Version |
|---|---------|
| Source (Cargo.toml) | <version> |
| Latest tag | v<version> |
| Latest release | v<version> |
| Deployed | v<version> |
| Docker image | exists/missing |

### Current Branch: <branch>

CI status: <pass/fail/pending/none>

### Action Items

- <any issues that need attention>
```

Possible action items to flag:
- **CI failing on master**: "master CI is red — run `/debug-ci` to investigate"
- **Unreleased version bump**: "Cargo.toml (v1.8.16) is ahead of latest tag (v1.8.15) — run `/release-finalize` if PR is merged"
- **Undeployed release**: "Latest tag v1.8.16 is not deployed — run `/deploy v1.8.16`"
- **Missing Docker image**: "Docker image for v<X> not found — check `build-tag.yml` run"
- **Build-tag failure**: "Last `build-tag.yml` run failed — Docker image may not have been pushed"
- **Stale deploy**: "Last deploy was >30 days ago — verify relay health with `/check-health all`"
- **Branch CI red**: "Current branch CI is failing — run `/debug-ci` to investigate"

If everything looks healthy, report: "All pipelines green. No action needed."
