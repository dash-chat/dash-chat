import { notificationHelperFor } from '../../helpers/components/notifications';
import type { NotificationHelper } from '../../helpers/components/notifications';
import { createProfiles } from '../../helpers/flows/create-profiles';
import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import {
	disableAndroidWifi,
	enableAndroidWifi,
} from '../../setup/platforms/android';
import { pushTestingEnabled } from '../../setup/push-server';
import { type Agent, setupAgents } from '../../setup/setup-agents';
import { deviceUdid } from '../../setup/webview';

/**
 * Photos that could not be fetched while the device had no route to their
 * sources must arrive once it does, without relaunching the app.
 *
 * Taken from a real report: for four minutes the device logged 390 connection
 * timeouts, 19 failed dials and not one successful connection, while polling
 * the mailbox over HTTPS 184 times. Messages arrived and photos did not,
 * because operations travel over HTTPS and blobs only over iroh. Every fetch in
 * that window read `source_count=3 fetched=false` — it knew three places to ask
 * and could not open a connection to any of them.
 *
 * Taking the device's Wi-Fi away reproduces that split rather than simulating
 * it: the mailbox stays reachable over the USB `adb reverse`, so operations
 * keep flowing and the gallery lays out its cells, while iroh has no route to
 * anyone. Both ends also run with p2p off, so nothing can arrive peer-to-peer
 * and leave the route in doubt. None of that is the bug — with no route the
 * bytes genuinely cannot come, and the test does not pretend otherwise.
 *
 * The bug is what happens after. Wi-Fi returns, the mailbox is reachable, every
 * photo is fetchable — and the question is whether anything still wants them. A
 * cell whose request was refused while the device was cut off is one nothing
 * re-asks for, so only a relaunch rebuilds the requests, which is why
 * restarting appears to fix it and waiting does not.
 *
 * The receiver must be on Wi-Fi rather than cellular: a device with mobile data
 * keeps a route when Wi-Fi drops, nothing is cut off, and this passes while
 * testing nothing.
 *
 * Needs a real mobile receiver with push configured.
 */

/** A batch, sent as one message — and exactly as many as the gallery lays out
 * as cells, so every photo sent is one the chat view shows. */
const PHOTO_COUNT = 5;

/** What a phone camera produces after the composer's re-encode. */
const PHOTO = { w: 1600, h: 1200 };

/** Long enough for the gallery to be on screen and to have asked for every one
 * of its photos, which is all the outage has to outlast. The route comes back
 * immediately afterwards — the photos are then fetchable within seconds, and
 * whether they appear is the whole question. */
const LOOKED_AT_MS = 5_000;

/** What the photos get once a route exists again. Past the Wi-Fi
 * reassociation the harness allows, so a photo still missing at the end is
 * missing rather than waiting on the radio. */
const RENDER_BUDGET_MS = 120_000;

/** What sending the batch gets. */
const SEND_BUDGET_MS = 180_000;

/** Per-photo wait inside a sweep. Short because a sweep retries. */
const SWEEP_WAIT_MS = 3_000;

describe('Media after a spell with no route', function () {
	this.timeout(SEND_BUDGET_MS + RENDER_BUDGET_MS * 4);

	let receiver: Agent;
	let sender: Agent;
	let notifications: NotificationHelper;
	let udid: string;
	const senderName = 'Sam';
	const receiverName = 'Rex';
	const labels = Array.from(
		{ length: PHOTO_COUNT },
		(_, i) => `photo-${i}-${Date.now()}`,
	);
	let cutOff = false;

	before(async function () {
		if (!pushTestingEnabled()) this.skip();
		// The receiver is the one that loses its route, so it has to be the
		// Android device; the sender only has to share the mailbox.
		[receiver, sender] = await setupAgents(this, [
			{ platform: 'android' },
			{ platform: 'any' },
		]);
		udid = deviceUdid(receiver);
		notifications = notificationHelperFor(receiver);
		await Promise.all([receiver.disableP2p(), sender.disableP2p()]);
		await createProfiles({
			[receiverName]: receiver,
			[senderName]: sender,
		});
		await exchangeContacts([receiver, sender]);
	});

	afterEach(async function () {
		// A device left without Wi-Fi outlives a failed test and would strand
		// every spec after it; a failure between native-context calls leaves the
		// driver in NATIVE_APP.
		if (cutOff) {
			await enableAndroidWifi(udid);
			cutOff = false;
		}
		if (this.currentTest?.state === 'failed') await notifications.recover();
	});

	it('shows the photos once the device has a route again', async () => {
		// Out of the chat before quitting: a receiver sitting on the sender's
		// conversation is not notified about it, so quitting from here is what
		// makes the wake-up arrive at all.
		await receiver.directChatPage.back.click();
		await receiver.homePage.ready();
		await receiver.pause(5_000);
		await receiver.stopApp();

		const composer = sender.directChatPage.composer;
		for (const label of labels) {
			await composer.attachNoisePhoto(label, PHOTO.w, PHOTO.h);
		}
		await composer.send();
		await sender.waitUntil(
			async () =>
				(await sender.directChatPage.messages.lastMessageStatus()) ===
				'mailbox',
			{
				timeout: SEND_BUDGET_MS,
				timeoutMsg: 'The batch never reached the mailbox',
			},
		);

		// The route goes before the app opens. The mailbox is still reachable
		// over USB, so the message and its gallery still arrive — which is the
		// reported split: text through, photos not.
		await disableAndroidWifi(udid);
		cutOff = true;

		await notifications.tapNotification(senderName);
		await notifications.returnToApp();
		await receiver.directChatPage.ready();
		await receiver.pause(LOOKED_AT_MS);

		// Straight back on, having looked at the photos and seen nothing. The
		// bytes are reachable again well before the app next tries for them:
		// a fetch that failed with no route waits out a whole fetch interval,
		// by which time the request the cell made has long since been refused.
		await enableAndroidWifi(udid);
		cutOff = false;

		// Swept rather than waited on one at a time: a photo that has not been
		// asked for yet has not failed, and a full budget spent on the first
		// would leave nothing for the rest.
		const deadline = Date.now() + RENDER_BUDGET_MS;
		let missing = [...labels];
		while (missing.length > 0 && Date.now() < deadline) {
			const stillMissing: string[] = [];
			for (const label of missing) {
				try {
					await receiver.directChatPage.messages.waitForPhotoMessage(
						label,
						SWEEP_WAIT_MS,
					);
				} catch {
					stillMissing.push(label);
				}
			}
			missing = stillMissing;
		}

		if (missing.length > 0) {
			throw new Error(
				`${missing.length} of ${PHOTO_COUNT} photos never appeared within ` +
					`${RENDER_BUDGET_MS}ms of the device regaining a route seconds after ` +
					`the photos were looked at, though its ` +
					`sources were reachable for all of it. Photos refused while there was ` +
					`no route are photos nothing asks for again.`,
			);
		}
	});
});
