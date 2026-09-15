/**
 * The TCP proxy the cloud mailbox's port goes through, so a spec can degrade
 * the link between an agent and the mailbox while the server itself keeps
 * running — a mailbox that is up but slow, hanging or refusing, which no
 * signal to its process can produce. wdio.conf.ts spawns one
 * `toxiproxy-server` per run in its own process group, like the mailbox, and
 * records its API port so spec workers can reach it; a link's toxics are
 * driven over the HTTP API.
 */
import { type ChildProcess, spawn } from 'node:child_process';
import {
	closeSync,
	mkdirSync,
	openSync,
	readFileSync,
	writeFileSync,
} from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { startAgentLogger } from './agent-logger';
import { allocateFreePort } from './allocate-port';
import { waitForPortListening } from './wait-for-port';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..', '..');
const INFO_PATH = path.join(ROOT, '.dbs', 'e2e', 'toxiproxy-info.json');

/** Each way; a request answers about a second late. */
const SLOW_LATENCY_MS = 500;
const SLOW_JITTER_MS = 250;

export async function startToxiproxy(): Promise<{
	proc: ChildProcess;
	logger: ChildProcess;
	port: number;
}> {
	const port = await allocateFreePort();
	const logFile = path.join(ROOT, '.dbs', 'e2e', 'toxiproxy', 'toxiproxy.log');
	mkdirSync(path.dirname(logFile), { recursive: true });
	const logger = startAgentLogger('toxiproxy', logFile);
	const logFd = openSync(logFile, 'a');
	const proc = spawn(
		'toxiproxy-server',
		['-host', '127.0.0.1', '-port', String(port)],
		{
			stdio: ['ignore', logFd, logFd],
			detached: true,
			// The checkout-scoped cleanup tells this run's server from another
			// checkout's by this path in its environment.
			env: { ...process.env, E2E_DBS: path.join(ROOT, '.dbs') + path.sep },
		},
	);
	closeSync(logFd);
	proc.on('error', err => {
		console.error(`[toxiproxy] ERROR ${err.message}`);
	});
	await waitForPortListening(port);
	writeFileSync(INFO_PATH, JSON.stringify({ pid: proc.pid, port }));
	console.log(`[toxiproxy] ready on port ${port} (pid=${proc.pid})`);
	return { proc, logger, port };
}

function apiUrl(): string {
	const { port } = JSON.parse(readFileSync(INFO_PATH, 'utf-8')) as {
		port: number;
	};
	return `http://127.0.0.1:${port}`;
}

async function api(
	method: 'POST' | 'PATCH' | 'DELETE',
	pathname: string,
	body?: object,
): Promise<void> {
	const res = await fetch(`${apiUrl()}${pathname}`, {
		method,
		headers: { 'content-type': 'application/json' },
		body: body === undefined ? undefined : JSON.stringify(body),
	});
	if (!res.ok) {
		throw new Error(
			`toxiproxy ${method} ${pathname} answered ${res.status}: ${await res.text()}`,
		);
	}
}

/** The link between a port agents connect to and the server bound behind
 *  it. Healthy until a spec degrades it; `heal` puts it back. */
export class Link {
	constructor(readonly name: string) {}

	/** Forward `port`, on every interface, to the server on `upstreamPort`. */
	static async open(
		name: string,
		port: number,
		upstreamPort: number,
	): Promise<Link> {
		await api('POST', '/proxies', {
			name,
			listen: `[::]:${port}`,
			upstream: `127.0.0.1:${upstreamPort}`,
			enabled: true,
		});
		return new Link(name);
	}

	private toxic(type: string, stream: string, attributes: object) {
		return api('POST', `/proxies/${this.name}/toxics`, {
			name: `${type}_${stream}`,
			type,
			stream,
			toxicity: 1,
			attributes,
		});
	}

	/** Every request still answers, about a second late. */
	async slow(): Promise<void> {
		await this.heal();
		const attributes = { latency: SLOW_LATENCY_MS, jitter: SLOW_JITTER_MS };
		await this.toxic('latency', 'upstream', attributes);
		await this.toxic('latency', 'downstream', attributes);
	}

	/** Connections open but nothing ever reaches the server, so every request
	 *  hangs until the client gives up. */
	async hang(): Promise<void> {
		await this.heal();
		await this.toxic('timeout', 'upstream', { timeout: 0 });
	}

	/** Connections are refused. */
	async cut(): Promise<void> {
		await this.heal();
		await api('PATCH', `/proxies/${this.name}`, { enabled: false });
	}

	/** Back to a healthy link. */
	async heal(): Promise<void> {
		const res = await fetch(`${apiUrl()}/proxies/${this.name}/toxics`);
		const toxics = (await res.json()) as { name: string }[];
		for (const { name } of toxics) {
			await api('DELETE', `/proxies/${this.name}/toxics/${name}`);
		}
		await api('PATCH', `/proxies/${this.name}`, { enabled: true });
	}
}
