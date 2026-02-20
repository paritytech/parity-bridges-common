# Parity Bridges Common

Cross-chain bridge infrastructure between Substrate-based blockchains. The `substrate-relay` binary relays headers, parachain heads, and messages between chains.

## Bridge Pairs

| Bridge | Environment | Domain |
|--------|-------------|--------|
| Kusama ↔ Polkadot (BridgeHubs) | production | `parity-chains` |
| Rococo ↔ Westend (BridgeHubs) | testnet | `parity-testnet` |
| Polkadot ↔ Polkadot Bulletin | production | `parity-chains` |
| Rococo ↔ Rococo Bulletin | testnet | `parity-testnet` |

## Repository Structure

```
substrate-relay/          # Main relay binary (Rust CLI)
relay-clients/            # RPC client adapters per chain (13 crates)
chains/                   # Chain type definitions
deployments/bridges/
  kusama-polkadot/        # Production dashboards & alerts (54+ rules)
  rococo-westend/         # Testnet dashboards & alerts (17 rules)
  rococo/                 # Rococo Bulletin dashboards
  grafana-github-bridge/  # Cloudflare Worker: Grafana alerts → GitHub issues
```

## Relay Operations

The relay runs as `substrate-relay relay-headers-and-messages` (combined mode, recommended). It handles:
- **Header relay**: GRANDPA finality proofs from source → target chain
- **Parachain relay**: Parachain head proofs → target BridgeHub
- **Message relay**: Cross-chain messages via message lanes (identified by lane IDs like `00000001`, `00000002`)
- **Confirmation relay**: Delivery confirmations back to source

Version guards abort the relay if the target chain runtime is upgraded and incompatible.

## Key Metrics (Prometheus → Grafana)

| Metric pattern | What it tracks |
|----------------|----------------|
| `*_MessageLane_*_lane_state_nonces` | Message lane state (generated vs received nonces) |
| `*_Sync_best_source_at_target_block_number` | Header sync progress |
| `relay_balance_at_*` | Relay account balance on target chain |
| `relayer_version` | Running relay binary version |

Grafana instance: `https://grafana.teleport.parity.io`
Prometheus data source UID: `PC96415006F908B67`

## Alert Categories

| Category | Emoji | Meaning | Typical action |
|----------|-------|---------|----------------|
| relay-down | 🔴 | Relay process not running | Check pod status, restart |
| version-guard | ⛔ | Runtime version mismatch | Redeploy relay with new runtime |
| headers-mismatch | 🔀 | Fork detected | Resync headers from canonical fork |
| finality-lag | ⏳ | Finality proofs not advancing | Check relay logs and source chain finality |
| delivery-lag | 📦 | Messages not being delivered | Check message relay process |
| confirmation-lag | ✅ | Confirmations not relayed back | Check confirmation relay |
| reward-lag | 💰 | Rewards not claimed | Check reward mechanism and relay balance |
| low-balance | 💸 | Relay account running low | Top up the account |

## Investigating Alerts

When investigating a bridge alert:

1. **Identify the bridge pair and environment** from the issue labels and title
2. **Check the Grafana dashboard** linked in the issue body (dashboard UID in annotations)
3. **Look at metric values** included in the issue — compare nonces, block numbers, balances
4. **Review the alert rules** in `deployments/bridges/<bridge-pair>/dashboard/grafana/` for thresholds and query definitions
5. **Check relay client code** in `relay-clients/client-bridge-hub-<chain>/` for RPC endpoints and type bindings
6. **Review relay bridge logic** in `substrate-relay/src/bridges/` for the specific bridge implementation

### Grafana API Queries

If `GRAFANA_URL` and `GRAFANA_TOKEN` are available:

```bash
# Query current metric value
curl -H "Authorization: Bearer $GRAFANA_TOKEN" \
  "$GRAFANA_URL/api/datasources/proxy/uid/PC96415006F908B67/api/v1/query?query=<promql>"

# Get alert rules
curl -H "Authorization: Bearer $GRAFANA_TOKEN" \
  "$GRAFANA_URL/api/v1/provisioning/alert-rules"

# Get dashboard by UID
curl -H "Authorization: Bearer $GRAFANA_TOKEN" \
  "$GRAFANA_URL/api/dashboards/uid/<dashboard_uid>"
```

## Build & Deploy

```bash
# Build relay
cargo build --release -p substrate-relay

# Docker image (published on tagged releases)
docker pull paritytech/substrate-relay:v1.8.15

# Deploy Grafana-GitHub bridge worker
cd deployments/bridges/grafana-github-bridge && npx wrangler deploy
```
