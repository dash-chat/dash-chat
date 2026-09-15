/**
 * Process cleanup, scoped to this checkout: another checkout's run may be
 * driving its own agents, mailbox and drivers on the same machine, and must
 * be left alone. An app or driver this harness launched carries a DATA_DIR
 * under this checkout's `.dbs`; a server it launched runs from this
 * checkout's `target/debug`.
 */
import { type ChildProcess, execSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..', '..');
const DBS = path.join(ROOT, '.dbs') + path.sep;

/** Kill a child process with SIGKILL and wait for it to exit (up to timeoutMs). */
export function killAndWait(
	proc: ChildProcess | undefined,
	timeoutMs = 5_000,
): Promise<void> {
	if (!proc || proc.exitCode !== null) return Promise.resolve();
	return new Promise(resolve => {
		const timer = setTimeout(() => {
			resolve();
		}, timeoutMs);
		proc.once('exit', () => {
			clearTimeout(timer);
			resolve();
		});
		try {
			proc.kill('SIGKILL');
		} catch {
			clearTimeout(timer);
			resolve();
		}
	});
}

/** SIGKILL every process named `name` whose environment points it at this
 *  checkout's `.dbs`, whichever run launched it. Matched by exact process
 *  name so the shell running the loop can never match itself. */
function killOursNamed(name: string) {
	try {
		execSync(
			`for pid in $(pgrep -x ${name}); do ` +
				`grep -qzF ${JSON.stringify(DBS)} /proc/$pid/environ 2>/dev/null && kill -9 $pid 2>/dev/null; ` +
				'done',
			{ stdio: 'ignore' },
		);
	} catch {
		/* ignore */
	}
}

/** SIGKILL every process running this checkout's build of `binary`. */
function killOursBuiltFrom(binary: string) {
	try {
		execSync(
			`pkill -9 -f ${JSON.stringify(path.join(ROOT, 'target', 'debug', binary))}`,
			{ stdio: 'ignore' },
		);
	} catch {
		/* ignore */
	}
}

/** Kill this checkout's E2E dash-chat and tauri-driver processes (NOT the
 *  mailbox server). */
export function killAllE2EProcesses() {
	killOursNamed('tauri-driver');
	killOursNamed('dash-chat');
}

/** Kill this checkout's leftover mailbox, hub and push servers from previous
 *  interrupted runs. */
export function killLeftoverMailboxServers() {
	killOursBuiltFrom('mailbox-server');
	killOursBuiltFrom('mailbox-local-server');
	killOursBuiltFrom('push-notifications-server');
}

/** Kill any process listening on the given TCP ports. */
export function killPortHolders(ports: number[]) {
	for (const p of ports) {
		try {
			execSync(
				`ss -tlnp 'sport = :${p}' | grep -oP 'pid=\\K[0-9]+' | xargs -r kill -9`,
				{ stdio: 'ignore' },
			);
		} catch {
			/* ignore */
		}
	}
}
