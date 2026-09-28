import { createProfiles } from '../../helpers/flows/create-profiles';
import { navigateToAddContact } from '../../helpers/flows/exchange-contacts';
import { SYNC_TIMEOUT } from '../../helpers/timeouts';
import {
	isRemoteMailbox,
	resumeMailbox,
	suspendMailbox,
} from '../../setup/mailbox-control';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Contact accepted over p2p while the scanner is away', () => {
	let alice: Agent; // the scanned contact
	let bob: Agent; // the scanner
	let mailboxSuspended = false;

	before(async function () {
		// The accept must only be able to travel peer to peer.
		if (isRemoteMailbox()) this.skip();
		[alice, bob] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await createProfiles({ Alice: alice, Bob: bob });
		suspendMailbox();
		mailboxSuspended = true;
	});

	after(() => {
		if (mailboxSuspended) resumeMailbox();
	});

	it('lists Alice in Bob’s new-group picker after both come back', async () => {
		await navigateToAddContact(alice);
		const aliceCode = await alice.addContactPage.getAddContactLink();
		await alice.addContactPage.back.click();
		await alice.newMessagePage.back.click();
		await alice.homePage.ready();

		await navigateToAddContact(bob);
		await bob.addContactPage.enterAddContactLink(aliceCode);
		await bob.directChatPage.ready();

		const bobRow = alice.homePage.chatListItem('Bob');
		await bobRow.waitForExist({ timeout: SYNC_TIMEOUT });
		await bob.stopApp();

		await bobRow.click();
		await alice.directChatPage.acceptButton.waitForExist();
		await alice.directChatPage.acceptContactRequest();
		await alice.directChatPage.acceptButton.waitForExist({ reverse: true });
		await alice.restart();
		await alice.homePage.ready();

		await bob.startApp();
		await bob.homePage.ready();
		await bob.homePage.newMessageButton.click();
		await bob.newMessagePage.ready();
		await bob.newMessagePage.newGroup.click();
		await bob.newGroupPage.ready();
		await bob.newGroupPage.addMembersStep.contactList
			.contactItem('Alice')
			.waitForExist({ timeout: SYNC_TIMEOUT });
	});
});
