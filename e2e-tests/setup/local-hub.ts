/**
 * Standalone local message hubs — `mailbox-local-server`, the binary the
 * docker image ships — spawned on the host's LAN for specs that exercise hub
 * discovery from a phone. Unlike the cloud mailbox, hubs are per-spec: one
 * running during any other spec would hand it a local mailbox it does not
 * expect, so a spec spawns its own in `before` and stops them in `after`.
 */
import { type ChildProcess, spawn } from 'node:child_process';
import { closeSync, existsSync, mkdirSync, openSync } from 'node:fs';
import { networkInterfaces } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { startAgentLogger } from './agent-logger';
import { allocateFreePort } from './allocate-port';
import { waitForMailboxReady } from './mailbox-server';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..', '..');

export const LOCAL_HUB_PACKAGE = 'mailbox-local-server';

export interface LocalHub {
	/** The `name` it was spawned under; prefixes its log lines. */
	name: string;
	/** Its MailboxId — also the mDNS instance name it announces under. */
	id: string;
	/** A URL the harness can reach it at, on the address it is bound to. */
	url: string;
	port: number;
	proc: ChildProcess;
	logger: ChildProcess;
}

/**
 * Spawn a hub, wait until it answers /health, and echo its log with a
 * `[hub-<name>]` prefix. Each hub gets its own db, hence its own key and
 * MailboxId. It announces the production service type, exactly as a deployed
 * hub does, on every LAN the host is on. Passing the `port` a previous hub
 * of the same `name` had brings that hub back as a restart: same key, same
 * MailboxId, same URL.
 */
export async function spawnLocalHub(
	name: string,
	port?: number,
): Promise<LocalHub> {
	const bin = path.join(ROOT, 'target', 'debug', LOCAL_HUB_PACKAGE);
	if (!existsSync(bin)) {
		throw new Error(
			`${bin} not found — the suite builds it against a local mailbox; ` +
				`otherwise run 'cargo build -p ${LOCAL_HUB_PACKAGE}'`,
		);
	}
	if (port === undefined) port = await allocateFreePort();
	const dir = path.join(ROOT, '.dbs', 'e2e', 'hubs', name);
	mkdirSync(dir, { recursive: true });
	const logFile = path.join(dir, 'hub.log');
	const logger = startAgentLogger(`hub-${name}`, logFile);
	// The log goes to a file rather than a pipe for the same reason the cloud
	// mailbox's does: nothing must block the hub's writes if the spec worker
	// that spawned it is gone.
	const logFd = openSync(logFile, 'a');
	// `detached: true` puts it in its own process group so stopLocalHub can
	// signal -pid without touching the test runner.
	const proc = spawn(
		bin,
		['--db-path', path.join(dir, 'mailbox.redb'), '--port', String(port)],
		{ cwd: ROOT, stdio: ['ignore', logFd, logFd], detached: true },
	);
	closeSync(logFd);
	const url = `http://127.0.0.1:${port}`;
	try {
		await waitForMailboxReady(url);
	} catch (err) {
		signalHub({ proc } as LocalHub, 'SIGKILL');
		logger.kill();
		throw err;
	}
	const health = (await (await fetch(`${url}/health`)).json()) as {
		endpoint_id: string;
	};
	console.log(
		`[hub-${name}] ready on port ${port} (pid=${proc.pid}, mailbox id ${health.endpoint_id})`,
	);
	return { name, id: health.endpoint_id, url, port, proc, logger };
}

/** Bring a stopped hub back as the same hub: same db, so same key, MailboxId
 *  and port. */
export function restartLocalHub(hub: LocalHub): Promise<LocalHub> {
	return spawnLocalHub(hub.name, hub.port);
}

function signalHub(hub: LocalHub, signal: NodeJS.Signals): void {
	try {
		process.kill(-hub.proc.pid!, signal);
	} catch {
		/* already gone */
	}
}

/**
 * Stop a hub and wait for it to exit. SIGINT by default: the hub only sends
 * its mDNS goodbye on a graceful shutdown, and without one every phone on the
 * LAN keeps its records cached until they age out. SIGKILL is for a spec that
 * wants exactly that: a hub that vanished without a word.
 */
export function stopLocalHub(
	hub: LocalHub,
	signal: 'SIGINT' | 'SIGKILL' = 'SIGINT',
	timeoutMs = 10_000,
): Promise<void> {
	hub.logger.kill();
	return new Promise(resolve => {
		if (hub.proc.exitCode !== null) {
			resolve();
			return;
		}
		const timer = setTimeout(() => {
			signalHub(hub, 'SIGKILL');
			resolve();
		}, timeoutMs);
		hub.proc.once('exit', () => {
			clearTimeout(timer);
			resolve();
		});
		signalHub(hub, signal);
	});
}

/** The host's IPv4 addresses on its up-and-running non-loopback interfaces:
 *  one per LAN the host is on, and so one per LAN a hub can be bound to. */
export function hostLanAddresses(): string[] {
	return Object.values(networkInterfaces())
		.flat()
		.filter(
			(info): info is NonNullable<typeof info> =>
				info !== undefined && info.family === 'IPv4' && !info.internal,
		)
		.map(info => info.address);
}
