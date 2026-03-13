---
name: debug-ci
description: Diagnose CI pipeline failures and suggest fixes
metadata:
  argument-hint: "[pr-number|run-id|job-name]"
---

# Debug CI

Diagnose CI pipeline failures by fetching logs from GitHub Actions, identifying the root cause, and suggesting local reproduction steps.

## Usage

```
/debug-ci              # Diagnose the latest CI failure on the current branch
/debug-ci 3218         # Diagnose CI for a specific PR
/debug-ci clippy       # Diagnose a specific job type
```

## CI Jobs Reference

The `ci.yml` workflow runs these jobs (all depend on `set-variables`):

| Job | Command | Runner | Blocks merge | Notes |
|-----|---------|--------|--------------|-------|
| `fmt` | `cargo +nightly fmt --all -- --check` | ubuntu-latest | Yes | |
| `clippy` | `SKIP_WASM_BUILD=1 cargo clippy --all-targets --locked --workspace` | ubuntu-latest | No (`continue-on-error`) | Rust cache disabled |
| `spellcheck` | `cargo spellcheck check` | ubuntu-latest | Yes | Excludes `codegen_runtime.rs`, `weights.rs` |
| `check` | `SKIP_WASM_BUILD=1 cargo check --locked --workspace` | ubuntu-latest | Yes | Rust cache disabled |
| `test` | `cargo test --workspace` | parity-large | Yes | Uses Rust cache |
| `deny` | `cargo deny check advisories bans sources` | ubuntu-latest | No (`continue-on-error`) | |
| `deny-licenses` | `cargo deny check licenses` | ubuntu-latest | Yes | |
| `check-rustdocs` | `cargo doc --no-deps --all --workspace --document-private-items` | ubuntu-latest | Yes | Uses Rust cache |
| `build` | `cargo build --release --workspace` | parity-large | Yes | Uses Rust cache |
| `build_docker` | Docker build (no push) | ubuntu-latest | Yes | Depends on `build` |

## Procedure

### Step 1: Identify the CI Run

Determine which run to debug based on `$ARGUMENTS`:

**If a PR number** (all digits, typically 4 digits):

```bash
gh run list --workflow=ci.yml --branch=<PR_HEAD_BRANCH> --limit=3
```

Or look up the PR first:

```bash
gh pr view <NUMBER> --json headRefName --jq '.headRefName'
```

**If a run ID** (large number):

```bash
gh run view <RUN_ID>
```

**If a job name** (e.g. `clippy`, `test`, `fmt`):
Find the latest run on the current branch, then focus on that specific job.

**If no arguments**: use the current branch:

```bash
gh run list --workflow=ci.yml --branch=$(git branch --show-current) --limit=3
```

If the current branch is `master`, show the latest failed run across all branches:

```bash
gh run list --workflow=ci.yml --status=failure --limit=5
```

Pick the most recent failed (or in-progress) run and note its `<RUN_ID>`.

### Step 2: Get Run Overview

Fetch the run summary to see which jobs passed and which failed:

```bash
gh run view <RUN_ID>
```

List all jobs and their statuses. Identify the failed job(s).

If all jobs passed, report success and stop.

### Step 3: Fetch Failed Job Logs

For each failed job, fetch the logs:

```bash
gh run view <RUN_ID> --log-failed
```

If the output is too large, target a specific job:

```bash
gh run view <RUN_ID> --job=<JOB_ID> --log
```

You can find job IDs from:

```bash
gh api repos/{owner}/{repo}/actions/runs/<RUN_ID>/jobs --jq '.jobs[] | "\(.id) \(.name) \(.conclusion)"'
```

### Step 4: Diagnose the Failure

Analyze the error output and classify it:

**Formatting (`fmt`)**:
- Error: "Diff in ..." — code is not formatted
- Fix: `cargo +nightly fmt --all`

**Linting (`clippy`)**:
- Parse clippy warnings/errors from the output
- Fix: `SKIP_WASM_BUILD=1 cargo clippy --all-targets --locked --workspace --fix --allow-dirty`
- Note: this job has `continue-on-error: true`, so it doesn't block merge

**Spellcheck**:
- Error: misspelled words in `.rs` files
- Fix: add words to `.config/spellcheck.toml` or fix the spelling
- Note: excludes `codegen_runtime.rs` and `weights.rs`

**Compile check (`check`)**:
- Error: compilation errors
- Fix: `SKIP_WASM_BUILD=1 cargo check --locked --workspace`
- Common causes: missing imports after codegen, type mismatches after runtime upgrade

**Tests (`test`)**:
- Parse test failure output: look for `FAILED`, `panicked at`, `test result: FAILED`
- Fix: `cargo test --workspace` locally
- Note: runs on parity-large runner, so resource-dependent tests may behave differently locally

**Dependency audit (`deny`)**:
- Error: advisory/ban/source violations
- Fix: `cargo deny check advisories bans sources`
- Note: this job has `continue-on-error: true`, so it doesn't block merge

**License check (`deny-licenses`)**:
- Error: license violation
- Fix: `cargo deny check licenses` — update `deny.toml` exceptions or change the dependency
- This one IS strict (blocks merge)

**Rustdocs (`check-rustdocs`)**:
- Error: documentation build failures (broken links, missing docs)
- Fix: `cargo doc --no-deps --all --workspace --document-private-items`

**Build**:
- Error: release build failure
- Fix: `cargo build --release --workspace`
- Common causes: same as `check`, but release-only optimizations can surface additional issues

**Docker build (`build_docker`)**:
- Error: Dockerfile issues, missing artifacts
- Fix: check `ci.Dockerfile` and ensure `build` job artifacts are correct

### Step 5: Check for Known Issues

Look for patterns that indicate systemic issues rather than code problems:

- **Rust cache disabled**: clippy and check jobs have cache disabled (`# todo: fixme`). If these jobs are slow or timing out, this is why.
- **Runner availability**: `parity-large` runner is needed for test and build. If these jobs are queued for a long time, it may be a runner capacity issue.
- **Concurrency cancellation**: the workflow cancels in-progress runs when a new push arrives. If a run shows as "cancelled", this is expected.
- **Fork PRs**: env variables don't work for fork PRs (handled via `set-variables` job outputs). If the CI image isn't resolving, check if the PR is from a fork.

### Step 6: Output Report

Present a structured report:

```
## CI Diagnosis: <branch or PR>

**Run**: <RUN_ID> (<status>)
**Triggered by**: <push to master | PR #N>

| Job | Status | Duration |
|-----|--------|----------|
| fmt | pass/fail | Xs |
| clippy | pass/fail (soft) | Xs |
| ... | ... | ... |

### Failure: <job name>

**Error**:
<key error lines from the log>

**Root cause**: <analysis>

**Fix**:
<specific commands to run locally>

### Recommendations
- <actionable next steps>
```

If the failure is in a `continue-on-error` job (clippy, deny), note that it doesn't block merge but should still be fixed.
