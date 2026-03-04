# Grafana → GitHub Alert Bridge

Receives Grafana Alertmanager webhook POSTs and creates categorized GitHub issues in `paritytech/parity-bridges-common`.

## Quick start

```bash
# Run directly
GITHUB_TOKEN=ghp_xxx node src/index.js

# Or with Docker
docker build -t grafana-github-bridge .
docker run -p 3000:3000 \
  -e GITHUB_TOKEN=ghp_xxx \
  -e WEBHOOK_SECRET=optional-secret \
  grafana-github-bridge
```

## Environment variables

| Variable | Required | Description |
|----------|----------|-------------|
| `GITHUB_TOKEN` | Yes | GitHub PAT with `issues:write` scope |
| `WEBHOOK_SECRET` | No | Shared secret — if set, requests must include `Authorization: Bearer <secret>` |
| `PORT` | No | Listen port (default: `3000`) |

## Endpoints

- `POST /` — Grafana webhook receiver
- `GET /health` — Health check (returns `{"status":"ok"}`)

## Grafana configuration

Add a contact point of type **Webhook** with:
- **URL**: `http://<host>:3000/`
- **HTTP Method**: `POST`
- If `WEBHOOK_SECRET` is set, add header: `Authorization: Bearer <your-secret>`

## Testing

```bash
GITHUB_TOKEN=fake PORT=9876 node test.js
```

## Alert classification

Alerts are classified by their `alertname` label into categories: `relay-down`, `version-guard`, `headers-mismatch`, `finality-lag`, `delivery-lag`, `confirmation-lag`, `reward-lag`, `low-balance`, or `other`.

Critical alerts and unclassified alerts get the `claude-escalate` label; others get the `claude` label.
