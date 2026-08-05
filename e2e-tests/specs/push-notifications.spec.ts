import {
	type NotificationHelper,
	notificationHelperFor,
} from '../helpers/components/notifications';
import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { pushTestingEnabled } from '../setup/push-server';
import { type Agent, setupAgents } from '../setup/setup-agents';

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
 * Runs on either an iOS or an Android receiver (the notification helper switches
 * on platform). Only runs when `FCM_SERVICE_ACCOUNT_KEY` points at a Firebase
 * service-account JSON (so the harness spawns the push-server) and PLATFORMS
 * includes a mobile device; otherwise it skips.
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

	it('delivers a backgrounded push carrying the message content', async () => {
		const marker = `PUSH_${Date.now()}`;
		const message = `hi ${marker}`;

		// Give the receiver a moment to register its push token after the OS
		// permission prompt (auto-accepted) and contact exchange, before it goes
		// to the background.
		await receiver.pause(5_000);
		await notifications.background();

		await sender.directChatPage.composer.sendMessage(message);

		// The notification body must contain the actual message text — a generic
		// "You have a new message" fallback fails this, which is the point.
		const text = await notifications.waitForNotification(marker);
		expect(text).toContain(marker);

		// Tapping routes the app to the chat; assert we land there with the message.
		await notifications.tapNotification(marker);
		await notifications.returnToApp();
		await receiver.directChatPage.ready();
		await receiver.directChatPage.messages.waitForMessage(message);
	});
});
