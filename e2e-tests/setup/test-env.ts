export function getSpecFileRetries(): number {
	const rawRetries =
		process.env.E2E_SPEC_FILE_RETRIES ??
		(process.env.CI === 'true' ? '1' : '0');
	const retries = Number.parseInt(rawRetries, 10);
	if (Number.isNaN(retries) || retries < 0) {
		throw new Error(
			`E2E_SPEC_FILE_RETRIES must be a non-negative integer, got ${rawRetries}`,
		);
	}
	return retries;
}

const AGENT_PLATFORMS = [
	'desktop',
	'android',
	'android-emulator',
	'ios',
] as const;
export type AgentPlatformName = (typeof AGENT_PLATFORMS)[number];

/**
 * Platforms of the launched agents, parsed from the PLATFORMS env var — an
 * unordered comma-separated multiset (duplicates set the agent count, order
 * carries no meaning). Index i runs agent slot i+1.
 */
export function platformNames(): AgentPlatformName[] {
	const raw = process.env.PLATFORMS;
	const names = (raw === undefined || raw === '' ? 'desktop,desktop' : raw)
		.split(',')
		.map(name => name.trim());
	for (const name of names) {
		if (!(AGENT_PLATFORMS as readonly string[]).includes(name)) {
			throw new Error(
				`PLATFORMS entry '${name}' is not a valid platform (expected one of: ${AGENT_PLATFORMS.join(', ')})`,
			);
		}
	}
	return names as AgentPlatformName[];
}

/**
 * Remote mailbox URL the suite should run against, taken from MAILBOX_URL, or
 * null when unset or when it names the suite's own locally-spawned server — in
 * which case the suite runs against a local mailbox server.
 */
export function remoteMailboxUrl(): string | null {
	const url = process.env.MAILBOX_URL;
	if (url === undefined || url === '') return null;
	// In local mode onPrepare exports the spawned server's own URL, and worker
	// processes inherit it — that's not a remote mailbox.
	if (/^http:\/\/localhost:\d+\/?$/.test(url)) return null;
	return url;
}

export interface WifiNetwork {
	ssid: string;
	/** '' for an open network. */
	passphrase: string;
	/** The host's own LAN rather than a lab one: the host is on it as a
	 *  matter of course, and a run never leaves, forgets or deletes it. */
	home: boolean;
}

/**
 * The Wi-Fi networks a run may walk phones and hubs through, from
 * E2E_WIFI_NETWORKS as `ssid:passphrase,ssid:passphrase` in the order moves
 * index them — a bare `ssid` is an open network; empty when unset. Neither
 * part may contain ':' or ','.
 */
export function wifiNetworks(): WifiNetwork[] {
	const raw = process.env.E2E_WIFI_NETWORKS;
	if (raw === undefined || raw.trim() === '') return [];
	return raw.split(',').map(entry => {
		const parts = entry.trim().split(':');
		if (parts.length > 2 || parts[0] === '') {
			throw new Error(
				`E2E_WIFI_NETWORKS entry '${entry.trim()}' is not 'ssid' or 'ssid:passphrase'`,
			);
		}
		return { ssid: parts[0], passphrase: parts[1] ?? '', home: false };
	});
}
