import { navigateToAddContact } from '../../helpers/flows/exchange-contacts';
import { type Agent, setupAgents } from '../../setup/setup-agents';

describe('Double contact request', () => {
	let alice: Agent;
	let bob: Agent;
	let bobCode: string;

	before(async function () {
		[alice, bob] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await Promise.all([
			alice.createProfilePage.createProfile('Alice', 'Test'),
			bob.createProfilePage.createProfile('Bob', 'Test'),
		]);
		await navigateToAddContact(bob);
		bobCode = await bob.addContactPage.getAddContactLink();
	});

	it('reuses the same chat on a repeat scan, silently', async () => {
		await navigateToAddContact(alice);
		await alice.addContactPage.enterAddContactLink(bobCode);
		await alice.directChatPage.ready();

		// Sending the request is silent — the chat it opens is the confirmation.
		expect(await alice.toast.lastToastMessage()).toBe(undefined);

		// Go back and enter the same contact code a second time.
		await alice.directChatPage.back.click();
		await alice.homePage.ready();
		await navigateToAddContact(alice);
		await alice.addContactPage.enterAddContactLink(bobCode);
		await alice.directChatPage.ready();

		// A repeat request is not an error, so it stays silent too.
		expect(await alice.toast.lastToastMessage()).toBe(undefined);

		// It reopened the chat the first scan created rather than starting a
		// second request alongside it.
		await alice.directChatPage.back.click();
		await alice.homePage.ready();
		expect(await alice.homePage.chatRowCount()).toBe(1);
	});
});
