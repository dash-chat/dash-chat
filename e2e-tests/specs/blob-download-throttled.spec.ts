/**
 * A large attachment arriving over a slow cloud link: the mailbox serves blob
 * bytes no faster than a budget, so the receiver's progress ring fills a bit
 * at every poll, and a mailbox that stops answering mid-download leaves the
 * progress where it was until it answers again, after which it climbs on.
 *
 * The run's toxiproxy link cannot stand in for the slow link: blobs travel
 * over iroh's QUIC (UDP) connection, not the mailbox's HTTP port. The mailbox
 * throttles its own blob provider instead, and suspending its process stands
 * in for the link going away. Both agents run without p2p, so the mailbox is
 * the only place the blob can come from.
 */
import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { MEDIA_SYNC_TIMEOUT, SYNC_TIMEOUT } from '../helpers/timeouts';
import {
	isRemoteMailbox,
	resumeMailbox,
	setMailboxBlobThrottle,
	suspendMailbox,
} from '../setup/mailbox-control';
import { type Agent, setupAgents } from '../setup/setup-agents';

/** iroh-blobs releases 16 KiB per throttle decision, so at this budget one
 * lands about every 256ms: at least one per poll. */
const THROTTLE_BYTES_PER_SEC = 64 * 1024;
const POLL_INTERVAL_MS = 500;
/** Polls to watch the progress climb over; well short of the whole transfer. */
const CLIMBING_POLLS = 6;
/** Long enough for the bytes already in flight to land before a reading. */
const IN_FLIGHT_SETTLE_MS = 1_000;
/** Long enough for chunks to have landed had the mailbox still been serving. */
const AWAY_MS = 2_000;

describe('Blob download over a slow cloud link', function () {
	this.timeout(300_000);

	let agent1: Agent;
	let agent2: Agent;

	const messages = () => agent2.directChatPage.messages;

	/** Bytes of `name` the receiver holds, read off its progress ring; 0 while
	 * the ring is still indeterminate. */
	async function receivedBytes(name: string): Promise<number> {
		const value = await messages()
			.fileProgressRing(name)
			.getAttribute('aria-valuenow');
		return value === null ? 0 : Number(value);
	}

	/** The sender streams the bytes to the mailbox after publishing the
	 * message, so the receiver's first attempt can find the mailbox without
	 * them and its next comes a fetch pass later. */
	async function waitForDownloadStarted(name: string): Promise<void> {
		await messages()
			.fileProgressRing(name)
			.waitForDisplayed({ timeout: SYNC_TIMEOUT });
		await agent2.waitUntil(async () => (await receivedBytes(name)) > 0, {
			timeout: MEDIA_SYNC_TIMEOUT,
			timeoutMsg: `no byte of ${name} arrived`,
		});
	}

	/** Every poll must read more bytes than the one before it. */
	async function expectClimbing(name: string, polls: number): Promise<void> {
		let last = await receivedBytes(name);
		for (let i = 0; i < polls; i++) {
			await agent2.pause(POLL_INTERVAL_MS);
			const now = await receivedBytes(name);
			if (now <= last) {
				throw new Error(
					`poll ${i + 1}: ${name} stayed at ${now} bytes (was ${last})`,
				);
			}
			last = now;
		}
	}

	async function expectComplete(name: string): Promise<void> {
		await messages()
			.fileProgressRing(name)
			.waitForDisplayed({ reverse: true, timeout: MEDIA_SYNC_TIMEOUT });
	}

	async function send(name: string, size: number): Promise<void> {
		await agent1.directChatPage.composer.attachFileOfSize(size, name);
		await agent1.directChatPage.composer.send();
		await agent1.directChatPage.messages.waitForFileMessage(name);
	}

	before(async function () {
		if (isRemoteMailbox()) this.skip();
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await Promise.all([agent1.disableP2p(), agent2.disableP2p()]);
		await agent1.createProfilePage.createProfile('Alice', 'Slow');
		await agent2.createProfilePage.createProfile('Bob', 'Slow');
		await exchangeContacts(agent1, agent2);
		await setMailboxBlobThrottle(THROTTLE_BYTES_PER_SEC);
	});

	after(async () => {
		await setMailboxBlobThrottle(null);
	});

	it('fills the progress ring at every poll while the mailbox serves slowly', async () => {
		// Attachments are zero-filled, so each case needs its own size to be
		// its own blob.
		await send('slow.bin', 640 * 1024);
		await waitForDownloadStarted('slow.bin');
		await expectClimbing('slow.bin', CLIMBING_POLLS);
		await expectComplete('slow.bin');
	});

	it('holds the progress while the mailbox is away and climbs again once it is back', async () => {
		await send('flaky.bin', 768 * 1024);
		await waitForDownloadStarted('flaky.bin');
		await expectClimbing('flaky.bin', 2);

		suspendMailbox();
		try {
			await agent2.pause(IN_FLIGHT_SETTLE_MS);
			const held = await receivedBytes('flaky.bin');
			await agent2.pause(AWAY_MS);
			expect(await receivedBytes('flaky.bin')).toBe(held);
		} finally {
			resumeMailbox();
		}

		await expectClimbing('flaky.bin', CLIMBING_POLLS);
		await expectComplete('flaky.bin');
	});
});
