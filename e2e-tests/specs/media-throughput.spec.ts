/**
 * Media fetch throughput: how long a receiver waits for a batch of photos that
 * are already on the mailbox by the time it comes online.
 *
 * Skips itself unless E2E_STRESS=1. Run it with:
 *   PLATFORMS=android,desktop just test e2e media-throughput
 */
import { existsSync, statSync } from 'node:fs';

import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { SYNC_TIMEOUT } from '../helpers/timeouts';
import { isRemoteMailbox, mailboxBlobPath } from '../setup/mailbox-control';
import { type Agent, setupAgents } from '../setup/setup-agents';

/** One photo per message: a gallery hides everything past its fifth cell behind
 * a "+N" overlay (`display: none`), and a hidden image never loads. */
const PHOTO_COUNT = 20;
const PHOTO_WIDTH = 320;
const PHOTO_HEIGHT = 240;

/** What the spec asserts: receiver online → last photo rendered. */
const FETCH_BUDGET_MS = 3_000;

/** Per-wait ceiling, far above the budget so a slow run still reports a number
 * instead of dying on a timeout. */
const CEILING_MS = 120_000;

interface Batch {
	labels: string[];
	bytes: number;
	uploadMs: number;
}

describe('Media fetch throughput', function () {
	this.timeout(CEILING_MS * 3);

	let sender: Agent;
	let receiver: Agent;
	let senderName: string;
	let batch: Batch | undefined;
	let fetchMs: number | undefined;

	before(async function () {
		if (process.env.E2E_STRESS !== '1') this.skip();
		// mailboxBlobPath reads the local mailbox's store directly.
		if (isRemoteMailbox()) this.skip();
		const [agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		// Prefer a phone as the receiver: fetching is the leg under test.
		[sender, receiver] =
			agent1.platform !== 'desktop' && agent2.platform === 'desktop'
				? [agent2, agent1]
				: [agent1, agent2];
		senderName = 'Sender';
		await sender.createProfilePage.createProfile(senderName, 'Throughput');
		await receiver.createProfilePage.createProfile('Receiver', 'Throughput');
		await exchangeContacts(sender, receiver);
		// The composer only mounts once the chat leaves its pending state, which
		// needs the peer's profile — so the receiver has to still be up for it.
		await sender.directChatPage.composer.messageInput.waitForExist({
			timeout: SYNC_TIMEOUT,
		});
	});

	after(() => {
		if (batch === undefined) return;
		const kb = Math.round(batch.bytes / 1024);
		console.log(
			`\n${batch.labels.length} photos, ${kb}K total` +
				`\n  upload  ${String(batch.uploadMs).padStart(6)}ms` +
				`\n  fetch   ${String(fetchMs ?? 0).padStart(6)}ms  (budget ${FETCH_BUDGET_MS}ms)\n`,
		);
	});

	it('uploads the batch to the mailbox', async () => {
		// Down for the whole send phase, so it cannot pull a blob straight off
		// the sender before the sender goes away.
		await receiver.stopApp();

		const composer = sender.directChatPage.composer;
		const labels = Array.from({ length: PHOTO_COUNT }, (_, i) => `batch-${i}`);

		const startedAt = Date.now();
		let bytes = 0;
		for (const label of labels) {
			const attachAt = Date.now();
			await composer.attachNoisePhoto(label, PHOTO_WIDTH, PHOTO_HEIGHT);
			const sendAt = Date.now();
			await composer.send();
			await sender.directChatPage.messages.waitForPhotoMessage(
				label,
				CEILING_MS,
			);
			console.log(
				`${label}: attach ${sendAt - attachAt}ms  send+render ${Date.now() - sendAt}ms`,
			);
			const hash = await sender.directChatPage.messages.photoHash(label);
			const blobPath = mailboxBlobPath(hash);
			await sender.waitUntil(async () => existsSync(blobPath), {
				timeout: CEILING_MS,
				timeoutMsg: `Mailbox never stored ${label} (${hash})`,
			});
			// The stored blob is what crossed the wire; the composer re-encodes
			// before sending, so the staged file is a good bit larger.
			bytes += statSync(blobPath).size;
		}

		batch = { labels, bytes, uploadMs: Date.now() - startedAt };
	});

	it('serves the batch to a receiver that comes online with the sender gone', async () => {
		if (batch === undefined) throw new Error('the batch was never uploaded');
		// Every byte the receiver renders from here came from the mailbox.
		await sender.stopApp();

		const startedAt = Date.now();
		await receiver.startApp();
		const restartedAt = Date.now();
		await receiver.homePage.ready();
		const readyAt = Date.now();
		await receiver.homePage.openChat(senderName);
		const chatOpenAt = Date.now();
		console.log(
			`restart ${restartedAt - startedAt}ms  ready ${readyAt - restartedAt}ms  ` +
				`openChat ${chatOpenAt - readyAt}ms`,
		);
		for (const label of batch.labels) {
			const at = Date.now();
			await receiver.directChatPage.messages.waitForPhotoMessage(
				label,
				CEILING_MS,
			);
			console.log(`${label}: render ${Date.now() - at}ms`);
		}
		fetchMs = Date.now() - startedAt;

		if (fetchMs > FETCH_BUDGET_MS) {
			throw new Error(
				`Receiver took ${fetchMs}ms to render ${batch.labels.length} photos ` +
					`(${Math.round(batch.bytes / 1024)}K), budget ${FETCH_BUDGET_MS}ms`,
			);
		}
	});
});
