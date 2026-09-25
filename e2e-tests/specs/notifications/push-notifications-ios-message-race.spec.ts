import { backToHome } from '../../helpers/flows/back-to-home';
import { createProfiles } from '../../helpers/flows/create-profiles';
import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import { pushTestingEnabled } from '../../setup/push-server';
import { type Agent, setupAgents } from '../../setup/setup-agents';

/** Photos the Mac sends into the chat the iPhone has open, one after another.
 *  Each one races the push extension against the open app's own mailbox poll. */
const ROUNDS = 5;

/** Well past a mailbox round trip and the app's ack debounce, so a round only
 *  fails when the app never processes the message at all. */
const PROCESSED_TIMEOUT = 30_000;

/**
 * A message sent into a chat the iPhone has open must be processed by that
 * app. The message reaches the iPhone twice: as a push, which wakes the push
 * extension, and through the app's own mailbox poll. When the extension
 * fetches it first, it lands in the shared database, the app's poll then asks
 * only for what comes after it, and the running app never processes it.
 * Two of the effects only the app's processing has show it: the delivery ack
 * the Mac waits on (the extension sends none), and the fetch of the photo.
 *
 * Skips itself unless E2E_STRESS=1, and unless push testing is available (a
 * Firebase service-account key plus a mobile agent). Run it with:
 *   PLATFORMS=ios,desktop just e2e run notifications/push-notifications-ios-message-race
 */
describe('Photos sent into a chat open on the iPhone', function () {
	this.timeout(ROUNDS * 2 * PROCESSED_TIMEOUT + 600_000);

	let iphone: Agent;
	let mac: Agent;

	before(async function () {
		if (process.env.E2E_STRESS !== '1') this.skip();
		if (!pushTestingEnabled()) this.skip();
		[iphone, mac] = await setupAgents(this, [
			{ platform: 'ios' },
			{ platform: 'desktop' },
		]);
		// Over a direct connection the Mac hands each message to the app itself,
		// before any push can wake the extension.
		await iphone.disableP2p();
		await mac.disableP2p();
		await createProfiles({ Rex: iphone, Sam: mac });
		await exchangeContacts([iphone, mac]);
		await backToHome([iphone, mac]);
		// An extension process built before the contact existed does not fetch
		// the topic the photos arrive on.
		await iphone.killPushExtension();
		await iphone.homePage.openChat('Sam');
		await mac.homePage.openChat('Rex');
	});

	it('processes every photo message the Mac sends', async () => {
		const logBefore = iphone.readLog().length;
		for (let round = 1; round <= ROUNDS; round++) {
			const label = `race-${round}`;
			const caption = `photo ${label}`;
			await mac.directChatPage.composer.attachNoisePhoto(label, 64, 48);
			await mac.directChatPage.composer.sendMessage(caption);

			await mac.directChatPage.messages.waitForMessageStatus(
				caption,
				['delivered'],
				PROCESSED_TIMEOUT,
			);
			await iphone.directChatPage.messages.waitForPhotoMessage(
				label,
				PROCESSED_TIMEOUT,
			);
		}

		// Without a round the extension fetched first, the assertions above say
		// nothing about the race.
		const log = iphone.readLog().slice(logBefore);
		expect(log).toMatch(/PushNotificationsExtension.*Posted nse-did-process/);
		expect(log).toMatch(
			/Dash Chat\[.*importing operations the push extension processed/,
		);
	});
});
