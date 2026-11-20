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

const PHONE_PLATFORMS = ['android', 'android-emulator', 'ios'] as const;
export type PhonePlatformName = (typeof PHONE_PLATFORMS)[number];
export type AgentPlatformName = 'desktop' | PhonePlatformName;

export function isMobile(platform: AgentPlatformName): boolean {
	return platform !== 'desktop';
}

/**
 * The phones the run drives, parsed from the PHONES env var — an unordered
 * comma-separated multiset (duplicates set the phone count, order carries no
 * meaning); none when unset. Index i runs agent slot i+1. Desktop agents are
 * never listed: `setupAgents` launches one for every agent no phone fills.
 */
export function phonePlatforms(): PhonePlatformName[] {
	if (process.env.PLATFORMS !== undefined) {
		throw new Error(
			'PLATFORMS is no longer read: list only the phones, in PHONES ' +
				'(desktop agents are launched as each spec needs them)',
		);
	}
	const raw = process.env.PHONES;
	if (raw === undefined || raw === '') return [];
	const names = raw.split(',').map(name => name.trim());
	for (const name of names) {
		if (!(PHONE_PLATFORMS as readonly string[]).includes(name)) {
			throw new Error(
				`PHONES entry '${name}' is not a phone platform (expected one of: ${PHONE_PLATFORMS.join(', ')})`,
			);
		}
	}
	return names as PhonePlatformName[];
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
	if (/^https:\/\/localhost:\d+\/?$/.test(url)) return null;
	return url;
}

/** A Wi-Fi network the harness joins: a test network a run walks phones and
 *  hubs onto and off, or the host's own (see [`hostWifiOverride`]). */
export interface WifiNetwork {
	ssid: string;
	/** '' for an open network. */
	passphrase: string;
}

/** One `ssid` or `ssid:passphrase` entry of the env var `variable`; a bare
 *  `ssid` is an open network. Neither part may contain ':' or ','. */
function parseNetwork(entry: string, variable: string): WifiNetwork {
	const parts = entry.trim().split(':');
	if (parts.length > 2 || parts[0] === '') {
		throw new Error(
			`${variable} entry '${entry.trim()}' is not 'ssid' or 'ssid:passphrase'`,
		);
	}
	return { ssid: parts[0], passphrase: parts[1] ?? '' };
}

/**
 * The Wi-Fi networks a run may walk phones and hubs through, from
 * E2E_WIFI_NETWORKS as `ssid:passphrase,ssid:passphrase` in the order moves
 * index them; empty when unset. Test networks only: the host's own comes
 * from the host, or [`hostWifiOverride`].
 */
export function wifiNetworks(): WifiNetwork[] {
	const raw = process.env.E2E_WIFI_NETWORKS;
	if (raw === undefined || raw.trim() === '') return [];
	return raw.split(',').map(entry => parseNetwork(entry, 'E2E_WIFI_NETWORKS'));
}

export function testNetworkSsids(): string[] {
	return wifiNetworks().map(n => n.ssid);
}

/** The host's own Wi-Fi network from E2E_HOST_WIFI, or null when unset: for
 *  a host whose Wi-Fi the harness cannot read, as on macOS or on a wire. */
export function hostWifiOverride(): WifiNetwork | null {
	const raw = process.env.E2E_HOST_WIFI;
	if (raw === undefined || raw.trim() === '') return null;
	// Split at the first ':' and keep spaces: a real passphrase may hold both.
	const colon = raw.indexOf(':');
	const ssid = colon === -1 ? raw : raw.slice(0, colon);
	if (ssid.trim() === '') {
		throw new Error("E2E_HOST_WIFI is not 'ssid' or 'ssid:passphrase'");
	}
	return { ssid, passphrase: colon === -1 ? '' : raw.slice(colon + 1) };
}
