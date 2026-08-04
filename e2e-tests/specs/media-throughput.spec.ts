/**
 * Media transfer throughput over the mailbox.
 *
 * The two agents are never online together while the media exists, so the
 * mailbox is provably the only way the bytes can reach the receiver: the
 * receiver's app is closed for the whole send phase, and the sender's app is
 * closed before the receiver comes back. What is measured is the wait a user
 * actually sees — open the app after being offline, how long until the photos
 * render.
 *
 * Too slow for the default suite, so it skips itself unless E2E_STRESS=1.
 * Run it with:
 *   PLATFORMS=android,desktop just test e2e media-throughput
 *   E2E_STRESS=1 just test e2e
 */
import { existsSync, statSync } from 'node:fs';

import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { SYNC_TIMEOUT } from '../helpers/timeouts';
import { isRemoteMailbox, mailboxBlobPath } from '../setup/mailbox-control';
import { type Agent, setupAgents } from '../setup/setup-agents';

interface PhotoCase {
	label: string;
	width: number;
	height: number;
}

const PHOTO_CASES: PhotoCase[] = [
	{ label: 'small', width: 640, height: 480 },
	{ label: 'medium', width: 1280, height: 960 },
	{ label: 'large', width: 1920, height: 1440 },
];

const RUNS = 3;

/** What the spec asserts: every upload and every download finishes inside this.
 * A starting point, not a measured target — the first real-device runs exist to
 * produce the distribution the real number should come from. */
const MEDIA_TIMEOUT = 30_000;

interface Sample {
	label: string;
	caption: string;
	hash: string;
	bytes: number;
	/** Send → the mailbox has the bytes on disk. */
	uploadMs: number;
	/** Receiver app start → its caption rendered. */
	opMs?: number;
	/** Receiver app start → its photo rendered. */
	mediaMs?: number;
}

function formatReport(samples: Sample[]): string {
	const ms = (v: number | undefined) =>
		v === undefined ? '     -' : String(v).padStart(6);
	return samples
		.map(s =>
			[
				s.label.padEnd(7),
				`${Math.round(s.bytes / 1024)}K`.padStart(7),
				`up ${ms(s.uploadMs)}ms`,
				`op ${ms(s.opMs)}ms`,
				`media ${ms(s.mediaMs)}ms`,
			].join('  '),
		)
		.join('\n');
}

describe('Media transfer throughput', function () {
	// Each test walks the whole sweep and every transfer in it is allowed
	// MEDIA_TIMEOUT, so mocha's own limit has to clear that worst case —
	// otherwise it fires first and the timeout under test never applies.
	this.timeout(PHOTO_CASES.length * RUNS * MEDIA_TIMEOUT);

	let sender: Agent;
	let receiver: Agent;
	let senderName: string;
	const samples: Sample[] = [];

	before(async function () {
		// Too slow for the default suite. `just test e2e NAME` sets E2E_STRESS
		// so naming this spec runs it.
		if (process.env.E2E_STRESS !== '1') this.skip();
		// The mailbox's blob store has to be readable from here to tell whether a
		// blob landed before the sender is killed.
		if (isRemoteMailbox()) this.skip();
		const [agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		// Prefer a phone as the sender: the upload leg is where platforms differ
		// most, and the receiver only ever talks to the mailbox.
		[sender, receiver] =
			agent1.platform === 'desktop' && agent2.platform !== 'desktop'
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

		// Closed for the whole send phase, so it cannot pull a single blob
		// directly off the sender before the sender goes away.
		await receiver.deleteSession();
	});

	after(() => {
		if (samples.length === 0) return;
		console.log(`\nMedia transfer samples:\n${formatReport(samples)}\n`);
	});

	/** Send one photo and wait until its bytes are in the mailbox's blob store —
	 * killing the sender before that would strand them, since the mailbox's
	 * fetch backstop cannot dial a dead process. */
	async function send(photoCase: PhotoCase, run: number): Promise<Sample> {
		const label = `${photoCase.label}-${run}`;
		const caption = `probe ${label}`;
		const composer = sender.directChatPage.composer;
		await composer.attachNoisePhoto(label, photoCase.width, photoCase.height);
		await composer.type(caption);

		const startedAt = Date.now();
		await composer.send();
		await sender.directChatPage.messages.waitForPhotoMessage(label);
		const hash = await sender.directChatPage.messages.photoHash(label);

		const blobPath = mailboxBlobPath(hash);
		await sender.waitUntil(async () => existsSync(blobPath), {
			timeout: MEDIA_TIMEOUT,
			timeoutMsg: `Mailbox never stored ${label} (${hash})`,
		});

		// The stored blob is what actually crosses the wire — the composer
		// re-encodes before sending, so the staged file is a good bit larger.
		const bytes = statSync(blobPath).size;
		return { label, caption, hash, bytes, uploadMs: Date.now() - startedAt };
	}

	it('uploads the sweep to the mailbox', async () => {
		for (const photoCase of PHOTO_CASES) {
			for (let run = 0; run < RUNS; run++) {
				samples.push(await send(photoCase, run));
			}
		}
	});

	it('serves the whole sweep to a receiver that starts with the sender gone', async () => {
		// From here the sender's process no longer exists, so every byte the
		// receiver renders came from the mailbox.
		await sender.deleteSession();

		const startedAt = Date.now();
		await receiver.restart();
		await receiver.homePage.ready();
		await receiver.homePage.openChat(senderName);

		for (const sample of samples) {
			await receiver.directChatPage.messages.waitForMessage(
				sample.caption,
				SYNC_TIMEOUT,
			);
			sample.opMs = Date.now() - startedAt;
			await receiver.directChatPage.messages.waitForPhotoMessage(
				sample.label,
				MEDIA_TIMEOUT,
			);
			sample.mediaMs = Date.now() - startedAt;
		}
	});

});
