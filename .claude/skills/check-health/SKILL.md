---
name: check-health
description: Verify substrate-relay health after deployment
metadata:
  argument-hint: "[all|finality|messages|version-guard|balances]"
---

# Check Health

Verify that the substrate-relay is operating correctly after a deployment or upgrade. Queries Grafana alerts, Prometheus metrics, relay logs, and GitHub Actions to produce a health report.

## Usage

```
/check-health all                # Run all checks
/check-health finality           # Check finality sync only
/check-health messages           # Check message delivery only
/check-health version-guard      # Check for version guard aborts
/check-health balances           # Check relay account balances
```

If `$ARGUMENTS` is empty, default to `all`.

## Environments

Two bridge environments exist. Run checks for **both** unless the user specifies one.

| Environment | Prometheus domain | Grafana alert file |
|-------------|-------------------|--------------------|
| Kusama <> Polkadot (prod) | `parity-chains` | `deployments/bridges/kusama-polkadot/dashboard/grafana/bridge-kusama-polkadot-alerts.json` |
| Rococo <> Westend (testnet) | `parity-testnet` | `deployments/bridges/rococo-westend/dashboard/grafana/bridge-rococo-westend-alerts.json` |

## Procedure

### Step 1: Determine Scope

Parse `$ARGUMENTS`:
- `all` or empty → run checks 2-5
- `finality` → run check 2 only
- `messages` → run check 3 only
- `version-guard` → run check 4 only
- `balances` → run check 5 only

### Step 2: Check Finality Sync

For each bridge pair, verify headers are being synced by checking the most recent GitHub Actions workflow runs and relay status.

**Kusama <> Polkadot** (lane `00000001`):
- Polkadot → KusamaBridgeHub: headers should increase by ≥5000 in 25h
- Kusama → PolkadotBridgeHub: headers should increase by ≥5000 in 25h

**Rococo <> Westend** (lane `00000002`):
- Rococo → WestendBridgeHub: headers should increase by ≥500 in 120m
- Westend → RococoBridgeHub: headers should increase by ≥500 in 390m

Also check for header mismatches (source-at-target diverging from source).

**Prometheus metrics to reference** (from Grafana alert configs):
```
# Finality sync progress
{Source}_to_{Target}_Sync_best_source_at_target_block_number{domain="<domain>"}

# Fork detection
{Source}_to_{Target}_Sync_is_source_and_source_at_target_using_different_forks{domain="<domain>"}
{Source}_to_{Target}_MessageLane_{LaneId}_is_source_and_source_at_target_using_different_forks{domain="<domain>"}
```

Report for each direction:
- PASS: finality sync is progressing normally
- WARN: sync is slow but still moving
- FAIL: sync has stalled or headers are mismatched

### Step 3: Check Message Delivery

For each bridge pair and direction, verify messages are flowing.

**Metrics to reference:**
```
# Delivery: source_latest_generated vs target_latest_received
{Source}_to_{Target}_MessageLane_{LaneId}_lane_state_nonces{domain="<domain>",type="source_latest_generated"}
{Source}_to_{Target}_MessageLane_{LaneId}_lane_state_nonces{domain="<domain>",type="target_latest_received"}

# Confirmation: target_latest_received vs source_latest_confirmed
{Source}_to_{Target}_MessageLane_{LaneId}_lane_state_nonces{domain="<domain>",type="source_latest_confirmed"}

# Rewards: source_latest_confirmed vs target_latest_confirmed
{Source}_to_{Target}_MessageLane_{LaneId}_lane_state_nonces{domain="<domain>",type="target_latest_confirmed"}
```

**Thresholds** (from Grafana alerts):
- Delivery lag: alert if undelivered messages exist and no delivery in 10m
- Confirmation lag: alert if >50 unconfirmed messages
- Reward lag: alert if >10 unconfirmed rewards

Report for each direction:
- Delivery status (PASS/FAIL)
- Confirmation status (PASS/FAIL)
- Reward status (PASS/FAIL)

### Step 4: Check Version Guard

Check relay logs for "Aborting relay" messages indicating the version guard has triggered.

**Loki query (from Grafana alerts):**
```
count_over_time({container="bridges-common-relay"} |= `Aborting relay` [1m])
```

Since we may not have direct Loki access, check the **relay pod status** instead:

```bash
# Check if the relay deployment is running (not in CrashLoopBackOff)
gh run list --workflow=deploy.yml --limit=3
```

Also check the bundled spec_versions against on-chain values:

```bash
grep 'spec_version' relay-clients/client-*/src/lib.rs
```

Report:
- PASS: no version guard aborts detected, relay is running
- FAIL: version guard is aborting relay (chain was upgraded, relay needs redeployment)

### Step 5: Check Relay Balances

**Metrics to reference:**
```
# Kusama <> Polkadot
at_BridgeHubKusama_relay_BridgeHubPolkadotMessages_balance{domain="parity-chains"}
at_BridgeHubPolkadot_relay_BridgeHubKusamaMessages_balance{domain="parity-chains"}

# Rococo <> Westend
at_BridgeHubRococo_relay_BridgeHubWestendMessages_balance{domain="parity-testnet"}
at_BridgeHubWestend_relay_BridgeHubRococoMessages_balance{domain="parity-testnet"}
```

**Thresholds** (from Grafana alerts):
- KusamaBridgeHub: alert if balance < 2
- PolkadotBridgeHub: alert if balance < 10
- RococoBridgeHub: alert if balance < 10
- WestendBridgeHub: alert if balance < 10

Since direct Prometheus access may not be available, suggest the user check the Grafana dashboards:
- Kusama <> Polkadot maintenance: dashboard `UFsgpJtVz`
- Rococo <> Westend maintenance: dashboard `UFsgbJtVz`

Report:
- PASS: balances are above thresholds
- WARN: balances are getting low
- FAIL: balances are below alert thresholds

### Step 6: Report Summary

Print a summary table:

```
## Health Report — <timestamp>

| Check          | Kusama <> Polkadot | Rococo <> Westend |
|----------------|--------------------|-------------------|
| Finality Sync  | PASS/WARN/FAIL     | PASS/WARN/FAIL    |
| Messages       | PASS/WARN/FAIL     | PASS/WARN/FAIL    |
| Version Guard  | PASS/WARN/FAIL     | PASS/WARN/FAIL    |
| Balances       | PASS/WARN/FAIL     | PASS/WARN/FAIL    |

Details: <expand any WARN or FAIL items with specific information>
```

If any check is FAIL, suggest remediation:
- **Finality stalled**: Check if relay pod is running. May need redeployment via [deploy.yml](https://github.com/paritytech/parity-bridges-common/actions/workflows/deploy.yml).
- **Messages lagging**: Check finality first (messages depend on headers). If finality is fine, check relay logs for errors.
- **Version guard abort**: A chain was upgraded. Run `/release` to prepare a new relay version.
- **Balance low**: Top up relay accounts. See maintenance dashboard for current balances.

## Grafana Dashboard Reference

| Dashboard | ID | Environment |
|-----------|----|-------------|
| Kusama <> Polkadot maintenance | `UFsgpJtVz` | prod |
| Rococo <> Westend maintenance | `UFsgbJtVz` | testnet |
| Kusama → Polkadot messages | `tkpc6_bnk` | prod |
| Polkadot → Kusama messages | `zqjpkXxnk` | prod |
| Rococo → Westend messages | `tkgc6_bnk` | testnet |
| Westend → Rococo messages | `zqjpgXxnk` | testnet |
