import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { createGroup } from '../helpers/flows/exchange-contacts-and-create-group';
import { SYNC_TIMEOUT } from '../helpers/timeouts';
import { type Agent, setupAgents } from '../setup/setup-agents';

describe('Message reactions', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await agent1.enablePreviewFeatures();
		await agent2.enablePreviewFeatures();
		await agent1.createProfilePage.createProfile('Alice', 'Test');
		await agent2.createProfilePage.createProfile('Bob', 'Test');
		await exchangeContacts(agent1, agent2);
	});

	it('adds and removes a reaction in a direct chat', async () => {
		await agent1.directChatPage.composer.sendMessage('React to me');
		const message1 =
			await agent1.directChatPage.messages.waitForMessage('React to me');
		const message2 =
			await agent2.directChatPage.messages.waitForMessage('React to me');

		await message2.reactWith('👍');
		await message2.waitForReaction('👍');
		await message1.waitForReaction('👍');

		// Tapping the chip opens the who-reacted sheet; it does not remove the
		// reaction.
		await message1.openReactionsSheet('👍');
		await agent1.waitUntil(() => message1.reactionsSheetShowsReactor('Bob'));
		expect(await message1.hasReaction('👍')).toBe(true);

		// Filtering by the emoji tab keeps the matching reactor visible.
		await message1.clickReactionsTab('👍');
		await agent1.waitUntil(() => message1.reactionsSheetShowsReactor('Bob'));
		await message1.closeReactionsSheet();

		// The reactor sees their own row and removes the reaction from there.
		await message2.openReactionsSheet('👍');
		await message2.removeOwnReaction();
		await message2.waitForNoReaction('👍');
		await message1.waitForNoReaction('👍');

		// Reacting with the same emoji again removes the reaction.
		await message2.reactWith('👍');
		await message2.waitForReaction('👍');
		await message1.waitForReaction('👍');
		await message2.reactWith('👍');
		await message2.waitForNoReaction('👍');
		await message1.waitForNoReaction('👍');
	});

	it('adds a reaction in a group chat', async () => {
		await agent1.directChatPage.back.click();
		await agent2.directChatPage.back.click();
		await agent1.homePage.ready();
		await agent2.homePage.ready();

		await createGroup(agent1, 'mygroup', 'Bob');

		// The group arrives over p2p sync, which can be slow on real devices.
		await agent2.homePage.chatListItem('mygroup').waitForExist({
			timeout: SYNC_TIMEOUT,
		});
		await agent2.homePage.chatListItem('mygroup').click();
		await agent2.groupChatPage.ready();

		await agent2.groupChatPage.composer.sendMessage('React in group');
		const message2 =
			await agent2.groupChatPage.messages.waitForMessage('React in group');
		const message1 =
			await agent1.groupChatPage.messages.waitForMessage('React in group');

		await message1.reactWith('❤️');
		await message1.waitForReaction('❤️');
		await message2.waitForReaction('❤️');

		// The who-reacted sheet resolves group members' profiles.
		await message2.openReactionsSheet('❤️');
		await agent2.waitUntil(() => message2.reactionsSheetShowsReactor('Alice'));
		await message2.closeReactionsSheet();

		// Reacting with the same emoji again removes it.
		await message1.reactWith('❤️');
		await message1.waitForNoReaction('❤️');
		await message2.waitForNoReaction('❤️');
	});
});
