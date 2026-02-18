---
name: check-health
description: Check bridge relay health across all dimensions
metadata:
  argument-hint: "[kusama-polkadot|rococo-westend|all]"
---

# Bridge Health Check

Check the health of bridge relays by reading Grafana alert definitions and cross-referencing with the codebase.

## Usage

```
/check-health                    # Check all bridges
/check-health kusama-polkadot    # Check Kusama <> Polkadot bridge only
/check-health rococo-westend     # Check Rococo <> Westend bridge only
```

If `$ARGUMENTS` is empty or `all`, check all bridges. Otherwise check only the specified bridge.

## Procedure

### Step 1: Read Alert Definitions

Read the Grafana alert JSON files to understand current thresholds and metric names:

- **Kusama <> Polkadot**: `deployments/bridges/kusama-polkadot/dashboard/grafana/bridge-kusama-polkadot-alerts.json`
- **Rococo <> Westend**: `deployments/bridges/rococo-westend/dashboard/grafana/bridge-rococo-westend-alerts.json`
- **BEEFY**: `deployments/bridges/rococo/dashboard/grafana/rococo-beefy-alerts.json`

### Step 2: Check 7 Health Dimensions

For each bridge in scope, evaluate the following dimensions by reading the alert JSON files and cross-referencing with the codebase:

#### 1. Finality Sync Status

Check that relay chain finality proofs are being synced to the target bridge hub.

| Bridge | Metric | Window | Threshold |
|--------|--------|--------|-----------|
| Kusama-Polkadot | `increase(Polkadot_to_BridgeHubKusama_Sync_best_source_at_target_block_number[24h])` | 24h | > 5000 |
| Kusama-Polkadot | `increase(Kusama_to_BridgeHubPolkadot_Sync_best_source_at_target_block_number[24h])` | 24h | > 5000 |
| Rococo-Westend | `increase(Rococo_to_BridgeHubWestend_Sync_best_source_at_target_block_number[120m])` | 120m | > 500 |
| Rococo-Westend | `increase(Westend_to_BridgeHubRococo_Sync_best_source_at_target_block_number[390m])` | 390m | > 500 |

**Alert fires when**: block number increase falls below threshold (finality stalled).

Cross-reference with bridge implementations in `substrate-relay/src/bridges/*/` to understand which relay chain headers are synced in each direction.

#### 2. Message Delivery Lags

Check that messages generated on the source chain are being delivered to the target chain. The alert expression checks that when `source_latest_generated > target_latest_received`, the `target_latest_received` nonce is still increasing over a 10-minute window.

| Bridge | Metric Family | Lane |
|--------|--------------|------|
| KBH > PBH | `BridgeHubKusama_to_BridgeHubPolkadot_MessageLane_00000001_lane_state_nonces` | `00000001` |
| PBH > KBH | `BridgeHubPolkadot_to_BridgeHubKusama_MessageLane_00000001_lane_state_nonces` | `00000001` |
| RBH > WBH | `BridgeHubRococo_to_BridgeHubWestend_MessageLane_00000002_lane_state_nonces` | `00000002` |
| WBH > RBH | `BridgeHubWestend_to_BridgeHubRococo_MessageLane_00000002_lane_state_nonces` | `00000002` |

Nonce types: `source_latest_generated`, `target_latest_received`, `source_latest_confirmed`, `target_latest_confirmed`

**Alert fires when**: pending messages exist (`source_latest_generated > target_latest_received`) AND `target_latest_received` is not increasing.

#### 3. Message Confirmation Lags

Check that delivered messages are being confirmed back to the source chain. The gap between `target_latest_received` and `source_latest_confirmed` should stay below 50.

| Bridge | Threshold |
|--------|-----------|
| KBH > PBH, PBH > KBH | > 50 unconfirmed |
| RBH > WBH, WBH > RBH | > 50 unconfirmed |

**Alert fires when**: `target_latest_received - source_latest_confirmed > 50`

#### 4. Reward Confirmation Lags

Check that rewards are being confirmed. The gap between `source_latest_confirmed` and `target_latest_confirmed` should stay below 10.

| Bridge | Threshold |
|--------|-----------|
| All message lanes | > 10 unconfirmed rewards |

**Alert fires when**: `source_latest_confirmed - target_latest_confirmed > 10`

#### 5. Header Mismatch / Fork Detection

Check that the source chain header stored at the target matches the actual source chain header (no forks).

Metrics (should all be 0):
- `Kusama_to_BridgeHubPolkadot_Sync_is_source_and_source_at_target_using_different_forks`
- `Polkadot_to_BridgeHubKusama_Sync_is_source_and_source_at_target_using_different_forks`
- `BridgeHubKusama_to_BridgeHubPolkadot_MessageLane_00000001_is_source_and_source_at_target_using_different_forks`
- `BridgeHubPolkadot_to_BridgeHubKusama_MessageLane_00000001_is_source_and_source_at_target_using_different_forks`
- `Rococo_to_BridgeHubWestend_Sync_is_source_and_source_at_target_using_different_forks`
- `Westend_to_BridgeHubRococo_Sync_is_source_and_source_at_target_using_different_forks`
- `BridgeHubRococo_to_BridgeHubWestend_MessageLane_00000002_is_source_and_source_at_target_using_different_forks`
- `BridgeHubWestend_to_BridgeHubRococo_MessageLane_00000002_is_source_and_source_at_target_using_different_forks`

**Alert fires when**: metric value > 0

#### 6. Relay Node Liveness

Check that the relay process is running:
- Production: `up{domain="parity-chains",container="bridges-common-relay"}`
- Testnet: `up{domain="parity-testnet",container="bridges-common-relay"}`

