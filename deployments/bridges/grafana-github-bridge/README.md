# grafana-github-bridge

Cloudflare Worker that converts Grafana Alertmanager webhook POSTs into GitHub issues assigned to `claude[bot]`.

```
Grafana alert fires → POST webhook → this Worker → GitHub Issue (label: claude)
```

## Deploy

```bash
npm install
npx wrangler secret put GITHUB_TOKEN      # PAT with issues:write scope
npx wrangler secret put WEBHOOK_SECRET     # optional, shared secret
npx wrangler deploy
```

The deploy prints the worker URL, e.g. `https://grafana-github-bridge.<account>.workers.dev`.

## Configure Grafana

1. **Alerting → Contact points → New contact point**
2. Name: `GitHub parity-bridges-common`
3. Integration: `Webhook`
4. URL: `https://grafana-github-bridge.<account>.workers.dev`
5. (Optional) Expand **Optional Webhook settings** → add header `Authorization: Bearer <WEBHOOK_SECRET>`
6. **Save contact point**
7. **Alerting → Notification policies** → route bridge alerts to this contact point

## Test

```bash
# Local
npx wrangler dev
WORKER_URL=http://localhost:8787 node test.js

# Production (dry run — creates a real issue)
WORKER_URL=https://grafana-github-bridge.<account>.workers.dev node test.js
```

## Monitor

- **Worker metrics**: Cloudflare dashboard → Workers → grafana-github-bridge → Metrics (request count, errors, latency)
- **Logs**: `npx wrangler tail` for real-time logs
- **GitHub side**: search `label:alert label:claude` in the repo issues
