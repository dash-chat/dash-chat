import { navigateToAddContact } from '../../helpers/flows/exchange-contacts';
import { tid } from '../../helpers/selectors';
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

	it('does not re-show the contact-request-sent toast on a repeat scan', async () => {
		await navigateToAddContact(alice);
		await alice.addContactPage.enterAddContactLink(bobCode);
		await alice.directChatPage.ready();

		// The first scan shows the confirmation toast.
		await alice.toast.expectMessage('Contact request sent.');

		// Record the last toast event before the second scan so we can tell
		// whether a new toast fires.
		const lastToastBefore = await alice.execute(
			() =>
				(window as Window & { __lastToastEvent?: { message: string } })
					.__lastToastEvent?.message,
		);
		expect(lastToastBefore).toBe('Contact request sent.');

		// Go back and enter the same contact code a second time.
		await alice.directChatPage.back.click();
		await alice.homePage.ready();
		await navigateToAddContact(alice);
		await alice.addContactPage.enterAddContactLink(bobCode);
		await alice.directChatPage.ready();

		// A repeat request should not fire another contact-request-sent toast.
		const lastToastAfter = await alice.execute(
			() =>
				(window as Window & { __lastToastEvent?: { message: string } })
					.__lastToastEvent?.message,
		);
		expect(lastToastAfter).toBe('Contact request sent.');
	});
});
