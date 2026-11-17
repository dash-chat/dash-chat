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

/** Spawn the relay in its own process group on every interface, wait until it
 *  accepts connections, and record its pid for the spec helpers below. */
export async function startLocalRelay(): Promise<{
	proc: ChildProcess;
	logger: ChildProcess;
}> {
	// The prebuilt binary (built by wdio.conf alongside the mailbox server):
	// `cargo run` here could rebuild with another toolchain.
	const bin = path.join(ROOT, 'target', 'debug', exe('relay-local-server'));
	if (!existsSync(bin)) {
		throw new Error(
			`${bin} not found — run the suite via 'just e2e' (which builds ` +
				`it) or 'cargo build -p relay-local-server'`,
		);
	}
	mkdirSync(RELAY_DIR, { recursive: true });
	const logFile = path.join(RELAY_DIR, 'relay.log');
	const logger = startAgentLogger('relay', logFile);
	const port = localRelayPort();
	const logFd = openSync(logFile, 'a');
	const proc = spawn(bin, ['--addr', `0.0.0.0:${port}`], {
		cwd: ROOT,
		stdio: ['ignore', logFd, logFd],
		detached: true,
	});
	closeSync(logFd);
	proc.on('exit', (code, signal) => {
		console.error(`[relay] EXITED code=${code} signal=${signal}`);
	});
	await waitForPortListening(port);
	writeFileSync(RELAY_INFO_PATH, JSON.stringify({ pid: proc.pid }));
	console.log(`Relay ready on port ${port} (pid=${proc.pid})`);
	return { proc, logger };
}

function relayPid(): number {
	return (JSON.parse(readFileSync(RELAY_INFO_PATH, 'utf-8')) as { pid: number })
		.pid;
}

/** Suspend the relay, so connections through it hang. */
export function suspendRelay(): void {
	signalGroup(relayPid(), 'SIGSTOP');
}

/** Resume a relay [`suspendRelay`] suspended. */
export function resumeRelay(): void {
	try {
		signalGroup(relayPid(), 'SIGCONT');
	} catch (err) {
		console.error(`[relay] could not resume the relay: ${String(err)}`);
	}
}
