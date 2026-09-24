/**
 * The TCP proxy the cloud mailbox's port goes through, so a spec can degrade
 * the link between an agent and the mailbox while the server itself keeps
 * running — a mailbox that is up but slow, hanging or refusing, which no
 * signal to its process can produce. wdio.conf.ts spawns one
 * `toxiproxy-server` per run in its own process group, like the mailbox, and
 * records its API port so spec workers can reach it through the
 * `toxiproxy-node-client` library.
 */
import { type ChildProcess, execSync, spawn } from 'node:child_process';
import {
	closeSync,
	mkdirSync,
	openSync,
	readFileSync,
	writeFileSync,
} from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
	type Bandwidth,
	type Latency,
	type Proxy,
	type Timeout,
	Toxiproxy,
} from 'toxiproxy-node-client';

import { startAgentLogger } from './agent-logger';
import { allocateFreePort } from './allocate-port';
import { waitForPortListening } from './wait-for-port';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..', '..');
const INFO_PATH = path.join(ROOT, '.dbs', 'e2e', 'toxiproxy-info.json');

/** Each way; a request answers about a second late. */
const SLOW_LATENCY_MS = 500;
const SLOW_JITTER_MS = 250;

/** Room for a message's few kilobytes, but a photo's megabyte takes about a
 *  minute: a phone on a weak uplink. */
const THROTTLED_UPLOAD_KB_PER_S = 20;

/** Every run's mailbox goes through the proxy, so a missing binary has to
 *  fail here, by name, rather than as a port that never listens. */
function assertToxiproxyAvailable(): void {
	try {
		execSync('command -v toxiproxy-server', { stdio: 'ignore' });
	} catch {
		throw new Error(
			'toxiproxy-server not found — every e2e run degrades the cloud mailbox ' +
				'through it. Run inside the nix dev shell, or install the ' +
				'toxiproxy-server binary from https://github.com/Shopify/toxiproxy/releases ' +
				'(2.12.0 is the version the shell pins) onto your PATH.',
		);
	}
}

export async function startToxiproxy(): Promise<{
	proc: ChildProcess;
	logger: ChildProcess;
	port: number;
}> {
	assertToxiproxyAvailable();
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
	// A spawn failure only reaches an 'error' listener; without this it
	// would surface as the port never listening.
	await new Promise<void>((resolve, reject) => {
		proc.once('spawn', resolve);
		proc.once('error', err =>
			reject(new Error(`could not start toxiproxy-server: ${err.message}`)),
		);
	});
	await waitForPortListening(port);
	writeFileSync(INFO_PATH, JSON.stringify({ pid: proc.pid, port }));
	console.log(`[toxiproxy] ready on port ${port} (pid=${proc.pid})`);
	return { proc, logger, port };
}

/** The run's server, from the port `startToxiproxy` recorded. */
function server(): Toxiproxy {
	const { port } = JSON.parse(readFileSync(INFO_PATH, 'utf-8')) as {
		port: number;
	};
	return new Toxiproxy(`http://127.0.0.1:${port}`);
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
		// IPv4, as the mailbox bound before the proxy fronted it: agents reach
		// it over IPv4, and a v6-only socket refuses them where bindv6only=1.
		await server().createProxy({
			name,
			listen: `0.0.0.0:${port}`,
			upstream: `127.0.0.1:${upstreamPort}`,
			enabled: true,
		});
		return new Link(name);
	}

	private proxy(): Promise<Proxy> {
		return server().get(this.name);
	}

	/** Every request still answers, about a second late. */
	async slow(): Promise<void> {
		const proxy = await this.healed();
		const attributes: Latency = {
			latency: SLOW_LATENCY_MS,
			jitter: SLOW_JITTER_MS,
		};
		for (const stream of ['upstream', 'downstream'] as const) {
			await proxy.addToxic<Latency>({
				name: `latency_${stream}`,
				type: 'latency',
				stream,
				toxicity: 1,
				attributes,
			});
		}
	}

	/** Requests reach the server at a trickle while answers come back at full
	 *  speed, so small requests land and large uploads crawl. */
	async throttleUploads(): Promise<void> {
		const proxy = await this.healed();
		await proxy.addToxic<Bandwidth>({
			name: 'bandwidth_upstream',
			type: 'bandwidth',
			stream: 'upstream',
			toxicity: 1,
			attributes: { rate: THROTTLED_UPLOAD_KB_PER_S },
		});
	}

	/** Connections open but nothing ever reaches the server, so every request
	 *  hangs until the client gives up. */
	async hang(): Promise<void> {
		const proxy = await this.healed();
		await proxy.addToxic<Timeout>({
			name: 'timeout_upstream',
			type: 'timeout',
			stream: 'upstream',
			toxicity: 1,
			attributes: { timeout: 0 },
		});
	}

	/** Connections are refused. */
	async cut(): Promise<void> {
		const proxy = await this.healed();
		await this.enable(proxy, false);
	}

	/** Back to a healthy link. */
	async heal(): Promise<void> {
		await this.healed();
	}

	/** The proxy with every toxic removed and its port listening. */
	private async healed(): Promise<Proxy> {
		const proxy = await this.proxy();
		// The client's Proxy carries no toxics, whatever its typings say.
		const { data: toxics } = await proxy.api.get<{ name: string }[]>(
			`${proxy.getPath()}/toxics`,
		);
		for (const { name } of toxics) {
			await (await proxy.getToxic(name)).remove();
		}
		return await this.enable(proxy, true);
	}

	private enable(proxy: Proxy, enabled: boolean): Promise<Proxy> {
		return proxy.update({
			enabled,
			listen: proxy.listen,
			upstream: proxy.upstream,
		});
	}
}
