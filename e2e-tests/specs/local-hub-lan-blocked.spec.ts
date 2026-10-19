/**
 * The local hub tells its user when phones can't find it, on macOS, where
 * Local Network privacy keeps an app's mDNS off the LAN until the user allows
 * it. The desktop agent runs as an app macOS has never seen, so its access
 * stays undetermined behind the system alert while the hub starts: the hub
 * announces to nobody, and Settings › Offline says so, with a badge on the
 * way there from the home page. Allowing it in the alert clears them.
 *
 * macOS only, and skips itself unless the terminal running the harness has
 * Accessibility access, which answering the alert needs. Every run leaves a
 * row in System Settings › Local Network, which macOS offers no way to remove.
 * macOS also remembers the answer by the binary's UUID, so a run on the very
 * binary an earlier run allowed starts allowed and fails.
 */
import { createProfiles } from '../helpers/flows/create-profiles';
import {
	answerLocalNetworkAlert,
	canAnswerSystemAlerts,
} from '../setup/platforms/macos';
import {
	type Agent,
	setupAgentWithoutLocalNetworkAccess,
} from '../setup/setup-agents';

/** The node probes the LAN every 3s. */
const PROBE_MS = 15_000;

describe('Local hub LAN warning', function () {
	let agent: Agent;

	before(async function () {
		if (process.platform !== 'darwin' || !canAnswerSystemAlerts()) {
			this.skip();
		}
		agent = await setupAgentWithoutLocalNetworkAccess();
		await createProfiles({ Alice: agent });
	});

	it('warns that phones cannot find the hub while macOS keeps it off the LAN', async () => {
		await agent.homePage.settingsLink.click();
		await agent.settingsPage.ready();
		await agent.settingsPage.offlineLink.click();
		await agent.offlinePage.ready();
		await agent.offlinePage.setLocalMailboxEnabled(true);

		await agent.offlinePage.localMailboxLanBlocked.waitForDisplayed({
			timeout: PROBE_MS,
			timeoutMsg: 'no LAN warning while local network access is pending',
		});

		await agent.offlinePage.back.click();
		await agent.settingsPage.ready();
		await agent.settingsPage.offlineHubWarning.waitForDisplayed();
		await agent.settingsPage.back.click();
		await agent.homePage.ready();
		await agent.homePage.settingsHubWarning.waitForDisplayed();
	});

	it('clears the warning once the user allows local network access', async () => {
		answerLocalNetworkAlert('Allow');

		await agent.homePage.settingsHubWarning.waitForDisplayed({
			reverse: true,
			timeout: PROBE_MS,
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
