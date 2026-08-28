import {
	type NotificationHelper,
	notificationHelperFor,
} from '../../helpers/components/notifications';
import { deleteAccount } from '../../helpers/flows/delete-account';
import { navigateToAddContact } from '../../helpers/flows/exchange-contacts';
import { pushTestingEnabled } from '../../setup/push-server';
import { type Agent, setupAgents } from '../../setup/setup-agents';

/**
 * Real-device, end-to-end push tests: operations produced by one agent must
 * arrive on a backgrounded or quit receiver device as actual OS notifications
 * whose content carries the payload (contact-request name, message text) and
 * whose tap navigates to the chat the operation belongs to — exercising the
 * full pipeline (send → local mailbox → local push-server → FCM →
 * APNs/Android → NSE/OS → notification → tap route).
 *
 * Asserting the notification *content* (not just that some notification
 * appeared) is what catches the failure mode where the op can't be fetched in
 * time and the receiver falls back to a generic "You have a new message":
 * waiting for the expected content instead would make that fallback
 * indistinguishable from no push arriving — both just time out.
 *
 * Tap-to-navigate assertions need the receiver device unlocked for the whole
 * run.
 *
 * Runs on either an iOS or an Android receiver. Only runs when `E2E_PUSH=1`.
 */
describe('Push notifications (real device, end-to-end)', () => {
	let receiver: Agent;
	let sender: Agent;
	let notifications: NotificationHelper;
	let receiverLink: string;

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

	afterEach(async function () {
		// A failure between native-context helper calls (e.g. a content
		// assertion after a successful wait) leaves the receiver's driver in
		// NATIVE_APP; restore it so one failure doesn't cascade CSS errors
		// into every following test.
		if (this.currentTest?.state === 'failed') await notifications.recover();
	});

	it('creates profiles; the receiver shares its contact link', async () => {
		await receiver.createProfilePage.createProfile('Rex', 'Test');
		await sender.createProfilePage.createProfile('Sam', 'Test');

		await navigateToAddContact(receiver);
		receiverLink = await receiver.addContactPage.getAddContactLink();
		await receiver.addContactPage.back.click();
		await receiver.newMessagePage.back.click();
		await receiver.homePage.ready();
	});

	it('delivers a contact-request push whose tap opens the request', async () => {
		await receiver.pause(1_000);
		await receiver.backgroundApp();

		await navigateToAddContact(sender);
		await sender.addContactPage.enterAddContactLink(receiverLink);
		await sender.directChatPage.ready();

		const text = await notifications.waitForAppNotification();
		expect(text).toContain('Sam');

		// Tap by the title: OEM shades (MIUI) render only the title while the
		// notification is collapsed, so the body text is not a tappable anchor.
		await notifications.tapNotification('New contact request');
		await notifications.returnToApp();
		await receiver.directChatPage.ready();
		await expect(receiver.directChatPage.peerName).toHaveText(
			expect.stringContaining('Sam'),
		);
		await receiver.directChatPage.acceptContactRequest();
	});

	it('a second contact-request push taps through to its own chat', async () => {
		await receiver.pause(1_000);
		await receiver.backgroundApp();

		// A second request needs a fresh identity: recreating the sender's
		// account gives it a new device key and thus a new direct-chat topic.
		await sender.directChatPage.back.click();
		await sender.homePage.ready();
		await deleteAccount(sender);
		await sender.createProfilePage.createProfile('Zoe', 'Test');

		await navigateToAddContact(sender);
		await sender.addContactPage.enterAddContactLink(receiverLink);
		await sender.directChatPage.ready();

		const text = await notifications.waitForAppNotification();
		expect(text).toContain('Zoe');

		// The receiver was backgrounded while sitting on Sam's chat — staying
		// there instead of switching to Zoe's chat is the regression this
		// assertion guards against.
		await notifications.tapNotification('New contact request');
		await notifications.returnToApp();
		await receiver.directChatPage.ready();
		await expect(receiver.directChatPage.peerName).toHaveText(
			expect.stringContaining('Zoe'),
		);
		await receiver.directChatPage.acceptContactRequest();
	});

	it('delivers a message push to a backgrounded app whose tap opens the chat', async () => {
		const marker = `PUSH_BG_${Date.now()}`;
		const message = `hi ${marker}`;

		await receiver.directChatPage.back.click();
		await receiver.homePage.ready();
		await receiver.pause(1_000);
		await receiver.backgroundApp();

		await sender.directChatPage.composer.sendMessage(message);

		const text = await notifications.waitForAppNotification();
		expect(text).toContain(marker);

		await notifications.tapNotification('Zoe');
		await notifications.returnToApp();
		await receiver.directChatPage.ready();
		await expect(receiver.directChatPage.peerName).toHaveText(
			expect.stringContaining('Zoe'),
		);
		await receiver.directChatPage.messages.waitForMessage(message);
	});

	it('delivers a message push to a force-quit app whose tap cold-starts into the chat', async () => {
		const marker = `PUSH_QUIT_${Date.now()}`;
		const message = `hi ${marker}`;

		// Quit from the home page — what a user swiping the app away in the app
		// switcher does.
		await receiver.directChatPage.back.click();
		await receiver.homePage.ready();
		await receiver.pause(5_000);
		await receiver.stopApp();

		await sender.directChatPage.composer.sendMessage(message);

		const text = await notifications.waitForAppNotification();
		expect(text).toContain(marker);

		await notifications.tapNotification('Zoe');
		await notifications.returnToApp();
		await receiver.directChatPage.ready();
		await expect(receiver.directChatPage.peerName).toHaveText(
			expect.stringContaining('Zoe'),
		);
		await receiver.directChatPage.messages.waitForMessage(message);
	});

	it('a notification already in the shade taps through after the app was launched by a different one', async () => {
		const marker = `PUSH_SHADE_${Date.now()}`;
		const message = `hi ${marker}`;

		// Park a message notification from Zoe in the shade of the killed app,
		// then have a contact request from a fresh identity launch the app: its
		// route sits in the launch intent when Zoe's older notification is
		// tapped from the foreground — the state where the launch route can
		// shadow the tapped notification's route.
		await receiver.directChatPage.back.click();
		await receiver.homePage.ready();
		await receiver.pause(5_000);
		await receiver.stopApp();

		await sender.directChatPage.composer.sendMessage(message);
		await notifications.waitForNotification(marker);

		await sender.directChatPage.back.click();
		await sender.homePage.ready();
		await deleteAccount(sender);
		await sender.createProfilePage.createProfile('Ben', 'Test');
		await navigateToAddContact(sender);
		await sender.addContactPage.enterAddContactLink(receiverLink);
		await sender.directChatPage.ready();

		await notifications.waitForNotification('Ben');
		await notifications.tapNotification('New contact request');
		await notifications.returnToApp();
		await receiver.directChatPage.ready();
		await expect(receiver.directChatPage.peerName).toHaveText(
			expect.stringContaining('Ben'),
		);

		await notifications.waitForNotification(marker);
		await notifications.tapNotification('Zoe');
		await notifications.returnToApp();
		await receiver.directChatPage.ready();
		await expect(receiver.directChatPage.peerName).toHaveText(
			expect.stringContaining('Zoe'),
		);
		await receiver.directChatPage.messages.waitForMessage(message);
	});
});
