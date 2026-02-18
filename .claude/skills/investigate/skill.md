---
name: investigate
description: Root-cause analysis for bridge relay alerts and issues
metadata:
  argument-hint: "<alert-name or issue description>"
---

# Bridge Alert Investigation

Perform root-cause analysis for a specific bridge alert or reported issue.

## Usage

```
/investigate Polkadot -> KusamaBridgeHub finality sync lags
/investigate version guard abort kusama-polkadot
/investigate relay balance low at PBH
/investigate messages not being delivered rococo-westend
```

`$ARGUMENTS` describes the alert or issue to investigate.

## Procedure

### Step 1: Classify the Alert

Parse `$ARGUMENTS` and classify into one of these categories:

| Category | Keywords / Patterns |
|----------|-------------------|
| **Finality Lag** | "finality", "sync lag", "best_source_at_target", "headers not advancing" |
| **Message Delivery Lag** | "delivery lag", "messages not delivered", "target_latest_received" |
| **Confirmation Lag** | "confirmation lag", "unconfirmed", "source_latest_confirmed" |
| **Reward Lag** | "reward lag", "target_latest_confirmed" |
| **Header Mismatch** | "header mismatch", "different_forks", "fork" |
| **Node Down** | "node down", "relay down", "up{", "not running" |
| **Version Guard** | "version guard", "aborting relay", "abort" |
| **Balance Low** | "balance", "low balance", "funds", "top up" |
| **BEEFY Issues** | "beefy", "lagging sessions", "beefy best block" |
| **Test Message Failure** | "test message", "not generated" |

### Step 2: Identify the Bridge

Determine which bridge is affected from the alert name or description:

| Bridge | Identifiers |
|--------|------------|
| Kusama <> Polkadot | "kusama", "polkadot", "KBH", "PBH", "00000001", "parity-chains" |
| Rococo <> Westend | "rococo", "westend", "RBH", "WBH", "00000002", "parity-testnet" |
| Polkadot Bulletin | "polkadot bulletin", "PBC" |
| Rococo Bulletin | "rococo bulletin", "RBC" |

### Step 3: Run Category-Specific Decision Tree

#### Finality Lag

**Likely causes** (check in order):

1. **Relay node down** - Check if `up{container="bridges-common-relay"} < 1`
   - Evidence: Look at relay liveness alerts in the same alert file
   - Resolution: Restart relay pod via ArgoCD

2. **Source chain stalled** - The relay chain itself stopped producing blocks
   - Evidence: Check if the relay chain is producing blocks (external to this repo)
   - Resolution: Wait for chain recovery; nothing to do on relay side

3. **Version guard abort** - Relay detected runtime mismatch and stopped
   - Evidence: Search alert JSON for `"Aborting relay"` Loki queries
   - Cross-reference: Read `relay-clients/client-*/src/lib.rs` for bundled `SimpleRuntimeVersion`
   - Resolution: `/release` to update chain versions

4. **Target chain congestion** - Target bridge hub is congested, transactions failing
   - Evidence: Check relay balance (may be draining from failed txs)
   - Resolution: Wait for congestion to clear; monitor balance

5. **Code bug in header relay** - Logic error in header sync
   - Evidence: Check recent commits to `substrate-relay/src/bridges/*/` and `substrate-relay/src/cli/relay_headers.rs`
   - Resolution: Investigate and fix

#### Message Delivery Lag

**Likely causes**:

1. **Finality relay stalled** - Messages can't be delivered without finality proofs
   - Evidence: Check finality sync alerts for the same bridge
   - Resolution: Fix finality relay first

2. **No pending messages** - `source_latest_generated == target_latest_received` (false alarm)
   - Evidence: Both nonces are equal
   - Resolution: No action needed; alert may be transient

3. **Relay balance too low** - Can't submit delivery transactions
   - Evidence: Check balance alerts for the target bridge hub
   - Resolution: Top up relay account

4. **Version guard abort** - Same as finality lag cause #3
   - Resolution: `/release`

5. **Message lane stuck** - Logic error or on-chain issue
   - Evidence: Check `substrate-relay/src/bridges/*/bridge_hub_*_messages_to_*.rs`
   - Resolution: Investigate message lane configuration

#### Confirmation Lag

**Likely causes**:

1. **Delivery relay in one direction is stalled** - Confirmations travel in the reverse direction
   - Evidence: Check delivery lag alerts for the reverse direction
   - Resolution: Fix the reverse-direction relay

2. **High message volume** - Confirmations batched, temporarily behind
   - Evidence: Gap is growing slowly, not stuck
   - Resolution: Usually self-healing; monitor

3. **Relay balance too low on source chain** - Can't submit confirmation transactions
   - Evidence: Check balance for the source bridge hub relay account
   - Resolution: Top up relay account

#### Reward Lag

**Likely causes**:

1. **Confirmation relay stalled** - Rewards depend on confirmations
   - Evidence: Check confirmation lag for the same direction
   - Resolution: Fix confirmation relay

