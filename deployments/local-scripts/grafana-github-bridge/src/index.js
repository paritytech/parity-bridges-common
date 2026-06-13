/**
 * Grafana → GitHub Issue bridge.
 *
 * Receives Grafana Alertmanager webhook POSTs and creates GitHub issues
 * with the "alert" label, categorised by bridge alert type.
 *
 * Environment variables (set as Worker secrets):
 *   GITHUB_TOKEN  – GitHub PAT with `issues:write` scope
 *   WEBHOOK_SECRET – (optional) shared secret for request validation
 */

const REPO = 'paritytech/parity-bridges-common';

// ---------------------------------------------------------------------------
// Alert classification
// ---------------------------------------------------------------------------

const ALERT_CATEGORIES = [
	{
		id: 'relay-down',
		label: 'relay-down',
		match: (t) => /node is down/i.test(t),
		emoji: '🔴',
		action: 'Check relay pod status and restart if needed.',
	},
	{
		id: 'version-guard',
		label: 'version-guard',
		match: (t) => /version guard|abort/i.test(t),
		emoji: '⛔',
		action:
			'A chain was upgraded — redeploy the relay with the new runtime.',
	},
	{
		id: 'headers-mismatch',
		label: 'headers-mismatch',
		match: (t) => /headers? mismatch|different.?forks/i.test(t),
		emoji: '🔀',
		action:
			'Source chain forked — the relay may need to re-sync headers from the canonical fork.',
	},
	{
		id: 'finality-lag',
		label: 'finality-lag',
		match: (t) => /finality.*lag|sync.*lag/i.test(t),
		emoji: '⏳',
		action:
			'Finality headers are not advancing — check relay logs and source chain finality.',
	},
	{
		id: 'delivery-lag',
		label: 'delivery-lag',
		match: (t) => /delivery.*lag/i.test(t),
		emoji: '📦',
		action:
			'Messages generated but not delivered — check message relay process.',
	},
	{
		id: 'confirmation-lag',
		label: 'confirmation-lag',
		match: (t) => /confirmation.*lag/i.test(t),
		emoji: '✅',
		action:
			'Messages delivered but not confirmed back to source — check confirmation relay.',
	},
	{
		id: 'reward-lag',
		label: 'reward-lag',
		match: (t) => /reward.*lag/i.test(t),
		emoji: '💰',
		action:
			'Confirmations not being rewarded — check reward mechanism and relay balance.',
	},
	{
		id: 'low-balance',
		label: 'low-balance',
		match: (t) => /balance/i.test(t),
		emoji: '💸',
		action: 'Relay account balance is low — top up the account.',
	},
];

function classify(alertname) {
	for (const cat of ALERT_CATEGORIES) {
		if (cat.match(alertname)) return cat;
	}
	return {
		id: 'other',
		label: 'bridge-alert',
		emoji: '⚠️',
		action: null,
	};
}

// Extract environment (prod vs testnet) from labels or title
function detectEnv(alert) {
	const domain = alert.labels?.domain || '';
	const title = alert.labels?.alertname || '';
	if (domain === 'parity-testnet' || /rococo|westend/i.test(title))
		return 'testnet';
	if (domain === 'parity-chains' || /polkadot|kusama/i.test(title))
		return 'production';
	return 'unknown';
}

// Extract the bridge pair from the alert title, e.g. "Polkadot <> Kusama"
function detectBridgePair(alert) {
	const title = alert.labels?.alertname || '';
	// Match patterns like "Polkadot -> KusamaBridgeHub" or "KusamaBridgeHub <> PolkadotBridgeHub"
	const m = title.match(
		/(\w+?)(?:BridgeHub)?\s*(?:->|<>|to)\s*(\w+?)(?:BridgeHub)?[\s_]/i,
	);
	if (m) return `${m[1]} ↔ ${m[2]}`;
	return alert.labels?.bridge || null;
}

// ---------------------------------------------------------------------------
// Issue formatting
// ---------------------------------------------------------------------------

function formatTitle(alert, category) {
	const alertname = alert.labels?.alertname || 'Unknown alert';
	return `${category.emoji} [Alert] ${alertname}`;
}

function formatBody(alert, payload, category, env, bridgePair) {
	const labels = alert.labels || {};
	const annotations = alert.annotations || {};
	const values = alert.values || {};

	const lines = [
		`## ${category.emoji} ${labels.alertname || 'Alert'}`,
		'',
		`| Field | Value |`,
		`|-------|-------|`,
		`| **Status** | \`${alert.status}\` |`,
		`| **Severity** | \`${labels.severity || 'unknown'}\` |`,
		`| **Category** | \`${category.id}\` |`,
		`| **Environment** | \`${env}\` |`,
		bridgePair ? `| **Bridge** | \`${bridgePair}\` |` : null,
		`| **Started** | ${alert.startsAt || 'N/A'} |`,
		'',
	];

	if (annotations.summary) {
		lines.push(`### Summary`, '', annotations.summary, '');
	}
	if (annotations.description) {
		lines.push(`### Description`, '', annotations.description, '');
	}

	if (category.action) {
		lines.push(`### Suggested Action`, '', `> ${category.action}`, '');
	}

	if (Object.keys(values).length > 0) {
		lines.push('### Metric Values', '');
		for (const [key, val] of Object.entries(values)) {
			lines.push(`- **${key}:** \`${val}\``);
		}
		lines.push('');
	}

	// Links
	const linkLines = [];
	if (alert.generatorURL) linkLines.push(`- [Alert rule](${alert.generatorURL})`);
	if (payload.externalURL) linkLines.push(`- [Grafana](${payload.externalURL})`);
	if (annotations.__dashboardUid__) {
		const base = payload.externalURL || 'https://grafana.teleport.parity.io';
		const dashUrl = `${base}/d/${annotations.__dashboardUid__}`;
		linkLines.push(`- [Dashboard](${dashUrl})`);
	}
	if (linkLines.length) {
		lines.push('### Links', '', ...linkLines, '');
	}

	// All labels
	lines.push(
		'<details><summary>All labels</summary>',
		'',
		'```json',
		JSON.stringify(labels, null, 2),
		'```',
		'',
		'</details>',
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

// ---------------------------------------------------------------------------
// Worker
// ---------------------------------------------------------------------------

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
			const category = classify(alertname);
			const env_name = detectEnv(alert);
			const bridgePair = detectBridgePair(alert);

			const title = formatTitle(alert, category);
			const body = formatBody(alert, payload, category, env_name, bridgePair);

			const severity = alert.labels?.severity || 'warning';
			const ghLabels = ['alert', category.label];
			if (env_name === 'testnet') ghLabels.push('testnet');
			if (env_name === 'production') ghLabels.push('production');

			// Tiered model: critical/unknown → Sonnet (escalate), others → Haiku (triage)
			if (severity === 'critical' || category.id === 'other') {
				ghLabels.push('claude-escalate');
			} else {
				ghLabels.push('claude');
			}

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
						labels: ghLabels,
						assignees: [],
					}),
				},
			);

			results.push({
				alertname,
				category: category.id,
				env: env_name,
				status: resp.status,
				issue:
					resp.status === 201 ? (await resp.json()).html_url : null,
			});
		}

		return Response.json({ processed: results.length, results });
	},
};
