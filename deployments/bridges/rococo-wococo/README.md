# Rococo Bridge Hub <> Wococo Bridge Hub deployments

This folder contains some information and useful stuff from our other test deployment - between Rococo and Wococo
bridge hubs. The bridge overview and other helpful information can be found in
[this readme](https://github.com/paritytech/cumulus/tree/bridge-hub-rococo-wococo/parachains/runtimes/bridge-hubs).

## Grafana Alerts and Dashboards

JSON model for Grafana alerts and dashobards that we use, may be found in the [dasboard/grafana](./dashboard/grafana/)
folder.

**Dashboards:**
- rococo-wococo-maintenance-dashboard.json
- relay-rococo-to-wococo-messages-dashboard.json
- relay-wococo-to-rococo-messages-dashboard.json

(exported JSON directly from https://grafana.teleport.parity.io/dashboards/f/eblDiw17z/Bridges)

**Alerts:**
- bridge-rococo-wococo-alerts.json https://grafana.teleport.parity.io/api/ruler/grafana/api/v1/rules/Bridges/Bridge%20Rococo%20%3C%3E%20Wococo

_Note: All json files are formatted with `jq . file.json`._
