/**
 * Spawning the e2e mailbox server. Shared by wdio.conf.ts (which boots it once
 * for the whole run) and mailbox-control.ts (which respawns it after a spec
 * kills it), so the spawn command lives in exactly one place.
 */
import { type ChildProcess, spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..', '..');

/** Spawn the mailbox server in its own process group on the given port + db. */
export function spawnMailboxServer(port: number, dbPath: string): ChildProcess {
	// `detached: true` puts the mailbox (cargo + its mailbox-server child) in its
	// OWN process group, so signalling -pid reaches the binary under `cargo run`.
	return spawn(
		'cargo',
		[
			'run',
			'-p',
			'mailbox-server',
			'--',
			'--db-path',
			dbPath,
			'--addr',
			`0.0.0.0:${port}`,
		],
		{ cwd: ROOT, stdio: ['ignore', 'ignore', 'pipe'], detached: true },
	);
}

/** Poll the server's /health until it answers, or throw after `timeoutMs`. */
export async function waitForMailboxReady(
	url: string,
	timeoutMs = 30_000,
): Promise<void> {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		try {
			const res = await fetch(`${url}/health`);
			if (res.ok) return;
		} catch {
			/* not up yet */
		}
		await new Promise(r => setTimeout(r, 500));
	}
	throw new Error('Mailbox server failed to become ready');
}
