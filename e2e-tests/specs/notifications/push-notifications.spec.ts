import {
	type NotificationHelper,
	notificationHelperFor,
} from '../../helpers/components/notifications';
import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import { pushTestingEnabled } from '../../setup/push-server';
import { type Agent, setupAgents } from '../../setup/setup-agents';

/**
 * Real-device, end-to-end push test: a message sent by one agent must arrive on
 * a backgrounded device receiver as an actual notification whose body carries
 * the message text — exercising the full pipeline (send → local mailbox →
 * local push-server → FCM → APNs/Android → NSE/OS → notification).
 *
 * Asserting the notification *content* (not just that some notification
 * appeared) is what catches the failure mode where the op can't be fetched in
 * time and the receiver falls back to a generic "You have a new message".
 *
 * Runs on either an iOS or an Android receiver. Only runs when `E2E_PUSH=1`
 */
describe('Push notifications (real device, end-to-end)', () => {
	let receiver: Agent;
	let sender: Agent;
	let notifications: NotificationHelper;

	before(async function () {
		if (!pushTestingEnabled()) this.skip();
		// The receiver must be a real device (it runs the OS notification path);
		// the sender can be anything sharing the same local mailbox.
		[receiver, sender] = await setupAgents(this, [
			{ platform: 'mobile' },
			{ platform: 'any' },
		]);
		notifications = notificationHelperFor(receiver);
	});

	it('creates profiles and exchanges contacts', async () => {
		await receiver.createProfilePage.createProfile('Rex', 'Test');
		await sender.createProfilePage.createProfile('Sam', 'Test');
		await exchangeContacts(receiver, sender);
	});

	it('delivers a push to a backgrounded app, carrying the message content', async () => {
		const marker = `PUSH_BG_${Date.now()}`;
		const message = `hi ${marker}`;

		await receiver.pause(5_000);
		await receiver.backgroundApp();

		await sender.directChatPage.composer.sendMessage(message);

		// Wait for *any* notification of ours, then check its content. Waiting
		// for one that already contains the marker would make the generic
		// fallback ("You have a new message") indistinguishable from no push
		// arriving — both just time out — whereas this fails with the actual body
		// in the message.
		const text = await notifications.waitForAppNotification();
		expect(text).toContain(marker);

		// Tap-to-navigate needs an unlocked device; best-effort, so the content
		// assertion above is the pass criterion.
		try {
			await notifications.tapNotification(message);
			await notifications.returnToApp();
			await receiver.directChatPage.ready();
			await receiver.directChatPage.messages.waitForMessage(message);
		} catch (err) {
			console.warn(`push tap-to-navigate skipped: ${err}`);
		}
	});

	it('delivers a push to a force-quit app, carrying the message content', async () => {
		const marker = `PUSH_QUIT_${Date.now()}`;
		const message = `hi ${marker}`;

		// Bring the app back so it can be quit properly, then terminate it — what
		// a user swiping it away in the app switcher does
		await receiver.startApp();
		await receiver.homePage.ready();
		await receiver.pause(5_000);
		await receiver.stopApp();

		await sender.directChatPage.composer.sendMessage(message);

		const text = await notifications.waitForAppNotification();
		expect(text).toContain(marker);
	});
});
