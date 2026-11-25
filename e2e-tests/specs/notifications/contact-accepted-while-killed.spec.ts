import { createProfiles } from '../../helpers/flows/create-profiles';
import {
	contactLinkOf,
	navigateToAddContact,
} from '../../helpers/flows/exchange-contacts';
import { SYNC_TIMEOUT } from '../../helpers/timeouts';
import { pushTestingEnabled } from '../../setup/push-server';
import { type Agent, setupAgents } from '../../setup/setup-agents';

/**
 * Rex scans Sam's code and quits the app; Sam accepts and writes. The push
 * for the acceptance wakes the phone's push handler, which takes it in before
 * the app does, so the app first meets it at its next start.
 *
 * Needs a physical phone and a Firebase service-account key:
 *   PHONES=android just e2e run notifications/contact-accepted-while-killed
 */
describe('Contact accepted while the app is killed', () => {
	let rex: Agent;
	let sam: Agent;

	before(async function () {
		if (!pushTestingEnabled()) this.skip();
		[rex, sam] = await setupAgents(this, [
			{ platform: 'mobile' },
			{ platform: 'any' },
		]);
		await createProfiles({ Rex: rex, Sam: sam });
	});

	it('opens the chat with what the contact wrote once the app is back', async () => {
		const samsLink = await contactLinkOf(sam);
		await navigateToAddContact(rex);
		await rex.addContactPage.enterAddContactLink(samsLink);
		await rex.directChatPage.ready();
		await rex.directChatPage.awaitingAcceptBar.waitForExist();
		await rex.directChatPage.back.click();
		await rex.homePage.ready();
		// The request has to be out before the app goes away.
		const rexRow = sam.homePage.chatListItem('Rex');
		await rexRow.waitForExist({ timeout: SYNC_TIMEOUT });
		await rex.stopApp();

		await rexRow.click();
		await sam.directChatPage.ready();
		await sam.directChatPage.acceptContactRequest();
		await sam.directChatPage.composer.messageInput.waitForExist({
			timeout: SYNC_TIMEOUT,
		});
		const message = `hi ACCEPTED_WHILE_KILLED_${Date.now()}`;
		await sam.directChatPage.composer.sendMessage(message);
		// Long enough for the acceptance's push to wake the push handler and
		// for the handler to take the acceptance in, ahead of the app.
		await rex.pause(30_000);

		await rex.startApp();
		await rex.homePage.ready();
		await rex.homePage.openChat('Sam');
		await rex.directChatPage.ready();
		await rex.directChatPage.composer.messageInput.waitForExist({
			timeout: SYNC_TIMEOUT,
		});
		await rex.directChatPage.messages.waitForMessage(message);
		await sam.directChatPage.messages.waitForMessageStatus(message, [
			'delivered',
		]);
	});
});
