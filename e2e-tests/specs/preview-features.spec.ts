import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgents } from '../setup/setup-agents';

// Replies are a preview feature: with the flag off there is no way to author
// one, and the Help settings toggle is what turns them on.
describe('Preview features', () => {
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
		await agent1.directChatPage.composer.sendMessage('Are replies on?');
	});

	it('offers no way to reply while preview features are off', async () => {
		const message =
			await agent2.directChatPage.messages.waitForMessage('Are replies on?');
		expect(await message.hoverReplyButton.isExisting()).toBe(false);

		await message.openActions();
		expect(await message.replyAction.isExisting()).toBe(false);
		await message.closeActions();
	});

	it('turns replies on from the Help settings toggle', async () => {
		await agent2.directChatPage.back.click();
		await agent2.homePage.settingsLink.click();
		await agent2.settingsPage.ready();
		await agent2.settingsPage.helpLink.click();
		await agent2.helpPage.ready();

		expect(await agent2.helpPage.previewFeaturesEnabled()).toBe(false);
		await agent2.helpPage.togglePreviewFeatures();

		await agent2.helpPage.back.click();
		await agent2.settingsPage.ready();
		await agent2.settingsPage.back.click();
		await agent2.homePage.ready();
		await agent2.homePage.openChat('Alice');
		await agent2.directChatPage.ready();

		const message =
			await agent2.directChatPage.messages.waitForMessage('Are replies on?');
		await message.reply('They are now');

		const reply =
			await agent1.directChatPage.messages.waitForMessage('They are now');
		await reply.waitForReplyQuote('Are replies on?');
	});
});
