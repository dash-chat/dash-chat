/**
 * Drive the global mailbox server's lifecycle from inside a spec so tests can
 * exercise the offline-UX state transitions.
 *
 * wdio.conf.ts spawns the mailbox server in its own process group (`detached:
 * true`) during `onPrepare` and writes its pid + port to a JSON file. These
 * helpers use that pid to signal the whole group.
 *
 * Unix-only — relies on POSIX signal semantics.
 */
import {
	existsSync,
	readdirSync,
	readFileSync,
	writeFileSync,
} from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { spawnMailboxServer, waitForMailboxReady } from './mailbox-server';

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
	url: string;
	/** Set when the suite runs against a remote environment mailbox (MAILBOX_URL). */
	remote?: boolean;
	pid?: number;
	port?: number;
	dbPath?: string;
}

function readInfo(): MailboxInfo {
	return JSON.parse(readFileSync(MAILBOX_INFO_PATH, 'utf-8')) as MailboxInfo;
}

/**
 * True when the suite runs against a remote environment mailbox, whose
 * lifecycle specs cannot control. Specs using the helpers below should skip
 * themselves when this returns true.
 */
export function isRemoteMailbox(): boolean {
	return readInfo().remote === true;
}

function localInfo(): {
	pid: number;
	port: number;
	url: string;
	dbPath: string;
} {
	const { remote, pid, port, url, dbPath } = readInfo();
	if (
		remote === true ||
		pid === undefined ||
		port === undefined ||
		dbPath === undefined
	) {
		throw new Error(
			'mailbox lifecycle control is unavailable against a remote environment mailbox (MAILBOX_URL)',
		);
	}
	return { pid, port, url, dbPath };
}

function mailboxBlobsDir(): string {
	const { dbPath } = localInfo();
	return path.join(path.dirname(dbPath), 'mailbox_blobs', 'data');
}

/**
 * Absolute paths of every blob the local mailbox holds. Note iroh-blobs keeps
 * blobs below its inline threshold in the database instead, so small ones never
 * appear here.
 */
export function mailboxBlobs(): string[] {
	const dir = mailboxBlobsDir();
	if (!existsSync(dir)) return [];
	return readdirSync(dir)
		.filter(f => f.endsWith('.data'))
		.map(f => path.join(dir, f));
}

function signalGroup(pid: number, sig: NodeJS.Signals): void {
	process.kill(-pid, sig);
}

/** Suspend the mailbox server so all HTTP traffic to it hangs/times out. */
export function suspendMailbox(): void {
	signalGroup(localInfo().pid, 'SIGSTOP');
}

/** Resume a previously-suspended mailbox server. */
export function resumeMailbox(): void {
	signalGroup(localInfo().pid, 'SIGCONT');
}

/** Kill the mailbox server outright so connections to it are refused. */
export function killMailbox(): void {
	try {
		signalGroup(localInfo().pid, 'SIGKILL');
	} catch {
		/* already gone */
	}
}

/**
 * Respawn the mailbox server on the same port + db, so a spec that killed it
 * leaves a live server behind for the rest of the run. Updates mailbox-info.json
 * with the new pid.
 */
export async function restartMailbox(): Promise<void> {
	const info = localInfo();
	const server = spawnMailboxServer(info.port, info.dbPath);
	server.unref();
	writeFileSync(
		MAILBOX_INFO_PATH,
		JSON.stringify({ ...info, pid: server.pid }),
	);
	await waitForMailboxReady(info.url);
}
