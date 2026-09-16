import { createConnection } from 'node:net';

/** Whether something on localhost accepts TCP connections on `port`. */
export function isPortListening(port: number): Promise<boolean> {
	return new Promise(resolve => {
		const sock = createConnection({ port, host: '127.0.0.1' });
		sock.on('connect', () => {
			sock.destroy();
			resolve(true);
		});
		sock.on('error', () => resolve(false));
	});
}

/** Poll until a TCP port is free (connection refused). */
export async function waitForPortFree(
	port: number,
	timeout = 10_000,
): Promise<void> {
	const deadline = Date.now() + timeout;
	while (Date.now() < deadline) {
		if (!(await isPortListening(port))) return;
		await new Promise(r => setTimeout(r, 200));
	}
	throw new Error(`Port ${port} still in use after ${timeout}ms`);
}

/** Poll until a TCP port accepts connections. */
export async function waitForPortListening(
	port: number,
	timeout = 10_000,
): Promise<void> {
	const deadline = Date.now() + timeout;
	while (Date.now() < deadline) {
		if (await isPortListening(port)) return;
		await new Promise(r => setTimeout(r, 200));
	}
	throw new Error(`Port ${port} not listening after ${timeout}ms`);
}
