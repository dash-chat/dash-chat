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
