import { createProfiles } from '../helpers/flows/create-profiles';
import { exchangeContacts } from '../helpers/flows/exchange-contacts';
import { MAILBOX_HUNG_MS } from '../helpers/timeouts';
import { type Agent, setupAgents } from '../setup/setup-agents';

describe('Connection status bar', function () {
	this.timeout(300_000);

	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		[agent1, agent2] = await setupAgents(this, [
			{ platform: 'any' },
			{ platform: 'any' },
		]);
		await createProfiles({ Alice: agent1, Bob: agent2 });
		await exchangeContacts([agent1, agent2]);
	});

	it('comes down in a chat with one icon per channel, and goes back up', async () => {
		await agent1.directChatPage.connectionStatusIndicator.waitForStatus(
			'cloud',
			MAILBOX_HUNG_MS,
			'the chip did not read cloud with the mailbox up',
		);
		const statusBar = agent1.directChatPage.connectionStatusBar;

		await statusBar.openButton.click();
		await statusBar.bar.waitForDisplayed();
		await expect(statusBar.internetChannel).toHaveAttribute(
			'data-connected',
			'true',
		);
		await expect(statusBar.hubChannel).toHaveAttribute(
			'data-connected',
			'false',
		);
		await expect(statusBar.p2pChannel).toExist();
		await expect(statusBar.bar).toHaveText(
			await agent1.tr('connectionStatusCloudTitle'),
		);

		await statusBar.collapseButton.click();
		await statusBar.bar.waitForExist({ reverse: true });
	});

	it('opens the details dialog from the bar', async () => {
		await agent1.directChatPage.connectionStatusBar.expectDialog(
			await agent1.tr('connectionStatusCloudTitle'),
			await agent1.tr('connectionStatusCloudDescription'),
		);
	});

	it('comes down on home without a peer-to-peer channel', async () => {
		await agent1.directChatPage.back.click();
		await agent1.homePage.ready();
		const statusBar = agent1.homePage.connectionStatusBar;

		await statusBar.openButton.click();
		await statusBar.bar.waitForDisplayed();
		await expect(statusBar.p2pChannel).not.toExist();

		await statusBar.collapseButton.click();
		await statusBar.bar.waitForExist({ reverse: true });
	});
});
