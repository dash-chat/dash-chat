import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

/**
 * Regex patterns for the mailbox URLs the suite is allowed to run against,
 * shared with the Rust suite via `allowed-test-mailbox-url-patterns.json` at the
 * repo root.
 */
const ALLOWED_MAILBOX_URL_PATTERNS: RegExp[] = (
	JSON.parse(
		readFileSync(
			path.resolve(__dirname, '..', '..', 'allowed-test-mailbox-url-patterns.json'),
			'utf-8',
		),
	) as string[]
).map(pattern => new RegExp(pattern));

export function getSpecFileRetries(): number {
	const rawRetries = process.env.E2E_SPEC_FILE_RETRIES ?? '1';
	const retries = Number.parseInt(rawRetries, 10);
	if (Number.isNaN(retries) || retries < 0) {
		throw new Error(
			`E2E_SPEC_FILE_RETRIES must be a non-negative integer, got ${rawRetries}`,
		);
	}
	return retries;
}

const AGENT_PLATFORMS = ['desktop', 'android', 'android-emulator'] as const;
export type AgentPlatformName = (typeof AGENT_PLATFORMS)[number];

/** Platform of the given agent slot, from the AGENT_{slot} env var. */
export function agentPlatform(slot: number): AgentPlatformName {
	const name = process.env[`AGENT_${slot}`] ?? 'desktop';
	if (!(AGENT_PLATFORMS as readonly string[]).includes(name)) {
		throw new Error(
			`AGENT_${slot}=${name} is not a valid platform (expected one of: ${AGENT_PLATFORMS.join(', ')})`,
		);
	}
	return name as AgentPlatformName;
}

/**
 * Remote mailbox URL the suite should run against, taken from MAILBOX_URL when
 * it matches an allowlisted pattern, or null when unset — in which case the
 * suite spawns its own local mailbox server. Throws on a non-allowlisted URL.
 */
export function remoteMailboxUrl(): string | null {
	const url = process.env.MAILBOX_URL;
	if (url === undefined || url === '') return null;
	if (!ALLOWED_MAILBOX_URL_PATTERNS.some(pattern => pattern.test(url))) {
		throw new Error(`MAILBOX_URL=${url} is not an allowed test mailbox`);
	}
	return url;
}
