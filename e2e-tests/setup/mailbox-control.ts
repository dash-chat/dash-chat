/**
 * Drive the global mailbox server's lifecycle from inside a spec so tests can
 * exercise the offline-UX state transitions.
 *
 * wdio.conf.ts spawns the mailbox server in its own process group (`detached:
 * true`) during `onPrepare` and writes its pid + port to a JSON file. These
 * helpers use that pid to signal the whole group.
 */
import { existsSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { signalGroup } from './host-process';
import {
	MAILBOX_LINK,
	type MailboxFlags,
	spawnMailboxServer,
	waitForMailboxReady,
} from './mailbox-server';
import { mailboxAnswers } from './mailbox-tls';
import { remoteMailboxUrl } from './test-env';
import { Link, type LinkConditions } from './toxiproxy';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const MAILBOX_INFO_PATH = path.join(
	__dirname,
	'..',
	'..',
	'.dbs',
	'e2e',
	'mailbox-info.json',
);

/** What a suspended mailbox gets to answer before it counts as not serving:
 * a SIGSTOPped process never answers, and a healthy local one answers at
 * once. */
const HEALTH_TIMEOUT_MS = 2_000;

interface MailboxInfo {
	url: string;
	pid: number;
	port: number;
	bindPort: number;
	dbPath: string;
	pushNotificationsUrl?: string;
	disableBlobHttpUpload?: boolean;
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

/**
 * Whether an operation reaching the mailbox wakes a phone: it forwards
 * arrivals to a push-notifications server, and is answering. A spec that
 * suspends or kills the mailbox leaves nothing to forward them, so nothing
 * reaches a device whose app is away.
 */
export async function mailboxWakesPhones(): Promise<boolean> {
	if (isRemoteMailbox()) return false;
	if (readInfo().pushNotificationsUrl === undefined) return false;
	return await mailboxServing();
}

/** Whether the mailbox answers right now. A suspended or killed one never
 *  does, which is how a spec that took it down is known without being asked. */
export async function mailboxServing(): Promise<boolean> {
	if (isRemoteMailbox()) return false;
	return await mailboxAnswers(readInfo().url, HEALTH_TIMEOUT_MS);
}

/** Whether the link agents reach the mailbox through can be degraded: the
 *  proxy fronts the mailbox a run spawns, never a remote environment's. */
export function mailboxDegradable(): boolean {
	return !isRemoteMailbox();
}

function link(): Link {
	localInfo();
	return new Link(MAILBOX_LINK);
}

/** Every request still answers, about a second late. */
export function slowMailboxLink(): Promise<void> {
	return link().slow();
}

/** Every connection from now on crosses a network with these conditions. */
export function shapeMailboxLink(conditions: LinkConditions): Promise<void> {
	return link().shape(conditions);
}

/** Requests are accepted and then never answered. */
export function hangMailboxLink(): Promise<void> {
	return link().hang();
}

/** Connections are refused outright. */
export function cutMailboxLink(): Promise<void> {
	return link().cut();
}

/** Undo whatever degradation is in force. */
export function healMailboxLink(): Promise<void> {
	return link().heal();
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

/** The local mailbox's plaintext bind port (bypassing the TLS proxy and the
 *  degradable link), for specs that poke its HTTP API directly. */
export function mailboxBindPort(): number {
	return localInfo().bindPort;
}

/**
 * Respawn the mailbox server on the same port + db, so a spec that killed it
 * leaves a live server behind for the rest of the run. `flags` override the
 * persisted dev flags; without them the current ones carry over. Updates
 * mailbox-info.json with the new pid.
 */
export async function restartMailbox(flags?: MailboxFlags): Promise<void> {
	const info = localInfo();
	const disableBlobHttpUpload =
		flags?.disableBlobHttpUpload ?? info.disableBlobHttpUpload;
	const server = spawnMailboxServer(
		info.bindPort,
		info.dbPath,
		info.pushNotificationsUrl,
		{ disableBlobHttpUpload },
	);
	server.unref();
	writeFileSync(
		MAILBOX_INFO_PATH,
		JSON.stringify({ ...info, disableBlobHttpUpload, pid: server.pid }),
	);
	await waitForMailboxReady(info.url);
}

/**
 * Respawn the mailbox with dev flags in force (e.g. `disableBlobHttpUpload`
 * to confine blob delivery to iroh push), killing the running one first. The
 * flags persist into mailbox-info.json, so a mid-spec kill + restartMailbox
 * keeps them; ensureHealthyMailbox between spec files restores the default
 * server.
 */
export async function restartMailboxWithFlags(
	flags: MailboxFlags,
): Promise<void> {
	if (isAlive(localInfo().pid)) await killMailbox();
	await restartMailbox(flags);
}

/** Back to a healthy mailbox, whatever a spec left it in: running, not
 *  suspended, behind a link with nothing degrading it. Every spec file
 *  starts and ends here (wdio.conf.ts), so none has to put back what it
 *  broke, and a run that died mid-spec cannot hand the next one a dead
 *  mailbox. A no-op against a remote mailbox, which is never broken. */
export async function ensureHealthyMailbox(): Promise<void> {
	if (isRemoteMailbox()) return;
	const { pid, url, disableBlobHttpUpload } = readInfo();
	// Dev flags a spec put in force end with its file: the next one gets the
	// default server back.
	const flagged = disableBlobHttpUpload === true;
	if (isAlive(pid) && !flagged) {
		resumeMailbox();
	} else {
		if (isAlive(pid)) await killMailbox();
		await restartMailbox(
			flagged ? { disableBlobHttpUpload: false } : undefined,
		);
	}
	await healMailboxLink();
	await waitForMailboxReady(url);
}
