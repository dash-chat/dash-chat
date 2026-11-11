/**
 * Iroh blob push e2e — wiring only, so far.
 *
 * Restarts the cloud mailbox with --disable-blob-http-upload and confirms the
 * knob is in force (POST /blobs/upload → 410 Gone). The real scenario — an
 * agent's media blob reaching the mailbox with every delivery path but iroh
 * push disabled — builds on this.
 */
import http from 'node:http';

import {
	isRemoteMailbox,
	mailboxBindPort,
	restartMailboxWithFlags,
} from '../setup/mailbox-control';

/** POST /blobs/upload straight at the mailbox's bind port (no TLS, no link):
 *  the assertion is about the server's route table, not the transport. */
function postBlobUpload(port: number): Promise<number> {
	return new Promise((resolve, reject) => {
		const request = http.request(
			{ host: '127.0.0.1', port, path: '/blobs/upload', method: 'POST' },
			response => {
				response.resume();
				resolve(response.statusCode ?? 0);
			},
		);
		request.on('error', reject);
		request.end();
	});
}

describe('Iroh blob push', () => {
	before(async function () {
		// The flag is a knob on the mailbox the run spawns; a remote
		// environment's mailbox is not ours to restart.
		if (isRemoteMailbox()) this.skip();
		await restartMailboxWithFlags({ disableBlobHttpUpload: true });
	});

	it('runs the mailbox with HTTP blob upload disabled', async () => {
		const status = await postBlobUpload(mailboxBindPort());
		expect(status).toBe(410);
	});
});
