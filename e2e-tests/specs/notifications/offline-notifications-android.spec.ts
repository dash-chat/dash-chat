import {
	type NotificationHelper,
	notificationHelperFor,
} from '../../helpers/components/notifications';
import { exchangeContacts } from '../../helpers/flows/exchange-contacts';
import { type Agent, setupAgents } from '../../setup/setup-agents';

/**
 * Android offline notification test.
 *
 * Scenario: the receiver puts the app in "emergency mode" (Android power-save
 * / background restriction), then backgrounds the app. The sender, on a desktop
 * agent sharing the same local mailbox, sends a message. Because the receiver's
 * node keeps running in the background, it syncs the message via the local
 * mailbox and shows a local system notification — even though the app UI is
 * backgrounded and the network stack is constrained.
 *
 * This is *not* the FCM/APNs push path: it exercises the sync notification path
 * in src-tauri/src/notifications/mod.rs that fires when an operation arrives
 * through the regular sync pipeline while the app is backgrounded.
 */
// Skipped: the offline-mode toggle in the help page is disabled for now.
describe.skip('Offline notifications on Android (background sync)', () => {
	let receiver: Agent;
	let sender: Agent;
	let notifications: NotificationHelper;

	before(async function () {
		// Receiver must be a real Android device or emulator; sender can be desktop.
		[receiver, sender] = await setupAgents(this, [
			{ platform: 'android' },
			{ platform: 'desktop' },
		]);
		notifications = notificationHelperFor(receiver);
	});

	it('creates profiles and exchanges contacts', async () => {
		await receiver.createProfilePage.createProfile('Rex', 'Test');
		await sender.createProfilePage.createProfile('Sam', 'Test');
		await exchangeContacts(receiver, sender);
	});

	it('delivers a notification while the app is backgrounded in emergency mode', async () => {
		const marker = `OFFLINE_${Date.now()}`;
		const message = `offline hi ${marker}`;

		// Enable offline mode on the Android receiver from the help screen.
		await receiver.directChatPage.back.click();
		await receiver.homePage.ready();
		await receiver.homePage.settingsLink.waitForExist();
		await receiver.homePage.settingsLink.click();
		await receiver.settingsPage.ready();
		await receiver.settingsPage.helpLink.click();
		await receiver.helpPage.ready();
		await receiver.helpPage.enableOfflineMode();

		// Background the app. The background service keeps the node alive.
		await receiver.backgroundApp();
		await receiver.pause(5_000);

		await sender.directChatPage.composer.sendMessage(message);

		// The backgrounded Android node should sync the op and surface a notification.
		const text = await notifications.waitForNotification(marker);
		expect(text).toContain(marker);

		// Tap the notification and verify we return to the chat with the message.
		await notifications.tapNotification(marker);
		await notifications.returnToApp();
		await receiver.directChatPage.ready();
		await receiver.directChatPage.messages.waitForMessage(message);
	});
});
