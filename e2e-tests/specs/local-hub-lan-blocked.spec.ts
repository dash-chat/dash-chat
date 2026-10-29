/**
 * The agent's Local Network access stays pending behind the macOS alert, and
 * the tests share that state in order: the second allows what the first left
 * pending.
 */
import { createProfiles } from '../helpers/flows/create-profiles';
import {
	allowLocalNetworkAlerts,
	canAnswerSystemAlerts,
} from '../setup/platforms/macos';
import {
	type Agent,
	setupAgentWithoutLocalNetworkAccess,
} from '../setup/setup-agents';

const PROBE_TIMEOUT_MS = 15_000;

describe('Local hub LAN warning', function () {
	let agent: Agent;
	let alertsMayBeUp = false;

	before(async function () {
		if (process.platform !== 'darwin' || !canAnswerSystemAlerts()) {
			this.skip();
		}
		alertsMayBeUp = true;
		agent = await setupAgentWithoutLocalNetworkAccess();
		await createProfiles({ Alice: agent });
	});

	// A system alert left up by a failed test would sit over whatever runs next.
	after(() => {
		if (alertsMayBeUp) {
			allowLocalNetworkAlerts();
		}
	});

	it('warns that phones cannot find the hub while macOS keeps it off the LAN', async () => {
		await agent.homePage.settingsLink.click();
		await agent.settingsPage.ready();
		await agent.settingsPage.offlineLink.click();
		await agent.offlinePage.ready();
		await agent.offlinePage.setLocalMailboxEnabled(true);

		await agent.offlinePage.localMailboxLanBlocked.waitForDisplayed({
			timeout: PROBE_TIMEOUT_MS,
			timeoutMsg: 'no LAN warning while local network access is pending',
		});
		await agent.offlinePage.openLocalNetworkSettings.waitForDisplayed();

		await agent.offlinePage.back.click();
		await agent.settingsPage.ready();
		await agent.settingsPage.offlineHubWarning.waitForDisplayed();
		await agent.settingsPage.back.click();
		await agent.homePage.ready();
		await agent.homePage.settingsHubWarning.waitForDisplayed();
	});

	it('clears the warning once the user allows local network access', async () => {
		allowLocalNetworkAlerts();

		await agent.homePage.settingsHubWarning.waitForDisplayed({
			reverse: true,
			timeout: PROBE_TIMEOUT_MS,
			timeoutMsg: 'the LAN warning stayed after allowing local network access',
		});
		await agent.homePage.settingsLink.click();
		await agent.settingsPage.ready();
		await agent.settingsPage.offlineLink.click();
		await agent.offlinePage.ready();
		await agent.offlinePage.localMailboxLanBlocked.waitForDisplayed({
			reverse: true,
		});
	});
});
