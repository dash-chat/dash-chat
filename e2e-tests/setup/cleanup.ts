import { type ChildProcess, execSync } from 'node:child_process';

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

/**
 * Kill orphan dash-chat E2E processes (NOT the mailbox server).
 *
 * In parallel-worker mode, pass `dataDirFilter` (e.g. `"worker-0-0"`) to scope
 * the sweep to a single worker so we don't kill peer workers' agents. Without
 * a filter, every E2E dash-chat under `.dbs/e2e` or `.dbs/compat` is reaped.
 *
 * NOTE: this also kills any leftover `tauri-driver` processes. tauri-driver
 * exposes no env hint we can filter on, so callers that need per-worker
 * isolation must kill their own driver child processes by PID before falling
 * back to this helper.
 */
export function killAllE2EProcesses(dataDirFilter?: string) {
	const grepPattern = dataDirFilter
		? // Match the worker dir inside .dbs/e2e but stay defensive against shell
			// meta-chars: dataDirFilter is constructed in-repo, but keep it simple.
			`\\.dbs/e2e/${dataDirFilter}`
		: '\\.dbs/e2e\\|\\.dbs/compat';
	try {
		execSync(
			`for pid in $(pgrep -f "target/(debug|release)/dash-chat"); do ` +
				`grep -qz "${grepPattern}" /proc/$pid/environ 2>/dev/null && kill -9 $pid 2>/dev/null; ` +
				`done`,
			{ stdio: 'ignore' },
		);
	} catch {
		/* ignore */
	}
	// Only the unscoped sweep should nuke every tauri-driver. Per-worker callers
	// are responsible for killing their own driver child processes by PID.
	if (!dataDirFilter) {
		try {
			execSync('pkill -9 tauri-driver', { stdio: 'ignore' });
		} catch {
			/* ignore */
		}
	}
}

/** Kill leftover mailbox-server processes from previous interrupted runs. */
export function killLeftoverMailboxServers() {
	try {
		execSync('pkill -9 -f mailbox-server', { stdio: 'ignore' });
	} catch {
		/* ignore */
	}
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
