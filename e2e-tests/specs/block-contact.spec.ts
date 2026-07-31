import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgent } from '../setup/setup-agents';

describe('block contact', () => {
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

	it('blocks from chat settings and shows the indicators', async () => {
		await agent1.directChatPage.settingsLink.click();
		await agent1.chatSettingsPage.ready();

		await agent1.chatSettingsPage.blockToggle.click();
		await agent1.chatSettingsPage.blockConfirm.waitForDisplayed();
		await agent1.chatSettingsPage.blockConfirm.click();

		await agent1.chatSettingsPage.back.click();
		await agent1.directChatPage.ready();
		await agent1.directChatPage.blockedBanner.waitForDisplayed();
		await agent1.directChatPage.blockedNameIcon.waitForDisplayed();

		await agent1.directChatPage.back.click();
		await agent1.homePage.ready();
		await agent1.homePage.blockedRowIcon.waitForDisplayed();
	});

	it('unblocks from the blocked banner', async () => {
		await agent1.homePage.chatRow.click();
		await agent1.directChatPage.ready();
		await agent1.directChatPage.unblockButton.click();
		await agent1.directChatPage.unblockConfirm.waitForDisplayed();
		await agent1.directChatPage.unblockConfirm.click();
		await agent1.directChatPage.blockedBanner.waitForDisplayed({
			reverse: true,
		});
	});
});
