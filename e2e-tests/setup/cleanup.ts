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

/** Pids of the processes named `name` whose environment holds `marker`.
 *  Matched by exact process name so the shell running the check can never
 *  match itself. Linux reads /proc; macOS has no /proc, but `ps -E` prints
 *  the environment of the user's own processes after the command. */
export function pidsNamedWithEnv(name: string, marker: string): number[] {
	try {
		if (process.platform === 'darwin') {
			return execSync('ps -E -axo pid=,command=', { encoding: 'utf8' })
				.split('\n')
				.filter(line => {
					const [pid, argv0] = line.trim().split(/\s+/);
					return (
						pid !== undefined &&
						argv0 !== undefined &&
						path.basename(argv0) === name &&
						line.includes(marker)
					);
				})
				.map(line => Number(line.trim().split(/\s+/)[0]));
		}
		return execSync(
			`for pid in $(pgrep -x ${name}); do ` +
				`grep -qzF ${JSON.stringify(marker)} /proc/$pid/environ 2>/dev/null && echo $pid; ` +
				'done',
			{ encoding: 'utf8' },
		)
			.split('\n')
			.filter(line => line !== '')
			.map(Number);
	} catch {
		return [];
	}
}

/** SIGKILL every process named `name` whose environment points it at this
 *  checkout's `.dbs`, whichever run launched it. */
function killOursNamed(name: string) {
	for (const pid of pidsNamedWithEnv(name, DBS)) {
		try {
			process.kill(pid, 'SIGKILL');
		} catch {
			/* already gone */
		}
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

/** Kill this checkout's E2E dash-chat processes, and the tauri-driver ones
 *  the compat suite runs (NOT the mailbox server). */
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
	// Not built here: startToxiproxy stamps this checkout's `.dbs` into its
	// environment instead.
	killOursNamed('toxiproxy-server');
}

/** Kill any process listening on the given TCP ports. */
export function killPortHolders(ports: number[]) {
	for (const p of ports) {
		try {
			execSync(
				process.platform === 'darwin'
					? `lsof -nP -iTCP:${p} -sTCP:LISTEN -t | xargs kill -9`
					: `ss -tlnp 'sport = :${p}' | grep -oP 'pid=\\K[0-9]+' | xargs -r kill -9`,
				{ stdio: 'ignore' },
			);
		} catch {
			/* ignore */
		}
	}
}
