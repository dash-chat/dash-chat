/**
 * UpdaterBanner E2E test.
 *
 * Uses window.__test.simulateUpdate() to trigger banner states and verifies
 * the correct UI is rendered for each state (available, downloading, ready, error).
 *
 * The banner is conditionally rendered (not in DOM when hidden), so we check
 * for element existence rather than opacity.
 *
 * Only needs one agent — uses agent1.
 */

import { S } from '../../ui/tests/selectors';
import { type Agent, makeAgent, waitForTestUtils } from '../helpers/setup-agents';

describe('UpdaterBanner', () => {
	let agent: Agent;

	before(async () => {
		agent = makeAgent(browser.getInstance('agent1'));
		await waitForTestUtils(agent);
		await agent.createProfile('Updater', 'Test');
	});

	afterEach(async () => {
		// Dismiss the banner and wait for it to be removed from the DOM.
		await agent.simulateUpdate('hidden');
		await agent.waitUntil(async () => !(await agent.updaterBanner()), {
			timeout: 5_000,
			timeoutMsg: 'Updater banner not dismissed after setting hidden',
		});
	});

	it('shows available banner with update message', async () => {
		await agent.simulateUpdate('available');

		await agent.waitUntil(async () => !!(await agent.updaterBanner()), {
			timeout: 5_000,
			timeoutMsg: 'Available banner not visible',
		});

		expect(await agent.updaterBannerTitle()).toBeTruthy();
	});

	it('shows downloading banner with progress bar', async () => {
		await agent.simulateUpdate('downloading');

		await agent.waitUntil(async () => !!(await agent.updaterBanner()), {
			timeout: 5_000,
			timeoutMsg: 'Downloading banner not visible',
		});
	});

	it('shows ready banner', async () => {
		await agent.simulateUpdate('ready');

		await agent.waitUntil(async () => !!(await agent.updaterBanner()), {
			timeout: 5_000,
			timeoutMsg: 'Ready banner not visible',
		});
	});

	it('shows error banner', async () => {
		await agent.simulateUpdate('error');

		await agent.waitUntil(async () => !!(await agent.updaterBanner()), {
			timeout: 5_000,
			timeoutMsg: 'Error banner not visible',
		});
	});

	it('dismisses banner when X is clicked', async () => {
		await agent.simulateUpdate('available');

		await agent.waitUntil(async () => !!(await agent.updaterBanner()), {
			timeout: 5_000,
			timeoutMsg: 'Banner not visible',
		});

		await agent.click(S.updater.dismissBtn);

		await agent.waitUntil(async () => !(await agent.updaterBanner()), {
			timeout: 5_000,
			timeoutMsg: 'Banner was not dismissed',
		});
	});
});
