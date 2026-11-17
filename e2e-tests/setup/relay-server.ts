/**
 * The run's own iroh relay (relay-local-server), which every e2e build bakes
 * in (`E2E_RELAY_URL`). wdio.conf.ts starts it during `onPrepare` when the run
 * has a local mailbox and records its pid, so specs can take it down: with it
 * and the mailbox down, peers only reach each other directly.
 */
import { type ChildProcess, spawn } from 'node:child_process';
import {
	closeSync,
	existsSync,
	mkdirSync,
	openSync,
	readFileSync,
	writeFileSync,
} from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { startAgentLogger } from './agent-logger';
import { exe, signalGroup } from './host-process';
import { localRelayPort } from './relay';
import { waitForPortListening } from './wait-for-port';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..', '..');
const RELAY_DIR = path.join(ROOT, '.dbs', 'e2e', 'relay');
const RELAY_INFO_PATH = path.join(ROOT, '.dbs', 'e2e', 'relay-info.json');

const RELAY_LOG = path.join(RELAY_DIR, 'relay.log');

/** Start the run's relay and echo its log, for wdio.conf's `onPrepare`. */
export async function startLocalRelay(): Promise<{
	proc: ChildProcess;
	logger: ChildProcess;
}> {
	mkdirSync(RELAY_DIR, { recursive: true });
	const logger = startAgentLogger('relay', RELAY_LOG);
	try {
		return { proc: await spawnRelay(), logger };
	} catch (err) {
		logger.kill();
		throw err;
	}
}

/** Back to a running relay, whatever a spec left it in: resumed if it was
 *  suspended, started again if it died. Every spec file starts here
 *  (wdio.conf.ts), so none runs with connectivity other than it was written
 *  for. */
export async function ensureHealthyRelay(): Promise<void> {
	const pid = relayPid();
	if (isAlive(pid)) {
		signalGroup(pid, 'SIGCONT');
		return;
	}
	console.error(`[relay] pid ${pid} is gone; starting the relay again`);
	await spawnRelay();
}

/** Spawn the relay in its own process group on every interface, wait until it
 *  accepts connections, and record its pid for the helpers here. Its output
 *  goes to the log file `startLocalRelay` echoes. */
async function spawnRelay(): Promise<ChildProcess> {
	// The prebuilt binary (built by wdio.conf alongside the mailbox server):
	// `cargo run` here could rebuild with another toolchain.
	const bin = path.join(ROOT, 'target', 'debug', exe('relay-local-server'));
	if (!existsSync(bin)) {
		throw new Error(
			`${bin} not found — run the suite via 'just e2e' (which builds ` +
				`it) or 'cargo build -p relay-local-server'`,
		);
	}
	const port = localRelayPort();
	const logFd = openSync(RELAY_LOG, 'a');
	const proc = spawn(bin, ['--addr', `0.0.0.0:${port}`], {
		cwd: ROOT,
		stdio: ['ignore', logFd, logFd],
		detached: true,
	});
	closeSync(logFd);
	// A relay respawned from a spec worker outlives it.
	proc.unref();
	proc.on('exit', (code, signal) => {
		console.error(`[relay] EXITED code=${code} signal=${signal}`);
	});
	await waitForPortListening(port);
	writeFileSync(RELAY_INFO_PATH, JSON.stringify({ pid: proc.pid }));
	console.log(`Relay ready on port ${port} (pid=${proc.pid})`);
	return proc;
}

function isAlive(pid: number): boolean {
	try {
		process.kill(pid, 0);
		return true;
	} catch {
		return false;
	}
}

function relayPid(): number {
	if (!existsSync(RELAY_INFO_PATH)) {
		throw new Error(
			`no ${RELAY_INFO_PATH}: the run's relay was never started (a remote-mailbox run has none)`,
		);
	}
	return (JSON.parse(readFileSync(RELAY_INFO_PATH, 'utf-8')) as { pid: number })
		.pid;
}

/** Suspend the relay, so connections through it hang. */
export function suspendRelay(): void {
	signalGroup(relayPid(), 'SIGSTOP');
}
