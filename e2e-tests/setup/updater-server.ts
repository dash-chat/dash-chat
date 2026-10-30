/**
 * A minimal HTTP server serving a tauri-plugin-updater manifest, so a spec can
 * point an agent's real update check at it (`E2E_UPDATER_URL`) and drive the
 * banner through the plugin's own newer-version detection. Check-only: the
 * advertised artifact url and signature are placeholders that are never
 * downloaded.
 */
import { readFileSync } from 'node:fs';
import { createServer } from 'node:http';
import type { AddressInfo } from 'node:net';

export interface UpdaterServer {
	/** The endpoint to hand the app as `E2E_UPDATER_URL`. */
	url: string;
	/** Change the version later requests advertise. */
	setVersion(version: string): void;
	close(): Promise<void>;
}

const MANIFEST_PATH = '/latest.json';

/** Serve a manifest advertising `version` on an ephemeral localhost port. */
export async function startUpdaterServer(
	version: string,
): Promise<UpdaterServer> {
	let advertised = version;
	const server = createServer((req, res) => {
		if (req.url !== MANIFEST_PATH) {
			res.statusCode = 404;
			res.end();
			return;
		}
		res.setHeader('content-type', 'application/json');
		res.end(
			JSON.stringify({
				version: advertised,
				pub_date: new Date().toISOString(),
				url: 'http://127.0.0.1:1/never-downloaded',
				signature: 'e2e-placeholder',
			}),
		);
	});
	await new Promise<void>(resolve => server.listen(0, '127.0.0.1', resolve));
	const { port } = server.address() as AddressInfo;
	return {
		url: `http://127.0.0.1:${port}${MANIFEST_PATH}`,
		setVersion(version: string) {
			advertised = version;
		},
		close: () =>
			new Promise<void>((resolve, reject) => {
				// close() alone waits for the app's keep-alive socket to die.
				server.closeAllConnections();
				server.close(err => (err === undefined ? resolve() : reject(err)));
			}),
	};
}

/** The release manifest endpoint baked into production builds. */
export function productionUpdaterEndpoint(): string {
	const conf = JSON.parse(
		readFileSync(
			new URL('../../src-tauri/tauri.conf.json', import.meta.url),
			'utf8',
		),
	) as { plugins: { updater: { endpoints: string[] } } };
	return conf.plugins.updater.endpoints[0];
}

/** Whether the production update endpoint answers from this host, so the
 *  spec against it can skip on offline runs. A blackholing network must
 *  read as a skip, not a mocha hook timeout, hence the own timeout. */
export async function productionEndpointReachable(): Promise<boolean> {
	try {
		const response = await fetch(productionUpdaterEndpoint(), {
			signal: AbortSignal.timeout(5_000),
		});
		// An unread body keeps the socket pooled past the probe.
		await response.arrayBuffer();
		return response.ok;
	} catch {
		return false;
	}
}
