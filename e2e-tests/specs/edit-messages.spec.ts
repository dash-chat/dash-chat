import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgent } from '../setup/setup-agents';

describe('Editing messages', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async () => {
		[agent1, agent2] = await Promise.all([
			setupAgent('agent1'),
			setupAgent('agent2'),
		]);
		await agent1.createProfilePage.createProfile('Alice', 'Test');
		await agent2.createProfilePage.createProfile('Bob', 'Test');
		await exchangeContacts(agent1, agent2);
	});

	it('edits a message in place and shows the "Edited" indicator on both sides', async () => {
		await agent1.directChatPage.sendMessage('Helo world');
		await agent1.directChatPage.messages.waitForMessage('Helo world');
		await agent2.directChatPage.messages.waitForMessage('Helo world');

		await agent1.directChatPage.editMessage('Helo world', 'Hello world');

		// Author and peer both converge on the corrected text in place.
		await agent1.directChatPage.messages.waitForMessage('Hello world');
		await agent2.directChatPage.messages.waitForMessage('Hello world');

		await browser.waitUntil(
			() => agent1.directChatPage.messages.hasEditedIndicator('Hello world'),
			{ timeoutMsg: 'No "Edited" indicator on the author side' },
		);
		await browser.waitUntil(
			() => agent2.directChatPage.messages.hasEditedIndicator('Hello world'),
			{ timeoutMsg: 'No "Edited" indicator on the peer side' },
		);
	});

	it('shows the full edit history, original first', async () => {
		await agent1.directChatPage.messages.openEditHistory('Hello world');
		const versions = await agent1.directChatPage.editHistoryVersions();
		expect(versions).toEqual(['Hello world', 'Helo world']);
	});

	it('does not offer Edit on the peer’s messages', async () => {
		await agent2.directChatPage.sendMessage("Bob's message");
		await agent1.directChatPage.messages.waitForMessage("Bob's message");

		await agent1.directChatPage.messages.openActions("Bob's message");
		expect(await agent1.directChatPage.quickEditButton.isExisting()).toBe(
			false,
		);
	});
});