2. **Reward mechanism issue** - On-chain reward logic
   - Evidence: Check `substrate-relay/src/bridges/*/` for reward-related code
   - Resolution: Investigate on-chain

#### Header Mismatch

**Likely causes**:

1. **Chain fork** - Source chain experienced a fork, and the relay synced a fork block
   - Evidence: `*_is_source_and_source_at_target_using_different_forks > 0`
   - Resolution: Usually self-healing once the fork resolves. The relay should re-sync the correct header.

2. **Stale data after chain restart** - Temporary inconsistency
   - Evidence: Metric briefly spikes then returns to 0
   - Resolution: No action if transient

3. **Equivocation** - Validator equivocated on the source chain
   - Evidence: Persistent mismatch
   - Resolution: Check `detect-equivocations` command output

#### Node Down

**Likely causes**:

1. **Pod crashed or evicted** - Kubernetes scheduling issue
   - Evidence: `up{container="bridges-common-relay"} < 1`
   - Resolution: Check ArgoCD app `bridges-common-relay` status; pod may auto-restart

2. **OOM kill** - Relay binary ran out of memory
   - Evidence: Check pod logs for OOM signals
   - Resolution: Increase memory limits in ArgoCD deployment

3. **Configuration error** - Bad environment variables or missing keys
   - Evidence: Relay crashes on startup
   - Resolution: Check deployment configuration

#### Version Guard

**Likely causes**:

1. **Chain runtime upgraded** - The on-chain runtime version no longer matches the bundled version
   - Evidence: Search relay-client files for `RUNTIME_VERSION` constants and compare with on-chain values
   - Cross-reference: `relay-clients/client-bridge-hub-kusama/src/lib.rs`, `relay-clients/client-bridge-hub-polkadot/src/lib.rs`, etc.
   - Resolution: **This is the #1 cause.** Use `/release` to update chain versions and deploy.

2. **Test bridge with strict mode** - Test bridges normally skip version checks, but misconfiguration can enable them
   - Evidence: Only affects rococo-westend or rococo-bulletin
   - Resolution: Check relay startup flags

#### Balance Low

**Likely causes**:

1. **Normal depletion** - Relay has been running and spending on transactions
   - Evidence: Balance gradually declining
   - Resolution: Top up the relay account on the respective bridge hub

2. **Failed transactions draining balance** - Transactions failing but still paying fees
   - Evidence: Balance dropping faster than normal; check for version guard or other errors
   - Resolution: Fix the underlying issue first, then top up

Balance thresholds:
- KBH relay: < 2 KSM
- PBH relay: < 10 DOT
- RBH relay: < 10 ROC
- WBH relay: < 10 WND

#### BEEFY Issues

**Likely causes**:

1. **BEEFY protocol lag** - BEEFY finality gadget lagging behind GRANDPA
   - Evidence: `substrate_beefy_lagging_sessions` increasing
   - Cross-reference: `deployments/bridges/rococo/dashboard/grafana/rococo-beefy-alerts.json`
   - Resolution: Check Rococo validator BEEFY configuration

2. **BEEFY best block stalled** - `substrate_beefy_best_block` not advancing (threshold: < 100 increase/1h)
   - Evidence: Block metric flat
   - Resolution: Rococo infrastructure issue; escalate

#### Test Message Failure

**Likely causes**:

1. **Bridge fully stalled** - All relay components down
   - Evidence: Multiple other alerts firing simultaneously
   - Resolution: Diagnose and fix relay components

2. **Test message generation disabled** - Configuration change
   - Evidence: Only this alert fires, everything else OK
   - Resolution: Check relay configuration for test message generation settings

### Step 4: Cross-Reference Code

For code-related investigation:

1. Check recent commits: `git log --oneline -20 -- <relevant-paths>`
2. Read bridge configuration: `substrate-relay/src/bridges/<bridge>/mod.rs`
3. Read relay-client versions: `relay-clients/client-<chain>/src/lib.rs`
4. Check for open PRs: `gh pr list --state open`
5. Check for recent releases: `gh release list --limit 5`

### Step 5: Output Root-Cause Analysis

```
## Investigation: <alert-name>

**Category**: <classification>
**Bridge**: <bridge-name>
**Urgency**: CRITICAL / HIGH / MEDIUM / LOW

### Root Cause
<concise description of the most likely cause>

### Evidence
- <what was checked>
- <what was found>
- <code references with file:line>

### Impact
- <what is broken>
- <what users/bridges are affected>
- <data flow interruption details>

### Resolution
1. <step-by-step fix>
2. <verification steps>

### Related Skills
- `/release` - if version update needed
- `/check-health` - to verify fix across all dimensions
```

### Urgency Levels

| Level | Criteria |
|-------|----------|
| **CRITICAL** | Production bridge (Kusama-Polkadot) fully stalled; messages not flowing |
| **HIGH** | Production bridge degraded (one direction stalled, or balance critically low) |
| **MEDIUM** | Testnet bridge issues, or production warnings approaching thresholds |
| **LOW** | Cosmetic issues, non-blocking warnings, test message failures on testnet |
