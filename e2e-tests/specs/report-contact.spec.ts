import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgents } from '../setup/setup-agents';

describe('report contact', () => {
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
		await agent1.directChatPage.peerName.waitForDisplayed();
	});

	async function reportFromChatSettings() {
		await agent1.directChatPage.settingsLink.click();
		await agent1.chatSettingsPage.ready();
		await agent1.chatSettingsPage.reportButton.click();
		await agent1.chatSettingsPage.reportConfirm.waitForClickable();
		await agent1.chatSettingsPage.reportConfirm.click();
		await agent1.toast.expectMessage(
			await agent1.tr('contactReportedToast', { name: 'Bob Test' }),
		);
		await agent1.chatSettingsPage.back.click();
		await agent1.directChatPage.ready();
	}

	it('shows a report bubble in the chat after reporting', async () => {
		await reportFromChatSettings();

		await agent1.directChatPage.reportMessage.waitForDisplayed();
		await expect(agent1.directChatPage.reportMessage).toHaveText(
			await agent1.tr('youReportedThisContact'),
		);
	});

	it('keeps reporting available and adds a bubble per report', async () => {
		await reportFromChatSettings();

		await agent1.waitUntil(
			async () => (await agent1.directChatPage.reportMessageCount()) === 2,
		);
	});
});
