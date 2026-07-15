/**
 * Spawning the e2e mailbox server. Shared by wdio.conf.ts (which boots it once
 * for the whole run) and mailbox-control.ts (which respawns it after a spec
 * kills it), so the spawn command lives in exactly one place.
 */
import { type ChildProcess, spawn } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { allocatePort } from './allocate-port';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..', '..');

/** Spawn the mailbox server in its own process group on the given port + db. */
export function spawnMailboxServer(port: number, dbPath: string): ChildProcess {
	// The prebuilt binary (run-e2e.sh runs `cargo build -p mailbox-server`) is
	// spawned directly: `cargo run` here would rebuild with whatever toolchain
	// is on PATH — under the androidDev shell of an Android combo that means
	// recompiling the world and blowing the readiness timeout.
	// `detached: true` puts it in its own process group so lifecycle helpers
	// can signal -pid without touching the test runner.
	return spawn(
		path.join(ROOT, 'target', 'debug', 'mailbox-server'),
		['--db-path', dbPath, '--addr', `0.0.0.0:${port}`],
		{ cwd: ROOT, stdio: ['ignore', 'ignore', 'pipe'], detached: true },
	);
}

/**
 * Start a local mailbox server on a freshly allocated port: spawn it, echo its
 * stderr/lifecycle to the console, wait until it answers /health, expose its
 * URL via process.env.MAILBOX_URL, and persist mailbox-info.json so specs can
 * drive its lifecycle. Shared by the desktop and Android wdio configs.
 */
export async function startLocalMailboxServer(): Promise<{
	proc: ChildProcess;
	port: number;
	url: string;
}> {
	const port = allocatePort();
	const url = `http://localhost:${port}`;
	const dbPath = path.join(ROOT, '.dbs', 'e2e', 'mailbox-server', 'mailbox.db');
	mkdirSync(path.dirname(dbPath), { recursive: true });

	console.log(`Starting local mailbox server on ${url}...`);
	const proc = spawnMailboxServer(port, dbPath);
	console.log(`[mailbox-server] spawned (cargo pid=${proc.pid})`);
	proc.stderr?.on('data', (data: Buffer) => {
		console.error(`[mailbox-server] ${data.toString().trim()}`);
	});
	proc.on('exit', (code, signal) => {
		console.error(
			`[mailbox-server] EXITED code=${code} signal=${signal} at ${new Date().toISOString()}`,
		);
	});
	proc.on('error', err => {
		console.error(`[mailbox-server] ERROR ${err.message}`);
	});

	await waitForMailboxReady(url);
	process.env.MAILBOX_URL = url;
	console.log(`Mailbox server ready at ${url}`);

	writeFileSync(
		path.join(ROOT, '.dbs', 'e2e', 'mailbox-info.json'),
		JSON.stringify({ pid: proc.pid, port, url, dbPath }),
	);

	return { proc, port, url };
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
