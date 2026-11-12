/**
 * Iroh blob push e2e.
 *
 * The mailbox runs with both other blob-delivery paths off — HTTP upload
 * rejected with 410 Gone, its iroh fetch backstop never running — so a photo
 * Alice sends while Bob is offline can only reach the mailbox one way: her
 * client pushing it over iroh. Bob then comes back to find Alice gone, so
 * every byte he renders came from the mailbox.
 */
import http from 'node:http';

import { createProfiles } from '../helpers/flows/create-profiles';
import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { SYNC_TIMEOUT } from '../helpers/timeouts';
import {
	isRemoteMailbox,
	mailboxBindPort,
	mailboxBlobs,
	restartMailboxWithFlags,
} from '../setup/mailbox-control';
import { type Agent, setupAgents } from '../setup/setup-agents';

const PHOTO_WIDTH = 320;
const PHOTO_HEIGHT = 240;

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
	let alice: Agent;
	let bob: Agent;

	before(async function () {
		// The flags are knobs on the mailbox the run spawns; a remote
		// environment's mailbox is not ours to restart, nor its blob store
		// ours to read.
		if (isRemoteMailbox()) this.skip();
		await restartMailboxWithFlags({
			disableBlobHttpUpload: true,
			disableBlobFetch: true,
		});
		[alice, bob] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await createProfiles({ Alice: alice, Bob: bob });
		await exchangeContacts([alice, bob]);
		// The composer only mounts once the chat leaves its pending state,
		// which needs the peer's profile — so Bob has to still be up for it.
		await alice.directChatPage.composer.messageInput.waitForExist({
			timeout: SYNC_TIMEOUT,
		});
	});

	it('runs the mailbox with HTTP blob upload disabled', async () => {
		const status = await postBlobUpload(mailboxBindPort());
		expect(status).toBe(410);
	});

	it('delivers a photo from Alice to Bob through the mailbox over iroh push', async () => {
		// Offline for the whole send phase, so he cannot pull the blob straight
		// off Alice before she goes away.
		await bob.stopApp();

		const before = new Set(mailboxBlobs());
		await alice.directChatPage.composer.attachNoisePhoto(
			'pushed',
			PHOTO_WIDTH,
			PHOTO_HEIGHT,
		);
		await alice.directChatPage.composer.send();

		// HTTP upload is refused and the mailbox never fetches, so the blob
		// landing in its store can only be Alice's iroh push.
		await alice.waitUntil(
			async () => mailboxBlobs().some(p => !before.has(p)),
			{
				timeout: SYNC_TIMEOUT,
				timeoutMsg: 'mailbox never received the pushed blob',
			},
		);

		// Every byte Bob renders from here came from the mailbox.
		await alice.stopApp();
		await bob.startApp();
		await bob.homePage.ready();
		await bob.homePage.openChat('Alice');
		await bob.directChatPage.messages.waitForPhotoMessage('pushed');
	});
});
