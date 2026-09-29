/**
 * A large attachment arriving over a slow cloud link: the mailbox serves blob
 * bytes no faster than a budget, so the receiver's progress ring climbs poll
 * after poll, and a mailbox that stops answering mid-download leaves the
 * progress where it was until it answers again, after which it climbs on.
 *
 * The run's toxiproxy link cannot stand in for the slow link: blobs travel
 * over iroh's QUIC (UDP) connection, not the mailbox's HTTP port. The mailbox
 * throttles its own blob provider instead, and suspending its process stands
 * in for the link going away. Both agents run without p2p, so the mailbox is
 * the only place the blob can come from.
 */
import { createProfiles } from '../helpers/flows/create-profiles';
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
 * lands about every 256ms: a few per reading. */
const THROTTLE_BYTES_PER_SEC = 64 * 1024;
const READING_INTERVAL_MS = 500;
/** Readings to watch the progress climb over; well short of the transfer. */
const CLIMBING_READINGS = 6;
/** Long enough for the bytes already in flight to land before a reading. */
const IN_FLIGHT_SETTLE_MS = 1_000;
/** Long enough for chunks to have landed had the mailbox still been serving. */
const AWAY_MS = 2_000;

describe('Blob download over a slow cloud link', function () {
	this.timeout(300_000);

	let agent1: Agent;
	let agent2: Agent;
	let throttled = false;

	const messages = () => agent2.directChatPage.messages;

	/** What the receiver's progress ring reports for `name`: the bytes it
	 * holds, or no figure at all — the ring drops its value while it is still
	 * indeterminate, and again once it goes stalled, where it is a retry
	 * affordance rather than a progressbar. */
	async function readProgress(
		name: string,
	): Promise<{ bytes: number | null; stalled: boolean }> {
		const ring = messages().fileProgressRing(name);
		const [value, stalled] = await Promise.all([
			ring.getAttribute('aria-valuenow'),
			ring.getAttribute('data-stalled'),
		]);
		return {
			bytes: value === null ? null : Number(value),
			stalled: stalled === 'true',
		};
	}

	/** The bytes the ring reports, failing when it reports no figure — which
	 * is not the same as reporting zero bytes. */
	async function requireBytes(name: string): Promise<number> {
		const { bytes, stalled } = await readProgress(name);
		if (bytes === null) {
			throw new Error(
				stalled
					? `${name} download stalled`
					: `${name} progress ring shows no figure`,
			);
		}
		return bytes;
	}

	/** Wait until the ring reports more than `bytes`. A ring reporting no
	 * figure has not got there yet, so a download that stalls before it
	 * resumes keeps the wait alive rather than failing it. */
	async function waitForBytesAbove(
		name: string,
		bytes: number,
		timeoutMsg: string,
	): Promise<void> {
		await agent2.waitUntil(
			async () => {
				const now = (await readProgress(name)).bytes;
				return now !== null && now > bytes;
			},
			{ timeout: MEDIA_SYNC_TIMEOUT, timeoutMsg },
		);
	}

	/** The sender streams the bytes to the mailbox after publishing the
	 * message, so the receiver's first attempt can find the mailbox without
	 * them and its next comes a fetch pass later. */
	async function waitForDownloadStarted(name: string): Promise<void> {
		await messages()
			.fileProgressRing(name)
			.waitForDisplayed({ timeout: SYNC_TIMEOUT });
		await waitForBytesAbove(name, 0, `no byte of ${name} arrived`);
	}

	/** No reading may show fewer bytes than the one before it, and the last
	 * must show more than the first. */
	async function expectClimbing(name: string, readings: number): Promise<void> {
		const first = await requireBytes(name);
		let last = first;
		for (let i = 0; i < readings; i++) {
			await agent2.pause(READING_INTERVAL_MS);
			const now = await requireBytes(name);
			if (now < last) {
				throw new Error(
					`reading ${i + 1}: ${name} fell to ${now} bytes (was ${last})`,
				);
			}
			last = now;
		}
		if (last <= first) {
			throw new Error(
				`${name} stayed at ${first} bytes over ${readings} readings`,
			);
		}
	}

	/** Take the mailbox away for a moment, and answer with the bytes the
	 * receiver held throughout. Nothing fabricates progress locally: with the
	 * only source stopped, the reading is the bytes on disk and stays put. */
	async function bytesHeldWhileMailboxAway(name: string): Promise<number> {
		suspendMailbox();
		try {
			await agent2.pause(IN_FLIGHT_SETTLE_MS);
			const held = await requireBytes(name);
			await agent2.pause(AWAY_MS);
			expect(await requireBytes(name)).toBe(held);
			return held;
		} finally {
			resumeMailbox();
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
		await createProfiles({ Alice: agent1, Bob: agent2 });
		await exchangeContacts([agent1, agent2]);
		await setMailboxBlobThrottle(THROTTLE_BYTES_PER_SEC);
		throttled = true;
	});

	// Mocha runs this even when `before` skipped the suite or threw, and the
	// throttle helper is unavailable against a remote mailbox.
	after(async () => {
		if (throttled) await setMailboxBlobThrottle(null);
	});

	it('climbs the progress ring while the mailbox serves slowly', async () => {
		// Attachments are zero-filled, so each case needs its own size to be
		// its own blob.
		await send('slow.bin', 640 * 1024);
		await waitForDownloadStarted('slow.bin');
		await expectClimbing('slow.bin', CLIMBING_READINGS);
		await expectComplete('slow.bin');
	});

	it('holds the progress while the mailbox is away and climbs again once it is back', async () => {
		await send('flaky.bin', 768 * 1024);
		await waitForDownloadStarted('flaky.bin');
		await expectClimbing('flaky.bin', 2);

		const held = await bytesHeldWhileMailboxAway('flaky.bin');

		// The blob fetch loop retries on its own schedule, not on the
		// mailbox's return, so the climb is measured from the first byte that
		// proves the download picked up again.
		await waitForBytesAbove(
			'flaky.bin',
			held,
			'flaky.bin did not resume after the mailbox came back',
		);
		await expectClimbing('flaky.bin', CLIMBING_READINGS);
		await expectComplete('flaky.bin');
	});
});
