import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const ALLOWED_TEST_ENVS = ['testing'];

/**
 * Mailbox URL for the deployment environment named by DASHCHAT_TEST_ENV,
 * resolved from the repo-root `.env.<name>` file, or null when the var is
 * unset — in which case the suite spawns its own local mailbox server.
 * Throws on a non-allowlisted environment.
 */
export function testEnvMailboxUrl(): string | null {
	const name = process.env.DASHCHAT_TEST_ENV;
	if (name === undefined || name === '') return null;
	if (!ALLOWED_TEST_ENVS.includes(name)) {
		throw new Error(
			`DASHCHAT_TEST_ENV=${name} is not an allowed test environment (allowed: ${ALLOWED_TEST_ENVS.join(', ')})`,
		);
	}
	const file = path.resolve(__dirname, '..', '..', `.env.${name}`);
	const vars = new Map<string, string>();
	for (const line of readFileSync(file, 'utf-8').split('\n')) {
		const trimmed = line.trim();
		if (trimmed === '' || trimmed.startsWith('#')) continue;
		const eq = trimmed.indexOf('=');
		if (eq === -1) continue;
		const key = trimmed.slice(0, eq).trim();
		let value = trimmed.slice(eq + 1).trim();
		for (const [k, v] of vars) {
			value = value.split(`\${${k}}`).join(v);
		}
		vars.set(key, value);
	}
	const url = vars.get('MAILBOX_URL');
	if (url === undefined) throw new Error(`no MAILBOX_URL in ${file}`);
	return url;
}
