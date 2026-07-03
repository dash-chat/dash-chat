import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { createGroup } from '../helpers/flows/exchange-contacts-and-create-group';
import { type Agent, setupAgent } from '../setup/setup-agents';

describe('Message reactions', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async () => {
		[agent1, agent2] = await Promise.all([
			setupAgent('agent1'),
			setupAgent('agent2'),
		]);
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

		// Tapping the chip opens the who-reacted sheet; it does not remove the
		// reaction.
		await agent1.directChatPage.messages.openReactionsSheet(
			'React to me',
			'👍',
		);
		await agent1.waitUntil(() =>
			agent1.directChatPage.messages.reactionsSheetShowsReactor(
				'React to me',
				'Bob',
			),
		);
		expect(
			await agent1.directChatPage.messages.hasReaction('React to me', '👍'),
		).toBe(true);

		// Filtering by the emoji tab keeps the matching reactor visible.
		await agent1.directChatPage.messages.clickReactionsTab('React to me', '👍');
		await agent1.waitUntil(() =>
			agent1.directChatPage.messages.reactionsSheetShowsReactor(
				'React to me',
				'Bob',
			),
		);
		await agent1.directChatPage.messages.closeReactionsSheet('React to me');

		// The reactor sees their own row and removes the reaction from there.
		await agent2.directChatPage.messages.openReactionsSheet(
			'React to me',
			'👍',
		);
		await agent2.directChatPage.messages.removeOwnReaction('React to me');
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

		// The who-reacted sheet resolves group members' profiles.
		await agent2.groupChatPage.messages.openReactionsSheet(
			'React in group',
			'❤️',
		);
		await agent2.waitUntil(() =>
			agent2.groupChatPage.messages.reactionsSheetShowsReactor(
				'React in group',
				'Alice',
			),
		);
		await agent2.groupChatPage.messages.closeReactionsSheet('React in group');

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
