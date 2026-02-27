/**
 * Smoke test — sends fake Grafana alert payloads to the worker.
 *
 * Usage:
 *   WORKER_URL=http://localhost:8787 node test.js
 *   WORKER_URL=https://grafana-github-bridge.parity-bridges.workers.dev node test.js
 */

const url = process.env.WORKER_URL || 'http://localhost:8787';
const secret = process.env.WEBHOOK_SECRET;

const payload = {
	receiver: 'github-parity-bridges-common',
	status: 'firing',
	alerts: [
		{
			status: 'firing',
			labels: {
				alertname:
					'Polkadot -> KusamaBridgeHub finality sync lags (00000001)',
				severity: 'critical',
				domain: 'parity-chains',
			},
			annotations: {
				summary:
					'Less than 5000 Polkadot headers (~1/2 era) have been synced to KusamaBridgeHub in last 25 hours. Relay is not running?',
				__dashboardUid__: 'zqjpkXxnk',
				__panelId__: '2',
			},
			values: {
				A: '312',
				C: '312',
			},
			startsAt: new Date().toISOString(),
			generatorURL:
				'https://grafana.teleport.parity.io/alerting/list',
		},
		{
			status: 'firing',
			labels: {
				alertname:
					'KusamaBridgeHub <> PolkadotBridgeHub relay (00000001) node is down',
				severity: 'critical',
				domain: 'parity-chains',
				container: 'bridges-common-relay',
			},
			annotations: {
				summary:
					'KusamaBridgeHub <> PolkadotBridgeHub relay (00000001) node is down',
				__dashboardUid__: 'UFsgpJtVz',
				__panelId__: '16',
			},
			values: { A: '0' },
			startsAt: new Date().toISOString(),
			generatorURL:
				'https://grafana.teleport.parity.io/alerting/list',
		},
		{
			status: 'firing',
			labels: {
				alertname: 'Relay balances at PolkadotBridgeHub',
				severity: 'warning',
				domain: 'parity-chains',
			},
			annotations: {
				summary:
					'Relay balance at PolkadotBridgeHub is getting low',
			},
			values: { A: '1.23' },
			startsAt: new Date().toISOString(),
		},
		{
			status: 'firing',
			labels: {
				alertname:
					'Version guard has aborted RococoBridgeHub <> WestendBridgeHub relay (00000002)',
				severity: 'critical',
				domain: 'parity-testnet',
			},
			annotations: {
				summary:
					'The RococoBridgeHub <> WestendBridgeHub relay (00000002) has been aborted by version guard',
			},
			startsAt: new Date().toISOString(),
		},
		{
			status: 'resolved',
			labels: {
				alertname: 'Should be skipped (resolved)',
			},
		},
	],
	externalURL: 'https://grafana.teleport.parity.io',
};

const headers = { 'Content-Type': 'application/json' };
if (secret) headers['Authorization'] = `Bearer ${secret}`;

fetch(url, {
	method: 'POST',
	headers,
	body: JSON.stringify(payload),
})
	.then(async (r) => {
		console.log(`Status: ${r.status}`);
		const data = await r.json();
		console.log(JSON.stringify(data, null, 2));
		console.log(
			`\nProcessed ${data.processed} alerts (1 resolved skipped)`,
		);
		for (const r of data.results) {
			console.log(
				`  [${r.category}] ${r.env} — ${r.alertname} → ${r.issue || `HTTP ${r.status}`}`,
			);
		}
	})
	.catch(console.error);
