/**
 * Spawning the e2e push-notifications server for real end-to-end push tests.
 * Parallels mailbox-server.ts. Runs only when opted in with `E2E_PUSH=1` AND a
 * Firebase service-account key is present — the in-repo default
 * (`crates/push-notifications-server/service-account-key.json`, gitignored) or
 * an `FCM_SERVICE_ACCOUNT_KEY` override; otherwise push specs skip. The key must
 * target the same Firebase project as the device's GoogleService-Info.plist.
 *
 * The mailbox server is spawned with `--push-notifications-url` pointing here so
 * blob arrivals are forwarded, and the app build bakes
 * `PUSH_NOTIFICATIONS_SERVER_URL` so the device registers its FCM token here.
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

/** In-repo default location of the Firebase service-account key (gitignored).
 * When present, real-device push tests run with no env var needed. */
const DEFAULT_SERVICE_ACCOUNT_KEY =
	'crates/push-notifications-server/service-account-key.json';

/** The service-account key path to try: the `FCM_SERVICE_ACCOUNT_KEY` override
 * when set, otherwise the in-repo default. Relative paths resolve against the
 * repo root and the result is absolute (so the existence check and the push
 * server — spawned with cwd=ROOT — agree regardless of the worker's cwd). The
 * returned path may not exist. */
function serviceAccountKeyPath(): string {
	const override = process.env.FCM_SERVICE_ACCOUNT_KEY;
	const rel =
		override !== undefined && override !== ''
			? override
			: DEFAULT_SERVICE_ACCOUNT_KEY;
	return path.resolve(ROOT, rel);
}

/** Whether the caller opted into push tests with `E2E_PUSH=1` (or `true`). */
function pushOptIn(): boolean {
	const v = (process.env.E2E_PUSH ?? '').toLowerCase();
	return v === '1' || v === 'true';
}

/** Whether the real-device push spec + push server should run: opted in via
 * `E2E_PUSH=1` AND a service-account key present (override or in-repo default).
 * Shared by the spec's skip guard and the harness build gate so they never
 * disagree. */
export function pushTestingEnabled(): boolean {
	return pushOptIn() && existsSync(serviceAccountKeyPath());
}

/** Absolute path to the Firebase service-account key, or null when none is
 * present (push tests skip). */
export function pushServiceAccountKey(): string | null {
	const candidate = serviceAccountKeyPath();
	return existsSync(candidate) ? candidate : null;
}

/** Log file the spawned server's stdout/stderr are appended to. */
export function pushServerLogFile(dbPath: string): string {
	return path.join(path.dirname(dbPath), 'push-server.log');
}

/** Run `cargo build -p push-notifications-server`, resolving when built. */
export function buildPushServer(): Promise<void> {
	return new Promise<void>((resolve, reject) => {
		const proc = spawn('cargo', ['build', '-p', 'push-notifications-server'], {
			cwd: ROOT,
			stdio: 'inherit',
		});
		proc.on('error', reject);
		proc.on('exit', code => {
			if (code === 0) resolve();
			else
				reject(
					new Error(`cargo build -p push-notifications-server exited ${code}`),
				);
		});
	});
}

/** Spawn the push-notifications server in its own process group. */
export function spawnPushServer(
	port: number,
	dbPath: string,
	serviceAccountKey: string,
): ChildProcess {
	// Prebuilt binary (built by the e2e build step); `cargo run` here would risk
	// a rebuild that blows the readiness timeout, same as the mailbox server.
	const bin = path.join(ROOT, 'target', 'debug', 'push-notifications-server');
	if (!existsSync(bin)) {
		throw new Error(
			`${bin} not found — run the suite via 'just test e2e' (which builds ` +
				`it) or 'cargo build -p push-notifications-server'`,
		);
	}
	const logFd = openSync(pushServerLogFile(dbPath), 'a');
	const server = spawn(
		bin,
		[
			'--addr',
			`0.0.0.0:${port}`,
			'--service-account-key',
			serviceAccountKey,
			'--db-path',
			dbPath,
		],
		{ cwd: ROOT, stdio: ['ignore', logFd, logFd], detached: true },
	);
	closeSync(logFd);
	return server;
}

/**
 * Start a local push-notifications server on a freshly allocated port: spawn it,
 * echo its log, wait until it accepts HTTP, expose its URL via
 * process.env.PUSH_NOTIFICATIONS_SERVER_URL, and persist push-server-info.json.
 * Returns null when no service-account key is present (push specs then skip).
 */
export async function startLocalPushServer(): Promise<{
	proc: ChildProcess;
	logger: ChildProcess;
	port: number;
	url: string;
} | null> {
	const serviceAccountKey = pushServiceAccountKey();
	if (serviceAccountKey === null) return null;

	const port = allocatePort();
	const url = `http://localhost:${port}`;
	const dbPath = path.join(ROOT, '.dbs', 'e2e', 'push-server', 'push.db');
	mkdirSync(path.dirname(dbPath), { recursive: true });

	console.log(`Starting local push-notifications server on ${url}...`);
	const logger = startAgentLogger('push-server', pushServerLogFile(dbPath));
	const proc = spawnPushServer(port, dbPath, serviceAccountKey);
	console.log(`[push-server] spawned (pid=${proc.pid})`);
	proc.on('exit', (code, signal) => {
		console.error(
			`[push-server] EXITED code=${code} signal=${signal} at ${new Date().toISOString()}`,
		);
	});
	proc.on('error', err => {
		console.error(`[push-server] ERROR ${err.message}`);
	});

	await waitForPushServerReady(url);
	process.env.PUSH_NOTIFICATIONS_SERVER_URL = url;
	console.log(`Push-notifications server ready at ${url}`);

	writeFileSync(
		path.join(ROOT, '.dbs', 'e2e', 'push-server-info.json'),
		JSON.stringify({ pid: proc.pid, port, url, dbPath }),
	);

	return { proc, logger, port, url };
}

/**
 * Poll until the server accepts HTTP. It exposes no /health route, so any
 * response (including a 404 for `/`) proves it's up; only a connection refusal
 * counts as not-yet-ready.
 */
export async function waitForPushServerReady(
	url: string,
	timeoutMs = 30_000,
): Promise<void> {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		try {
			await fetch(url);
			return;
		} catch {
			/* not up yet */
		}
		await new Promise(r => setTimeout(r, 500));
	}
	throw new Error('Push-notifications server failed to become ready');
}
