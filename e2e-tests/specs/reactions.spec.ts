import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { createGroup } from '../helpers/flows/exchange-contacts-and-create-group';
import { type Agent, setupAgents } from '../setup/setup-agents';

describe('Message reactions', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		({ agent1, agent2 } = await setupAgents(this, {
			agent1: 'any',
			agent2: 'any',
		}));
		await agent1.enablePreviewFeatures();
		await agent2.enablePreviewFeatures();
		await agent1.createProfilePage.createProfile('Alice', 'Test');
		await agent2.createProfilePage.createProfile('Bob', 'Test');
		await exchangeContacts(agent1, agent2);
	});

	it('adds and removes a reaction in a direct chat', async () => {
		await agent1.directChatPage.sendMessage('React to me');
		await agent1.directChatPage.messages.waitForMessage('React to me');
		await agent2.directChatPage.messages.waitForMessage('React to me');

		await agent2.directChatPage.messages.reactWith('React to me', '👍');
		await agent2.directChatPage.messages.waitForReaction('React to me', '👍');
		await agent1.directChatPage.messages.waitForReaction('React to me', '👍');

		// Reacting with the same emoji again removes the reaction.
		await agent2.directChatPage.messages.reactWith('React to me', '👍');
		await agent2.directChatPage.messages.waitForNoReaction('React to me', '👍');
		await agent1.directChatPage.messages.waitForNoReaction('React to me', '👍');
	});

	it('adds a reaction in a group chat', async () => {
		await agent1.directChatPage.back.click();
		await agent2.directChatPage.back.click();
		await agent1.homePage.ready();
		await agent2.homePage.ready();

		await createGroup(agent1, 'mygroup', 'Bob');

		await agent2.homePage.chatListItem('mygroup').waitForExist();
		await agent2.homePage.chatListItem('mygroup').click();
		await agent2.groupChatPage.ready();

		await agent2.groupChatPage.sendMessage('React in group');
		await agent2.groupChatPage.messages.waitForMessage('React in group');
		await agent1.groupChatPage.messages.waitForMessage('React in group');

		await agent1.groupChatPage.messages.reactWith('React in group', '❤️');
		await agent1.groupChatPage.messages.waitForReaction('React in group', '❤️');
		await agent2.groupChatPage.messages.waitForReaction('React in group', '❤️');

		// Reacting with the same emoji again removes it.
		await agent1.groupChatPage.messages.reactWith('React in group', '❤️');
		await agent1.groupChatPage.messages.waitForNoReaction(
			'React in group',
			'❤️',
		);
		await agent2.groupChatPage.messages.waitForNoReaction(
			'React in group',
			'❤️',
		);
	});
});
