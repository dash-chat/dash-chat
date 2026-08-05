/**
 * Media stress: how long a receiver's photos take to download when they are
 * already on the mailbox by the time it comes online, over repeated rounds so
 * the cost of a chat that keeps growing shows up.
 *
 * Skips itself unless E2E_STRESS=1. Run it with:
 *   PLATFORMS=android,desktop just test e2e media-stress
 */
import { statSync } from 'node:fs';

import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { SYNC_TIMEOUT } from '../helpers/timeouts';
import { isRemoteMailbox, mailboxBlobs } from '../setup/mailbox-control';
import { type Agent, setupAgents } from '../setup/setup-agents';

const ROUNDS = 6;
const PHOTO_COUNT = 20;
const PHOTO_WIDTH = 320;
const PHOTO_HEIGHT = 240;

/** What the spec asserts, per photo: the webview issuing the blob request to
 * its last byte. Relaunching the app and opening the chat is not part of it. */
const DOWNLOAD_BUDGET_MS = 3_000;

/** Per-wait ceiling, far above the budget so a slow run still reports a number
 * instead of dying on a timeout. */
const CEILING_MS = 120_000;

interface Round {
	labels: string[];
	bytes: number;
	uploadMs: number;
	/** The slowest single photo download in the round. */
	downloadMs?: number;
}

/** Photos are matched by `alt.includes(label)`, so labels are zero-padded to a
 * fixed width — otherwise `r1-p1` also matches `r1-p10`. */
function roundLabels(round: number): string[] {
	return Array.from(
		{ length: PHOTO_COUNT },
		(_, i) => `r${round}-p${String(i).padStart(2, '0')}`,
	);
}

/** Relaunch a stopped agent and walk it back into its chat with `peer`, logging
 * where the time went. Download recording starts before the chat opens, so no
 * attachment's fetch is missed. */
async function reopenChat(
	agent: Agent,
	role: string,
	peerName: string,
): Promise<void> {
	const startedAt = Date.now();
	await agent.startApp();
	const restartedAt = Date.now();
	await agent.execute(() => window.__test.recordMediaDownloads());
	await agent.homePage.ready();
	const readyAt = Date.now();
	await agent.homePage.openChat(peerName);
	console.log(
		`${role}: restart ${restartedAt - startedAt}ms  ` +
			`ready ${readyAt - restartedAt}ms  openChat ${Date.now() - readyAt}ms`,
	);
}

describe('Media stress', function () {
	this.timeout(CEILING_MS * 3);

	let sender: Agent;
	let receiver: Agent;
	const senderName = 'Sender';
	const receiverName = 'Receiver';
	const rounds = new Map<number, Round>();

	before(async function () {
		if (process.env.E2E_STRESS !== '1') this.skip();
		// The blob count is read straight off the local mailbox's store.
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
		await sender.createProfilePage.createProfile(senderName, 'Stress');
		await receiver.createProfilePage.createProfile(receiverName, 'Stress');
		await exchangeContacts(sender, receiver);
		// The composer only mounts once the chat leaves its pending state, which
		// needs the peer's profile — so the receiver has to still be up for it.
		await sender.directChatPage.composer.messageInput.waitForExist({
			timeout: SYNC_TIMEOUT,
		});
	});

	after(() => {
		if (rounds.size === 0) return;
		const lines = [...rounds].map(([round, r]) => {
			const kb = String(Math.round(r.bytes / 1024)).padStart(5);
			const upload = String(r.uploadMs).padStart(6);
			const download = String(r.downloadMs ?? 0).padStart(6);
			return (
				`  round ${round}  ${kb}K  upload ${upload}ms  ` +
				`slowest download ${download}ms`
			);
		});
		console.log(
			`\n${PHOTO_COUNT} photos per message, ${rounds.size} round(s), ` +
				`download budget ${DOWNLOAD_BUDGET_MS}ms\n${lines.join('\n')}\n`,
		);
	});

	for (let round = 1; round <= ROUNDS; round++) {
		it(`round ${round}: uploads the batch to the mailbox`, async () => {
			// Down for the whole send phase, so it cannot pull a blob straight off
			// the sender before the sender goes away.
			await receiver.stopApp();
			// Every round but the first inherits a sender stopped by the previous
			// round's fetch phase.
			if (round > 1) await reopenChat(sender, 'sender', receiverName);

			const composer = sender.directChatPage.composer;
			const labels = roundLabels(round);
			const before = new Set(mailboxBlobs());

			const startedAt = Date.now();
			for (const label of labels) {
				await composer.attachNoisePhoto(label, PHOTO_WIDTH, PHOTO_HEIGHT);
			}
			await composer.send();
			await sender.directChatPage.messages.waitForPhotoMessage(
				labels[0],
				CEILING_MS,
			);
			const stored = () => mailboxBlobs().filter(p => !before.has(p));
			await sender.waitUntil(async () => stored().length >= PHOTO_COUNT, {
				timeout: CEILING_MS,
				timeoutMsg: `Mailbox stored ${stored().length}/${PHOTO_COUNT} blobs`,
			});

			// The stored blobs are what crossed the wire; the composer re-encodes
			// before sending, so the staged files are a good bit larger.
			const bytes = stored().reduce((sum, p) => sum + statSync(p).size, 0);
			rounds.set(round, { labels, bytes, uploadMs: Date.now() - startedAt });
		});

		it(`round ${round}: serves the batch to a receiver that comes online with the sender gone`, async () => {
			const current = rounds.get(round);
			if (current === undefined)
				throw new Error(`round ${round}'s batch was never uploaded`);
			// Every byte the receiver renders from here came from the mailbox.
			await sender.stopApp();

			await reopenChat(receiver, 'receiver', senderName);
			const messages = receiver.directChatPage.messages;
			// The gallery lays out five cells and hides the rest behind a "+N"
			// overlay, and a hidden image never loads. The lightbox filmstrip is
			// where every photo in the message actually gets requested.
			await messages.waitForPhotoMessage(current.labels[0], CEILING_MS);
			await messages.openPhoto(current.labels[0]);
			await messages.lightbox.waitForStripLoaded();

			const downloads: number[] = [];
			for (const label of current.labels) {
				downloads.push(await messages.photoDownloadMs(label));
			}
			current.downloadMs = Math.max(...downloads);
			console.log(`round ${round} downloads (ms): ${downloads.join(', ')}`);

			if (current.downloadMs > DOWNLOAD_BUDGET_MS) {
				throw new Error(
					`Round ${round}'s slowest photo took ${current.downloadMs}ms to ` +
						`download (${PHOTO_COUNT} photos, ` +
						`${Math.round(current.bytes / 1024)}K), budget ${DOWNLOAD_BUDGET_MS}ms`,
				);
			}
		});
	}
});
