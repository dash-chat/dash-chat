import { createProfiles } from '../../helpers/flows/create-profiles';
import {
	contactLinkOf,
	navigateToAddContact,
} from '../../helpers/flows/exchange-contacts';
import { SYNC_TIMEOUT } from '../../helpers/timeouts';
import { pushTestingEnabled } from '../../setup/push-server';
import { type Agent, setupAgents } from '../../setup/setup-agents';

/**
 * Reproduction of a user report (Sentry DASH-CHAT-7S): contacts the user had
 * exchanged messages with were missing from the new-group member picker until
 * the app was force-quit and reopened.
 *
 * The device log attached to the report showed the common factor: every
 * affected contact's ContactRequestAccept had been ingested by the iOS push
 * extension (recorded as extension-processed, then imported by the app),
 * while accepts processed live or by a manual in-app accept recorded the
 * contact fine. The accept handler spawns the publish of the AddContact op
 * that makes a contact a contact — after an extension-ingested accept that
 * publish never happens, so the chat exists (via the outgoing pending
 * request) but the contact never lands in the device-group log, and the
 * group member picker is driven by that log.
 *
 * Rex (iPhone) scans Sam's code, writes into the chat, and backgrounds. Sam
 * accepts while Rex is suspended, so only the push extension can ingest the
 * accept — no race with the app's own poll. Foregrounding Rex imports the
 * extension-processed op, the exact context the report's log shows failing.
 *
 * Needs a physical iPhone and a Firebase service-account key:
 *   PHONES=ios just e2e run notifications/push-notifications-ios-contact-accept
 */
describe('Contact accepted while the app is backgrounded (iOS push extension)', () => {
	let rex: Agent;
	let sam: Agent;

	before(async function () {
		if (!pushTestingEnabled()) this.skip();
		[rex, sam] = await setupAgents(this, [
			{ platform: 'ios' },
			{ platform: 'desktop' },
		]);
		// Over a direct connection Sam hands the accept to Rex's app himself,
		// before any push can wake the extension.
		await rex.disableP2p();
		await sam.disableP2p();
		await createProfiles({ Rex: rex, Sam: sam });
	});

	it('records the contact, offering them as a group member', async () => {
		// Rex scans Sam's code: Rex is the requester, so the acceptance comes
		// back on his private reply inbox.
		const samsLink = await contactLinkOf(sam);
		await navigateToAddContact(rex);
		await rex.addContactPage.enterAddContactLink(samsLink);
		await rex.directChatPage.ready();
		// The requester can write before being accepted — the reported case had
		// the user messaging the contacts that went missing.
		await rex.directChatPage.composer.sendMessage('hi before accept');
		await rex.directChatPage.back.click();
		await rex.homePage.ready();

		// The apps have to be settled on the relay before the first push: a
		// node only seconds old has no relay session for the extension to take.
		await rex.pause(15_000);
		// A fresh extension process builds its node from the current data; one
		// started before the request went out is not subscribed to the reply
		// inbox the accept lands on.
		await rex.killPushExtension();
		await rex.backgroundApp();

		// Sam accepts while Rex is suspended: the push extension is the only
		// process that can ingest the accept.
		const rexRow = sam.homePage.chatListItem('Rex');
		await rexRow.waitForExist({ timeout: SYNC_TIMEOUT });
		await rexRow.click();
		await sam.directChatPage.ready();
		await sam.directChatPage.acceptButton.waitForExist();
		await sam.directChatPage.acceptContactRequest();

		// The extension announcing it processed something is what puts the
		// import below on the accept's path; without it the assertions after it
		// say nothing about the reported bug.
		await rex.waitUntil(
			() =>
				/PushNotificationsExtension.*Posted nse-did-process/.test(
					rex.readLog(),
				),
			{
				timeout: SYNC_TIMEOUT,
				timeoutMsg:
					'the push extension never ingested the accept; the import path this reproduces went unexercised',
			},
		);

		await rex.startApp();
		await rex.homePage.ready();
		await rex.waitUntil(
			() =>
				/importing operations the push extension processed/.test(rex.readLog()),
			{
				timeout: SYNC_TIMEOUT,
				timeoutMsg:
					'the app never imported the operations the extension processed',
			},
		);

		// The chat shows even while the bug keeps the contact unrecorded: it is
		// listed via the outgoing pending request, not the device-group log.
		await rex.homePage.chatListItem('Sam').waitForExist({
			timeout: SYNC_TIMEOUT,
		});

		await rex.homePage.newMessageButton.click();
		await rex.newMessagePage.ready();
		await rex.newMessagePage.newGroup.click();
		await rex.newGroupPage.addMembersStep.ready();
		await rex.newGroupPage.addMembersStep.contactList
			.contactItem('Sam')
			.waitForExist({
				timeout: SYNC_TIMEOUT,
				timeoutMsg:
					'Sam was accepted via the push extension import but never recorded as a contact (no AddContact), so he is missing from the group member picker',
			});
	});
});
