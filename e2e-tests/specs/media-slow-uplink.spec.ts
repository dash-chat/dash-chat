/**
 * A photo sent over a slow uplink reaches the receiver once the link has had
 * time to carry its bytes, however long that is past any single request's
 * timeout. The budget ends before the mailbox's own fetch from the sender
 * starts, which would otherwise deliver the photo around the slow link.
 */
import { createProfiles } from '../helpers/flows/create-profiles';
import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { isRemoteMailbox, shapeMailboxLink } from '../setup/mailbox-control';
import { type Agent, setupAgents } from '../setup/setup-agents';

const UPLINK = {
	latencyMs: 100,
	jitterMs: 0,
	rateKBps: 15,
	stall: { share: 0, extraMs: 0 },
};
const PHOTO = 'over-slow-uplink';
/** Noise barely compresses, so this lands at about 500 KB of JPEG: 33 s at
 *  15 KB/s, past the 30 s an upload of that size gets. */
const PHOTO_WIDTH = 980;
const PHOTO_HEIGHT = 735;
const DELIVERY_BUDGET_MS = 40_000;

describe('Media over a slow uplink', function () {
	this.timeout(300_000);

	let sender: Agent;
	let receiver: Agent;

	before(async function () {
		if (isRemoteMailbox()) this.skip();
		[sender, receiver] = await setupAgents(this, [
			{ platform: 'desktop' },
			{ platform: 'desktop' },
		]);
		await Promise.all([sender.disableP2p(), receiver.disableP2p()]);
		await createProfiles({ Sender: sender, Receiver: receiver });
		await exchangeContacts([sender, receiver]);
		await shapeMailboxLink(UPLINK);
	});

	it('delivers a photo the uplink can carry within the budget', async () => {
		await sender.directChatPage.composer.attachNoisePhoto(
			PHOTO,
			PHOTO_WIDTH,
			PHOTO_HEIGHT,
		);
		await sender.directChatPage.composer.send();
		const messages = receiver.directChatPage.messages;
		// A photo that isn't on the mailbox yet errors and waits for a tap, as
		// a user would give it.
		await receiver.waitUntil(
			async () => {
				if (await messages.photoRetry.isDisplayed()) {
					await messages.photoRetry.click();
				}
				return await messages.isPhotoLoaded(PHOTO);
			},
			{
				timeout: DELIVERY_BUDGET_MS,
				interval: 2_000,
				timeoutMsg: `the photo did not reach the receiver within ${DELIVERY_BUDGET_MS / 1000}s over ${JSON.stringify(UPLINK)}`,
			},
		);
	});
});
