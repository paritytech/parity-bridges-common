/**
 * Smoke test — starts the server, sends a fake Grafana webhook, verifies response.
 * Does NOT create a real GitHub issue (would need GITHUB_TOKEN).
 */

import http from 'node:http';
import { spawn } from 'node:child_process';

const PORT = 9876;

const fakePayload = {
	receiver: 'webhook',
	status: 'firing',
	alerts: [
		{
			status: 'firing',
			labels: {
				alertname: 'Polkadot -> KusamaBridgeHub finality sync lag',
				severity: 'warning',
				domain: 'parity-chains',
			},
			annotations: {
				summary: 'Finality headers are lagging behind.',
			},
			startsAt: '2025-01-01T00:00:00Z',
			values: { lag: '42' },
		},
		{
			status: 'resolved',
			labels: { alertname: 'should-be-skipped' },
		},
	],
	externalURL: 'https://grafana.example.com',
};

async function run() {
	const server = spawn('node', ['src/index.js'], {
		env: { ...process.env, PORT: String(PORT), GITHUB_TOKEN: 'fake-token' },
		stdio: ['pipe', 'pipe', 'inherit'],
	});

	// Wait for server to start
	await new Promise((resolve) => {
		server.stdout.on('data', (data) => {
			if (data.toString().includes('listening')) resolve();
		});
	});

	try {
		// Test health endpoint
		const healthRes = await fetch(`http://localhost:${PORT}/health`);
		console.assert(healthRes.status === 200, 'Health check should return 200');
		const healthBody = await healthRes.json();
		console.assert(healthBody.status === 'ok', 'Health body should be {status: "ok"}');
		console.log('✓ Health endpoint OK');

		// Test 405 for GET on /
		const getRes = await fetch(`http://localhost:${PORT}/`);
		console.assert(getRes.status === 405, 'GET / should return 405');
		console.log('✓ GET / returns 405');

		// Test webhook (will fail at GitHub API call, but we verify classification works)
		const res = await fetch(`http://localhost:${PORT}/`, {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify(fakePayload),
		});

		const body = await res.json();
		console.log('Response:', JSON.stringify(body, null, 2));

		// Should have processed 1 alert (the resolved one is skipped)
		console.assert(body.processed === 1, `Expected 1 processed, got ${body.processed}`);

		const result = body.results[0];
		console.assert(result.category === 'finality-lag', `Expected finality-lag, got ${result.category}`);
		console.assert(result.env === 'production', `Expected production, got ${result.env}`);
		// GitHub API call will fail with fake token — that's expected
		console.assert(result.status !== 201, 'Should not have created a real issue with fake token');

		console.log('✓ All tests passed');
	} finally {
		server.kill();
	}
}

run().catch((err) => {
	console.error('Test failed:', err);
	process.exit(1);
});
