# Kusama Bridge Hub <> Polkadot Bridge Hub deployments

This folder contains some information and useful stuff from our other test deployment - between Kusama and Polkadot
bridge hubs. The code and other helpful information can be found in
[this document](https://github.com/paritytech/polkadot-sdk/blob/master/bridges/docs/polkadot-kusama-bridge-overview.md)
and in the [code](https://github.com/polkadot-fellows/runtimes/tree/main/system-parachains/bridge-hubs).

## Grafana Alerts and Dashboards

JSON model for Grafana alerts and dashobards that we use, may be found in the [dasboard/grafana](./dashboard/grafana/)
folder.

**Dashboards:**
- kusama-polkadot-maintenance-dashboard.json
- relay-kusama-to-polkadot-messages-dashboard.json
- relay-polkadot-to-kusama-messages-dashboard.json

(exported JSON directly from https://grafana.teleport.parity.io/dashboards/f/eblDiw17z/Bridges)

**Alerts:**
- bridge-kusama-polkadot-alerts.json https://grafana.teleport.parity.io/alerting/list

_Note: All json files are formatted with `jq . file.json`._
