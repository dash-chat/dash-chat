import {
	type NotificationHelper,
	notificationHelperFor,
} from '../../helpers/components/notifications';
import { backToHome } from '../../helpers/flows/back-to-home';
import { createProfiles } from '../../helpers/flows/create-profiles';
import { contactLinkOf } from '../../helpers/flows/exchange-contacts';
import { createGroup } from '../../helpers/flows/exchange-contacts-and-create-group';
import { type Agent, setupAgents } from '../../setup/setup-agents';

/**
 * A notification tap must land on the notification's chat even when the app
 * was earlier opened from a contact deep link.
 *
 * The deep-link plugin keeps returning the link that opened the app for the
 * life of the process. If handling the tap re-reads that link as a fresh one,
 * the app adds the contact again and navigates to their direct chat, away from
 * the chat the user tapped.
 */
describe('Notification tap after opening the app from a deep link', () => {
	let receiver: Agent;
	let sender: Agent;
	let notifications: NotificationHelper;

	before(async function () {
		[receiver, sender] = await setupAgents(this, [
			{ platform: 'mobile' },
			{ platform: 'any' },
		]);
		notifications = notificationHelperFor(receiver);
		await createProfiles({ Rex: receiver, Sam: sender });
	});

	afterEach(async function () {
		if (this.currentTest?.state === 'failed') await notifications.recover();
	});

	it('opens the tapped group chat, not the chat of the deep-linked contact', async () => {
		await receiver.handleDeepLink(await contactLinkOf(sender));
		await receiver.directChatPage.ready();
		await backToHome([receiver]);

		// The tapped notification has to belong to a chat other than the
		// deep-linked contact's, or landing on the wrong one goes unnoticed.
		await sender.homePage.openChat('Rex');
		await sender.directChatPage.acceptContactRequest();
		await backToHome([sender]);
		await createGroup(sender, 'Tapped group', ['Rex']);

		// The receiver stays on its chat list, so the message arrives through
		// sync and the app posts the notification itself.
		await notifications.clear();
		const message = `hi DEEPLINK_TAP_${Date.now()}`;
		await sender.groupChatPage.composer.sendMessage(message);
		await notifications.waitForNotification(message);

		await notifications.tapNotification('Sam');
		await notifications.returnToApp();
		await receiver.groupChatPage.ready();
		await receiver.groupChatPage.messages.waitForMessage(message);
		await expect(receiver.directChatPage.page).not.toBeExisting();
	});
});
