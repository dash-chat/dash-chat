import { navigateToAddContact } from '../helpers/flows/exchange-contacts';
import {
	isRemoteMailbox,
	killMailbox,
	restartMailbox,
} from '../setup/mailbox-control';
import { type Agent, setupAgents } from '../setup/setup-agents';

describe('report contact', () => {
	let agent1: Agent;
	let agent2: Agent;
	let agent1Code: string;

	// One-way exchange first, so agent2 sits on the contact-request banner and we
	// can report from there before accepting completes the exchange.
	before(async function () {
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await agent1.createProfilePage.createProfile('Alice', 'Test');
		await agent2.createProfilePage.createProfile('Bob', 'Test');

		await navigateToAddContact(agent1);
		await navigateToAddContact(agent2);
		agent1Code = await agent1.addContactPage.getAddContactLink();
		const agent2Code = await agent2.addContactPage.getAddContactLink();
		await agent1.addContactPage.enterAddContactLink(agent2Code);

		await agent2.addContactPage.back.click();
		await agent2.newMessagePage.back.click();
		await agent2.homePage.ready();
		await agent2.waitUntil(async () => agent2.homePage.hasChatListItem('Alice'));
	});

	it('reports from the contact-request banner and shows the reported state', async () => {
		await agent2.homePage.openChat('Alice');
		await agent2.waitUntil(async () =>
			agent2.directChatPage.isContactRequestBannerVisible(),
		);

		await agent2.directChatPage.reportButton.click();
		await agent2.directChatPage.reportConfirm.waitForDisplayed();
		await agent2.directChatPage.reportConfirm.click();

		await agent2.waitUntil(
			async () =>
				(await agent2.directChatPage.reportButton.getText()).includes(
					'Reported',
				),
			{
				timeoutMsg:
					'banner report button never switched to the reported state',
			},
		);
	});

	it('leaves the reported banner button inert', async () => {
		await agent2.directChatPage.reportButton.click();
		expect(await agent2.directChatPage.reportConfirm.isDisplayed()).toBe(false);
	});

	it('completes the contact exchange', async () => {
		await agent2.directChatPage.back.click();
		await agent2.homePage.ready();
		await navigateToAddContact(agent2);
		await agent2.addContactPage.enterAddContactLink(agent1Code);
		await agent2.directChatPage.ready();
		await agent1.directChatPage.ready();
	});

	// Runs before the successful reports below: once a report lands, the contact
	// stays reported and the row is no longer actionable.
	it('warns to retry when no mailbox could be reached', async function () {
		if (isRemoteMailbox()) this.skip();

		killMailbox();
		try {
			await agent1.directChatPage.settingsLink.click();
			await agent1.chatSettingsPage.ready();

			await agent1.chatSettingsPage.reportItem.click();
			await agent1.chatSettingsPage.reportConfirm.waitForDisplayed();
			await agent1.chatSettingsPage.reportConfirm.click();

			await agent1.toast.expectMessageContaining(
				'No message server could be reached',
			);

			// Still reportable, so the user can retry once back online.
			expect(await agent1.chatSettingsPage.reportItem.getText()).not.toContain(
				'Reported',
			);
		} finally {
			await restartMailbox();
		}

		await agent1.chatSettingsPage.back.click();
		await agent1.directChatPage.ready();
	});

	it('reports from chat settings and shows the reported indicator', async () => {
		await agent1.directChatPage.settingsLink.click();
		await agent1.chatSettingsPage.ready();

		await agent1.chatSettingsPage.reportItem.waitForDisplayed();
		await agent1.chatSettingsPage.reportItem.click();
		await agent1.chatSettingsPage.reportConfirm.waitForDisplayed();
		await agent1.chatSettingsPage.reportConfirm.click();

		await agent1.waitUntil(
			async () =>
				(await agent1.chatSettingsPage.reportItem.getText()).includes(
					'Reported',
				),
			{ timeoutMsg: 'report item never switched to the reported state' },
		);
	});

	it('keeps the reported state after navigating away and back', async () => {
		await agent1.chatSettingsPage.back.click();
		await agent1.directChatPage.ready();
		await agent1.directChatPage.settingsLink.click();
		await agent1.chatSettingsPage.ready();

		await agent1.waitUntil(
			async () =>
				(await agent1.chatSettingsPage.reportItem.getText()).includes(
					'Reported',
				),
			{ timeoutMsg: 'reported state did not persist across navigation' },
		);
	});
});
