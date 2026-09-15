import { execSync } from 'node:child_process';
import { type AddressInfo, createServer } from 'node:net';

/** Allocate a free TCP port by briefly binding to port 0. Sync (a subprocess
 *  does the binding) because config-load-time callers can't await. */
function allocatePort(): number {
	return Number(
		execSync(
			'node -e "const s=require(\'net\').createServer();s.listen(0,()=>{process.stdout.write(String(s.address().port));s.close()})"',
		)
			.toString()
			.trim(),
	);
}

/** The port a briefly-bound server on `port` actually got (0 = any free
 *  port), or null when it can't be bound. */
function tryBind(port: number): Promise<number | null> {
	return new Promise(resolve => {
		const server = createServer();
		server.once('error', () => resolve(null));
		server.listen(port, () => {
			const bound = (server.address() as AddressInfo).port;
			server.close(() => resolve(bound));
		});
	});
}

/**
 * The preferred port when it is free, a random free one otherwise. Servers
 * whose URL gets baked into an app build (the iOS mailbox/push URLs) use this
 * so an unchanged build keeps pointing at a live server across runs — a
 * different port every run would force a rebuild every run.
 */
export async function allocatePreferredPort(
	preferred: number,
): Promise<number> {
	return (await tryBind(preferred)) ?? (await tryBind(0))!;
}

/**
 * Allocate a port once and pin it via an env var.
 *
 * WDIO's main process and worker process both load the config file
 * independently. The first load (main process) allocates the port and stores
 * it in the env var. The worker inherits the env var and reads the same
 * port, keeping capabilities and beforeSession in sync.
 */
export function allocatePinnedPort(envName: string): number {
	if (process.env[envName] === undefined) {
		process.env[envName] = String(allocatePort());
	}
	return Number(process.env[envName]);
}

/** Whether `port` can be bound right now. Sync, like [`allocatePort`]. */
function isFree(port: number): boolean {
	try {
		execSync(
			`node -e "const s=require('net').createServer();s.once('error',()=>process.exit(1));s.listen(${port},()=>{s.close()})"`,
			{ stdio: 'ignore' },
		);
		return true;
	} catch {
		return false;
	}
}

/**
 * Like [`allocatePinnedPort`], but the port is the first free one from
 * `base` up rather than any free one. For a port a phone binds as well as
 * the host: any-free lands in the range iOS also hands its own outgoing
 * connections (49152 and up), where the phone's bind then fails with
 * "Address already in use" as soon as the app has a few sockets open.
 */
export function allocatePinnedPortFrom(envName: string, base: number): number {
	if (process.env[envName] === undefined) {
		let port = base;
		while (!isFree(port)) port += 1;
		process.env[envName] = String(port);
	}
	return Number(process.env[envName]);
}

/** Any free TCP port, for servers whose port is not baked into a build. */
export async function allocateFreePort(): Promise<number> {
	return (await tryBind(0))!;
}
