/**
 * Spawning the e2e mailbox server. Shared by wdio.conf.ts (which boots it once
 * for the whole run) and mailbox-control.ts (which respawns it after a spec
 * kills it), so the spawn command lives in exactly one place.
 */
import { type ChildProcess, spawn } from 'node:child_process';
import { closeSync, openSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..', '..');

/** Log file the spawned server's stdout/stderr are appended to. */
export function mailboxLogFile(dbPath: string): string {
	return path.join(path.dirname(dbPath), 'mailbox.log');
}

/** Spawn the mailbox server in its own process group on the given port + db. */
export function spawnMailboxServer(port: number, dbPath: string): ChildProcess {
	// stdout/stderr go to a file, not pipes: the server's tracing output is on
	// stdout, and a respawned server (restartMailbox) outlives the spec worker
	// that spawned it — a pipe with no reader would eventually block its writes.
	const logFd = openSync(mailboxLogFile(dbPath), 'a');
	// `detached: true` puts the mailbox (cargo + its mailbox-server child) in its
	// OWN process group, so signalling -pid reaches the binary under `cargo run`.
	const server = spawn(
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
		{ cwd: ROOT, stdio: ['ignore', logFd, logFd], detached: true },
	);
	closeSync(logFd);
	return server;
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
