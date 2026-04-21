import { createConnection } from 'node:net';

/** Poll until a TCP port is free (connection refused). */
export async function waitForPortFree(port: number, timeout = 10_000): Promise<void> {
	const deadline = Date.now() + timeout;
	while (Date.now() < deadline) {
		const inUse = await new Promise<boolean>((resolve) => {
			const sock = createConnection({ port, host: '127.0.0.1' });
			sock.on('connect', () => { sock.destroy(); resolve(true); });
			sock.on('error', () => resolve(false));
		});
		if (!inUse) return;
		await new Promise(r => setTimeout(r, 200));
	}
	throw new Error(`Port ${port} still in use after ${timeout}ms`);
}

/** Poll until a TCP port accepts connections. */
export async function waitForPortListening(port: number, timeout = 10_000): Promise<void> {
	const deadline = Date.now() + timeout;
	while (Date.now() < deadline) {
		const listening = await new Promise<boolean>((resolve) => {
			const sock = createConnection({ port, host: '127.0.0.1' });
			sock.on('connect', () => { sock.destroy(); resolve(true); });
			sock.on('error', () => resolve(false));
		});
		if (listening) return;
		await new Promise(r => setTimeout(r, 200));
	}
	throw new Error(`Port ${port} not listening after ${timeout}ms`);
}
