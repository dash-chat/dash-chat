/**
 * The chat list still shows the iPhone's chats after the user switched away
 * and back many times in a row (DASH-CHAT-5P, 5Q, 3K: "Lost all my chats"
 * going back and forth between Signal and Dash Chat). Anything slow on each
 * switch piles up over a burst of them, and a chat list query that gives up
 * meanwhile is never asked again.
 */
import { createProfiles } from '../helpers/flows/create-profiles';
import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgents } from '../setup/setup-agents';

const SWITCHES = 25;

describe('Chat list after rapid app switching on the iPhone', function () {
	this.timeout(600_000);

	let alice: Agent;
	let bob: Agent;

	before(async function () {
		[alice, bob] = await setupAgents(this, [
			{ platform: 'desktop' },
			{ platform: 'ios' },
		]);
		await createProfiles({ Alice: alice, Bob: bob });
		await exchangeContacts([alice, bob]);
	});

	it('still shows the chats once the switching stops', async () => {
		await alice.directChatPage.composer.sendMessage('before the switching');
		await bob.directChatPage.messages.waitForMessage('before the switching');
		await bob.directChatPage.back.click();
		await bob.homePage.ready();
		await bob.homePage.chatListItem('Alice').waitForExist();

		for (let i = 0; i < SWITCHES; i++) {
			await bob.backgroundApp();
			await bob.startApp();
		}

		expect(await bob.errorPlaceholder.root.isExisting()).toBe(false);
		await bob.homePage.chatListItem('Alice').waitForExist();
		await alice.directChatPage.composer.sendMessage('after the switching');
		await bob.homePage.chatListItem('Alice').click();
		await bob.directChatPage.ready();
		await bob.directChatPage.messages.waitForMessage('after the switching');
	});
});
