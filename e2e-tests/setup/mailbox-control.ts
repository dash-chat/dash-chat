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
import { existsSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import {
	MAILBOX_LINK,
	spawnMailboxServer,
	waitForMailboxReady,
} from './mailbox-server';
import { remoteMailboxUrl } from './test-env';
import { Link } from './toxiproxy';

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
	pid: number;
	port: number;
	bindPort: number;
	dbPath: string;
	pushNotificationsUrl?: string;
}

function readInfo(): MailboxInfo {
	return JSON.parse(readFileSync(MAILBOX_INFO_PATH, 'utf-8')) as MailboxInfo;
}

/**
 * True when the suite runs against a remote environment mailbox (MAILBOX_URL),
 * whose lifecycle specs cannot control. Specs using the helpers below should
 * skip themselves when this returns true.
 */
export function isRemoteMailbox(): boolean {
	return remoteMailboxUrl() !== null;
}

function localInfo(): MailboxInfo {
	if (isRemoteMailbox()) {
		throw new Error(
			'mailbox lifecycle control is unavailable against a remote environment mailbox (MAILBOX_URL)',
		);
	}
	return readInfo();
}

/** The link every agent reaches the mailbox through, to degrade and heal. */
export function mailboxLink(): Link {
	localInfo();
	return new Link(MAILBOX_LINK);
}

/**
 * Cap the bytes per second the mailbox serves blobs at, or lift the cap with
 * `null`. Blobs travel over iroh's QUIC link rather than the mailbox's HTTP
 * port, so the toxiproxy link cannot slow them; the mailbox throttles its own
 * blob provider instead (its `/testing/blob-throttle` endpoint, enabled for
 * the e2e mailbox by `MAILBOX_TESTING_ENDPOINTS`).
 */
export async function setMailboxBlobThrottle(
	bytesPerSec: number | null,
): Promise<void> {
	const { url } = localInfo();
	const res = await fetch(`${url}/testing/blob-throttle`, {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ bytes_per_sec: bytesPerSec }),
	});
	if (!res.ok) {
		throw new Error(`mailbox blob throttle request failed: ${res.status}`);
	}
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

/** Kill the mailbox server outright so connections to it are refused, and
 *  confirm it is gone: a spec that goes on believing it killed the server
 *  measures a cloud-connected app instead of an offline one. */
export async function killMailbox(): Promise<void> {
	const { pid } = localInfo();
	try {
		signalGroup(pid, 'SIGKILL');
	} catch {
		/* already gone */
	}
	const deadline = Date.now() + 5_000;
	while (isAlive(pid)) {
		if (Date.now() > deadline) {
			throw new Error(`mailbox server pid ${pid} is still alive after SIGKILL`);
		}
		await new Promise(resolve => setTimeout(resolve, 100));
	}
}

function isAlive(pid: number): boolean {
	try {
		process.kill(pid, 0);
		return true;
	} catch {
		return false;
	}
}

/**
 * Respawn the mailbox server on the same port + db, so a spec that killed it
 * leaves a live server behind for the rest of the run. Updates mailbox-info.json
 * with the new pid.
 */
export async function restartMailbox(): Promise<void> {
	const info = localInfo();
	const server = spawnMailboxServer(
		info.bindPort,
		info.dbPath,
		info.pushNotificationsUrl,
	);
	server.unref();
	writeFileSync(
		MAILBOX_INFO_PATH,
		JSON.stringify({ ...info, pid: server.pid }),
	);
	await waitForMailboxReady(info.url);
}
