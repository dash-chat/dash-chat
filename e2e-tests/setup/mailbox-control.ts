/**
 * Drive the global mailbox server's lifecycle from inside a spec so tests can
 * exercise the offline-UX state transitions.
 *
 * wdio.conf.ts spawns the mailbox server in its own process group (`detached:
 * true`) during `onPrepare` and writes its pid + port to a JSON file. These
 * helpers use that pid to signal the whole group, so SIGSTOP/SIGCONT reach
 * the `mailbox-server` binary under the `cargo run` wrapper.
 *
 * Unix-only — relies on POSIX signal semantics.
 */
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const MAILBOX_INFO_PATH = path.join(
	__dirname,
	'..',
	'..',
	'.dbs',
	'e2e',
	'mailbox-info.json',
);

interface MailboxInfo {
	pid: number;
	port: number;
	url: string;
	dbPath: string;
}

function readInfo(): MailboxInfo {
	return JSON.parse(readFileSync(MAILBOX_INFO_PATH, 'utf-8')) as MailboxInfo;
}

function signalGroup(pid: number, sig: NodeJS.Signals): void {
	// Negative pid → signal the whole process group, which includes the
	// `mailbox-server` child spawned by `cargo run`.
	process.kill(-pid, sig);
}

/** Suspend the mailbox server so all HTTP traffic to it hangs/times out. */
export function suspendMailbox(): void {
	signalGroup(readInfo().pid, 'SIGSTOP');
}

/** Resume a previously-suspended mailbox server. */
export function resumeMailbox(): void {
	signalGroup(readInfo().pid, 'SIGCONT');
}