**Alert fires when**: `up < 1`

#### 7. Version Guard Abort

Check for version guard aborts in relay logs (Loki):
- `count_over_time({container="bridges-common-relay"} |= "Aborting relay" [1m]) > 0`

**Alert fires when**: the relay detects a runtime version mismatch and aborts. This means a new release is needed.

### Step 3: Check Relay Account Balances

| Account | Metric | Threshold |
|---------|--------|-----------|
| KBH relay | `at_BridgeHubKusama_relay_BridgeHubPolkadotMessages_balance` | < 2 |
| PBH relay | `at_BridgeHubPolkadot_relay_BridgeHubKusamaMessages_balance` | < 10 |
| RBH relay | `at_BridgeHubRococo_relay_BridgeHubWestendMessages_balance` | < 10 |
| WBH relay | `at_BridgeHubWestend_relay_BridgeHubRococoMessages_balance` | < 10 |

### Step 4: Check Runtime Version Compatibility

Read the `SimpleRuntimeVersion` constants from relay-client files to verify they match the deployed on-chain versions:

- `relay-clients/client-bridge-hub-kusama/src/lib.rs`
- `relay-clients/client-bridge-hub-polkadot/src/lib.rs`
- `relay-clients/client-bridge-hub-rococo/src/lib.rs`
- `relay-clients/client-bridge-hub-westend/src/lib.rs`
- `relay-clients/client-kusama/src/lib.rs`
- `relay-clients/client-polkadot/src/lib.rs`
- `relay-clients/client-polkadot-bulletin/src/lib.rs`
- `relay-clients/client-rococo/src/lib.rs`
- `relay-clients/client-westend/src/lib.rs`

Look for the `RUNTIME_VERSION` constant containing `SimpleRuntimeVersion { spec_version, transaction_version }`.

If a chain has recently been upgraded on-chain (check governance announcements or Polkadot-JS), flag a version mismatch and recommend `/release`.

### Step 5: Output Health Report

Output a structured report with the following format:

```
## Bridge Health Report: <bridge-name>

| Dimension | Status | Details |
|-----------|--------|---------|
| Finality Sync | OK/WARN/CRIT | <metric values and thresholds> |
| Message Delivery | OK/WARN/CRIT | <nonce gaps> |
| Confirmations | OK/WARN/CRIT | <confirmation lag> |
| Rewards | OK/WARN/CRIT | <reward lag> |
| Header Mismatch | OK/WARN/CRIT | <fork detection> |
| Relay Liveness | OK/WARN/CRIT | <up status> |
| Version Guard | OK/WARN/CRIT | <abort count> |
| Balance (KBH) | OK/WARN/CRIT | <balance vs threshold> |
| Balance (PBH) | OK/WARN/CRIT | <balance vs threshold> |
| Runtime Versions | OK/WARN | <bundled vs on-chain> |

### Recommendations
- <actionable next steps if any dimension is not OK>
```

Status levels:
- **OK**: Within normal thresholds
- **WARN**: Approaching threshold or unable to verify (e.g., no live metric access)
- **CRIT**: Threshold breached, immediate action needed

### Live Metric Queries (when available)

If `GRAFANA_URL` and `GRAFANA_TOKEN` environment variables are set, query live metrics via Grafana's API:

```bash
# Query a Prometheus metric via Grafana datasource proxy
curl -s -H "Authorization: Bearer $GRAFANA_TOKEN" \
  "$GRAFANA_URL/api/datasources/proxy/1/api/v1/query?query=up{container=\"bridges-common-relay\"}"
```

Useful queries:

```bash
# Relay liveness
curl -s -H "Authorization: Bearer $GRAFANA_TOKEN" \
  "$GRAFANA_URL/api/datasources/proxy/1/api/v1/query" \
  --data-urlencode 'query=up{container="bridges-common-relay"}'

# Finality sync (Polkadot → KBH, last 24h increase)
curl -s -H "Authorization: Bearer $GRAFANA_TOKEN" \
  "$GRAFANA_URL/api/datasources/proxy/1/api/v1/query" \
  --data-urlencode 'query=increase(Polkadot_to_BridgeHubKusama_Sync_best_source_at_target_block_number[24h])'

# Message nonces (delivery lag)
curl -s -H "Authorization: Bearer $GRAFANA_TOKEN" \
  "$GRAFANA_URL/api/datasources/proxy/1/api/v1/query" \
  --data-urlencode 'query=BridgeHubPolkadot_to_BridgeHubKusama_MessageLane_00000001_lane_state_nonces'

# Relay balance at PBH
curl -s -H "Authorization: Bearer $GRAFANA_TOKEN" \
  "$GRAFANA_URL/api/datasources/proxy/1/api/v1/query" \
  --data-urlencode 'query=at_BridgeHubPolkadot_relay_BridgeHubKusamaMessages_balance'

# Grafana alert states
curl -s -H "Authorization: Bearer $GRAFANA_TOKEN" \
  "$GRAFANA_URL/api/v1/provisioning/alert-rules"
```

The datasource proxy ID (`/proxy/1/`) may vary. To discover it:
```bash
curl -s -H "Authorization: Bearer $GRAFANA_TOKEN" "$GRAFANA_URL/api/datasources" | jq '.[].id'
```

Parse the JSON response — Prometheus returns `{"data":{"result":[{"value":[timestamp, "value"]}]}}`.

### Fallback (no live access)

If `GRAFANA_URL` is not set, base the report on:
1. Metric values from the GitHub issue body (passed through from the alert webhook)
2. Whether bundled runtime versions match known on-chain versions
3. Recent git history for relevant changes (use `git log --oneline -20`)
4. Any open issues mentioning bridge alerts
