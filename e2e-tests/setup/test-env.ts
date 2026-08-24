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
