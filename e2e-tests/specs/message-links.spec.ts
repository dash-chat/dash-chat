import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgents } from '../setup/setup-agents';

describe('Links in messages', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await agent1.createProfilePage.createProfile('Alice', 'Test');
		await agent2.createProfilePage.createProfile('Bob', 'Test');
		await exchangeContacts(agent1, agent2);
	});

	it('linkifies urls with and without a scheme, for both sender and receiver', async () => {
		const text = 'check https://my.thing and dashchat.org';
		await agent1.directChatPage.composer.sendMessage(text);

		for (const agent of [agent1, agent2]) {
			const message = await agent.directChatPage.messages.waitForMessage(text);
			expect(await message.linkHrefs()).toEqual([
				'https://my.thing',
				'https://dashchat.org',
			]);
		}
	});

	it('hands a tapped link to the OS without leaving the chat', async function () {
		// The url only lands somewhere observable on desktop, where the harness
		// puts an `xdg-open` stub on the app's PATH. On a phone the tap would
		// hand the app's foreground to the system browser, so it stays skipped.
		const agent = [agent1, agent2].find(a => a.platform === 'desktop');
		if (!agent) this.skip();
		const message =
			await agent.directChatPage.messages.waitForMessage('https://my.thing');
		await message.tapLink('https://my.thing');
		// The OS gets the anchor's resolved form.
		expect(await agent.waitForOpenedUrls()).toEqual(['https://my.thing/']);
		await expect(agent.directChatPage.page).toBeDisplayed();
	});

	it('excludes trailing punctuation from the link', async () => {
		const text = 'visit dashchat.org.';
		await agent1.directChatPage.composer.sendMessage(text);
		const message = await agent1.directChatPage.messages.waitForMessage(text);
		expect(await message.linkHrefs()).toEqual(['https://dashchat.org']);
	});

	it('leaves text without a url alone', async () => {
		const text = 'no links here, 3.5 stars, i.e. none';
		await agent1.directChatPage.composer.sendMessage(text);
		const message = await agent1.directChatPage.messages.waitForMessage(text);
		expect(await message.linkHrefs()).toEqual([]);
	});
});
