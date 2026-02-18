/**
 * Grafana → GitHub Issue bridge.
 *
 * Receives Grafana Alertmanager webhook POSTs and creates GitHub issues
 * assigned to claude[bot] with the "claude" label.
 *
 * Environment variables (set as Worker secrets):
 *   GITHUB_TOKEN  – GitHub PAT with `issues:write` scope
 *   WEBHOOK_SECRET – (optional) shared secret for request validation
 */

const REPO = 'paritytech/parity-bridges-common';

export default {
	async fetch(request, env) {
		if (request.method !== 'POST') {
			return new Response('Method not allowed', { status: 405 });
		}

		if (env.WEBHOOK_SECRET) {
			const auth = request.headers.get('Authorization');
			if (auth !== `Bearer ${env.WEBHOOK_SECRET}`) {
				return new Response('Unauthorized', { status: 401 });
			}
		}

		let payload;
		try {
			payload = await request.json();
		} catch {
			return new Response('Invalid JSON', { status: 400 });
		}

		const alerts = payload.alerts || [];
		const results = [];

		for (const alert of alerts) {
			if (alert.status !== 'firing') continue;

			const alertname = alert.labels?.alertname || 'Unknown alert';
			const title = `[Alert] ${alertname}`;
			const body = formatBody(alert, payload);

			const resp = await fetch(
				`https://api.github.com/repos/${REPO}/issues`,
				{
					method: 'POST',
					headers: {
						Authorization: `Bearer ${env.GITHUB_TOKEN}`,
						Accept: 'application/vnd.github+json',
						'User-Agent': 'grafana-github-bridge',
					},
					body: JSON.stringify({
						title,
						body,
						labels: ['alert', 'claude'],
						assignees: [],
					}),
				},
			);

			results.push({
				alertname,
				status: resp.status,
				issue: resp.status === 201 ? (await resp.json()).html_url : null,
			});
		}

		return Response.json({ processed: results.length, results });
	},
};

function formatBody(alert, payload) {
	const labels = alert.labels || {};
	const annotations = alert.annotations || {};
	const values = alert.values || {};

	const lines = [
		`**Alert:** ${labels.alertname || 'N/A'}`,
		`**Status:** ${alert.status}`,
		`**Severity:** ${labels.severity || 'unknown'}`,
		`**Bridge:** ${labels.bridge || labels.domain || 'N/A'}`,
		'',
		annotations.summary ? `**Summary:** ${annotations.summary}` : null,
		annotations.description
			? `**Description:** ${annotations.description}`
			: null,
		'',
		`**Started:** ${alert.startsAt || 'N/A'}`,
		alert.generatorURL ? `**Source:** ${alert.generatorURL}` : null,
		payload.externalURL
			? `**Grafana:** ${payload.externalURL}`
			: null,
	];

	// Include current metric values from the alert evaluation
	if (Object.keys(values).length > 0) {
		lines.push('', '### Current Metric Values', '');
		for (const [key, val] of Object.entries(values)) {
			lines.push(`- **${key}:** \`${val}\``);
		}
	}

	lines.push(
		'',
		'---',
		'',
		'<details><summary>Raw alert payload</summary>',
		'',
		'```json',
		JSON.stringify(alert, null, 2),
		'```',
		'',
		'</details>',
	);

	return lines.filter((l) => l !== null).join('\n');
}
