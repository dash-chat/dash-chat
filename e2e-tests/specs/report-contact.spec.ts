import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import {
	isRemoteMailbox,
	killMailbox,
	restartMailbox,
} from '../setup/mailbox-control';
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
