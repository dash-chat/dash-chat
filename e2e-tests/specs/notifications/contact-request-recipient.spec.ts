/**
 * A contact request is announced only on the device it was addressed to.
 *
 * Entering someone's link publishes into the inbox their QR code advertises,
 * which subscribes the sender to that inbox — so every request anyone else
 * sends them arrives on our device too. The notification used to be built
 * straight from the operation, so a phone announced "New contact request" for
 * a stranger who had written to one of its contacts, opening a chat it has no
 * part in and disclosing who had contacted whom.
 *
 * Rex enters Bob's link, which is what puts Bob's inbox on Rex's device.
 * Grace, a stranger to Rex, then writes to that same inbox and to Rex's own:
 * only the one addressed to Rex may show.
 *
 * Needs a physical Android receiver (its notifications are read through
 * dumpsys) and a Firebase service-account key:
 *   PLATFORMS=android,desktop,desktop just e2e run notifications/contact-request-recipient
 */
import {
	type NotificationHelper,
	notificationHelperFor,
} from '../../helpers/components/notifications';
import { backToHome } from '../../helpers/flows/back-to-home';
import { createProfiles } from '../../helpers/flows/create-profiles';
import {
	contactLinkOf,
	navigateToAddContact,
} from '../../helpers/flows/exchange-contacts';
import { pushTestingEnabled } from '../../setup/push-server';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Contact requests reach only the device they were sent to', () => {
	let rex: Agent;
	let bob: Agent;
	let grace: Agent;
	let rexsLink: string;
	let notifications: NotificationHelper;

	before(async function () {
		if (!pushTestingEnabled()) this.skip();
		[rex, bob, grace] = await setupAgents(this, [
			{ platform: 'android' },
			{ platform: 'desktop' },
			{ platform: 'desktop' },
		]);
		notifications = notificationHelperFor(rex);
		await createProfiles({ Rex: rex, Bob: bob, Grace: grace });
		rexsLink = await contactLinkOf(rex);

		const bobsLink = await contactLinkOf(bob);
		await navigateToAddContact(rex);
		await rex.addContactPage.enterAddContactLink(bobsLink);
		await rex.directChatPage.ready();
		await backToHome([rex]);
		await notifications.clear();
		await rex.backgroundApp();

		await navigateToAddContact(grace);
		await grace.addContactPage.enterAddContactLink(bobsLink);
		await grace.directChatPage.ready();
		// Bob showing the request is what puts it beyond the mailbox: from here
		// Rex's next sync of that inbox carries it.
		await bob.homePage.chatListItem('Grace').waitForExist();
	});

	afterEach(async function () {
		if (this.currentTest?.state === 'failed') await notifications.recover();
	});

	it('announces only the request addressed to Rex', async () => {
		await backToHome([grace]);
		await navigateToAddContact(grace);
		await grace.addContactPage.enterAddContactLink(rexsLink);
		await grace.directChatPage.ready();

		// Grace wrote to Bob's inbox before Rex's, and Rex syncs both, so once
		// the request addressed to Rex is on the shade the other one has had
		// its chance.
		await notifications.waitForNotification('Grace');

		const delivered = await notifications.delivered();
		expect(delivered.map(n => n.texts.join(' | '))).toEqual([
			expect.stringContaining('Grace'),
		]);
	});
});
