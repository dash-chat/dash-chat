import { navigateToAddContact } from '../helpers/flows/exchange-contacts';
import { type Agent, setupAgents } from '../setup/setup-agents';

describe('report contact', () => {
	let agent1: Agent;
	let agent2: Agent;

	/** Confirm the open report dialog and land back on the chat list. */
	async function confirmReport() {
		await agent1.chatSettingsPage.reportConfirm.waitForClickable();
		await agent1.chatSettingsPage.reportConfirm.click();
		await agent1.toast.expectMessage(
			await agent1.tr('contactReportedToast', { name: 'Bob Test' }),
		);
		await agent1.homePage.ready();
	}

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await agent1.createProfilePage.createProfile('Alice', 'Test');
		await agent2.createProfilePage.createProfile('Bob', 'Test');

		// One-way add, so Alice lands on a chat showing Bob's contact request.
		await navigateToAddContact(agent1);
		const aliceLink = await agent1.addContactPage.getAddContactLink();
		await agent1.addContactPage.back.click();
		await agent1.newMessagePage.back.click();
		await agent1.homePage.ready();

		await navigateToAddContact(agent2);
		await agent2.addContactPage.enterAddContactLink(aliceLink);
		await agent2.directChatPage.ready();

		await agent1.homePage.chatList.waitForExist();
		await agent1.homePage.openChat('Bob Test');
		await agent1.directChatPage.acceptButton.waitForExist();
	});

	it('reports from the contact request banner and returns to the chat list', async () => {
		await agent1.directChatPage.reportButton.click();
		await confirmReport();

		await agent1.homePage.openChat('Bob Test');
		await agent1.directChatPage.reportMessage.waitForDisplayed();
		await expect(agent1.directChatPage.reportMessage).toHaveText(
			await agent1.tr('youReportedThisContact'),
		);
	});

	it('keeps reporting available from chat settings and adds a bubble per report', async () => {
		await agent1.directChatPage.acceptContactRequest();

		await agent1.directChatPage.settingsLink.click();
		await agent1.chatSettingsPage.ready();
		await agent1.chatSettingsPage.reportButton.click();
		await confirmReport();

		await agent1.homePage.openChat('Bob Test');
		await agent1.waitUntil(
			async () => (await agent1.directChatPage.reportMessageCount()) === 2,
		);
	});

	it('reports from the new-message contact menu', async () => {
		await agent1.directChatPage.back.click();
		await agent1.homePage.ready();
		await agent1.homePage.newMessageButton.click();
		await agent1.newMessagePage.ready();

		await agent1.newMessagePage.openContactMenu('Bob');
		await agent1.newMessagePage.contactActionsMenu.report.click();
		await agent1.newMessagePage.contactActionsMenu.reportConfirm.waitForClickable();
		await agent1.newMessagePage.contactActionsMenu.reportConfirm.click();
		await agent1.toast.expectMessage(
			await agent1.tr('contactReportedToast', { name: 'Bob Test' }),
		);

		await agent1.newMessagePage.back.click();
		await agent1.homePage.openChat('Bob Test');
		await agent1.waitUntil(
			async () => (await agent1.directChatPage.reportMessageCount()) === 3,
		);
	});
});
