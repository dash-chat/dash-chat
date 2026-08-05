/**
 * Spawning the e2e mailbox server. Shared by wdio.conf.ts (which boots it once
 * for the whole run) and mailbox-control.ts (which respawns it after a spec
 * kills it), so the spawn command lives in exactly one place.
 */
import { type ChildProcess, spawn } from 'node:child_process';
import {
	closeSync,
	existsSync,
	mkdirSync,
	openSync,
	writeFileSync,
} from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { startAgentLogger } from './agent-logger';
import { allocatePort } from './allocate-port';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..', '..');

/** Log file the spawned server's stdout/stderr are appended to. */
export function mailboxLogFile(dbPath: string): string {
	return path.join(path.dirname(dbPath), 'mailbox.log');
}

/** Run `cargo build -p mailbox-server`, resolving when the binary is built. */
export function buildMailboxServer(): Promise<void> {
	return new Promise<void>((resolve, reject) => {
		const proc = spawn('cargo', ['build', '-p', 'mailbox-server'], {
			cwd: ROOT,
			stdio: 'inherit',
		});
		proc.on('error', reject);
		proc.on('exit', code => {
			if (code === 0) resolve();
			else reject(new Error(`cargo build -p mailbox-server exited ${code}`));
		});
	});
}

/** Spawn the mailbox server in its own process group on the given port + db.
 * When `pushNotificationsUrl` is given, the server forwards blob arrivals to
 * that push-notifications server (real end-to-end push tests). */
export function spawnMailboxServer(
	port: number,
	dbPath: string,
	pushNotificationsUrl?: string,
): ChildProcess {
	// The prebuilt binary (built by wdio.conf's onPrepare) is spawned
	// directly: `cargo run` here would rebuild with whatever toolchain is on
	// PATH — under the androidDev shell of an Android combo that means
	// recompiling the world and blowing the readiness timeout.
	const bin = path.join(ROOT, 'target', 'debug', 'mailbox-server');
	if (!existsSync(bin)) {
		throw new Error(
			`${bin} not found — run the suite via 'just test e2e' (which builds ` +
				`it) or 'cargo build -p mailbox-server'`,
		);
	}
	// stdout/stderr go to a file, not pipes: the server's tracing output is on
	// stdout, and a respawned server (restartMailbox) outlives the spec worker
	// that spawned it — a pipe with no reader would eventually block its writes.
	const logFd = openSync(mailboxLogFile(dbPath), 'a');
	const args = ['--db-path', dbPath, '--addr', `0.0.0.0:${port}`];
	if (pushNotificationsUrl !== undefined) {
		args.push('--push-notifications-url', pushNotificationsUrl);
	}
	// `detached: true` puts it in its own process group so lifecycle helpers
	// can signal -pid without touching the test runner.
	const server = spawn(bin, args, {
		cwd: ROOT,
		stdio: ['ignore', logFd, logFd],
		detached: true,
	});
	closeSync(logFd);
	return server;
}

/**
 * Start a local mailbox server on a freshly allocated port: spawn it, echo its
 * log file + lifecycle to the console, wait until it answers /health, expose
 * its URL via process.env.MAILBOX_URL, and persist mailbox-info.json so specs
 * can drive its lifecycle. Shared by the desktop and Android wdio configs.
 */
export async function startLocalMailboxServer(
	pushNotificationsUrl?: string,
): Promise<{
	proc: ChildProcess;
	logger: ChildProcess;
	port: number;
	url: string;
}> {
	const port = allocatePort();
	const url = `http://localhost:${port}`;
	const dbPath = path.join(ROOT, '.dbs', 'e2e', 'mailbox-server', 'mailbox.db');
	mkdirSync(path.dirname(dbPath), { recursive: true });

	console.log(`Starting local mailbox server on ${url}...`);
	// Tail the server's log file (its tracing output, redirected there by
	// spawnMailboxServer) and echo it with a prefix, like the agent logs.
	const logger = startAgentLogger('mailbox-server', mailboxLogFile(dbPath));
	const proc = spawnMailboxServer(port, dbPath, pushNotificationsUrl);
	console.log(`[mailbox-server] spawned (pid=${proc.pid})`);
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
		JSON.stringify({ pid: proc.pid, port, url, dbPath, pushNotificationsUrl }),
	);

	return { proc, logger, port, url };
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
