/**
 * Quick smoke test — sends a fake Grafana alert payload to the worker.
 *
 * Usage:
 *   WORKER_URL=http://localhost:8787 node test.js
 *   WORKER_URL=https://grafana-github-bridge.<account>.workers.dev node test.js
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
				alertname: 'Polkadot -> KusamaBridgeHub finality sync lags (00000001)',
				severity: 'critical',
				domain: 'parity-chains',
				bridge: 'kusama-polkadot',
			},
			annotations: {
				summary: 'Polkadot finality headers not advancing at KusamaBridgeHub',
				description:
					'increase(Polkadot_to_BridgeHubKusama_Sync_best_source_at_target_block_number[24h]) < 5000',
			},
			startsAt: new Date().toISOString(),
			generatorURL: 'https://grafana.teleport.parity.io/alerting/list',
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
		console.log(await r.json());
	})
	.catch(console.error);
