/**
 * A minimal HTTP server serving a tauri-plugin-updater manifest, so a spec can
 * point an agent's real update check at it (`E2E_UPDATER_URL`) and drive the
 * banner through the plugin's own newer-version detection. Check-only: the
 * advertised artifact url and signature are placeholders that are never
 * downloaded.
 */
import { createServer } from 'node:http';
import type { AddressInfo } from 'node:net';

export interface UpdaterServer {
	/** The endpoint to hand the app as `E2E_UPDATER_URL`. */
	url: string;
	close(): Promise<void>;
}

/** Serve a manifest advertising `version` on an ephemeral localhost port. */
export async function startUpdaterServer(
	version: string,
): Promise<UpdaterServer> {
	const manifest = JSON.stringify({
		version,
		pub_date: new Date().toISOString(),
		url: 'http://127.0.0.1:1/never-downloaded',
		signature: 'e2e-placeholder',
	});
	const server = createServer((_req, res) => {
		res.setHeader('content-type', 'application/json');
		res.end(manifest);
	});
	await new Promise<void>(resolve => server.listen(0, '127.0.0.1', resolve));
	const { port } = server.address() as AddressInfo;
	return {
		url: `http://127.0.0.1:${port}/latest.json`,
		close: () =>
			new Promise<void>((resolve, reject) => {
				server.close(err => (err === undefined ? resolve() : reject(err)));
			}),
	};
}
