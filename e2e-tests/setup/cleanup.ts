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

/** Kill all E2E dash-chat and tauri-driver processes (NOT the mailbox server). */
export function killAllE2EProcesses() {
	try {
		execSync('pkill -9 tauri-driver', { stdio: 'ignore' });
	} catch {
		/* ignore */
	}
	try {
		execSync(
			'for pid in $(pgrep -f "target/(debug|release)/dash-chat"); do ' +
				'grep -qz "\\.dbs/e2e\\|\\.dbs/compat" /proc/$pid/environ 2>/dev/null && kill -9 $pid 2>/dev/null; ' +
				'done',
			{ stdio: 'ignore' },
		);
	} catch {
		/* ignore */
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
