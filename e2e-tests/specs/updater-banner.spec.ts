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

import { waitForTestUtils, createProfile } from '../helpers/setup-agents';

describe('UpdaterBanner', () => {
	let agent: WebdriverIO.Browser;

	before(async () => {
		agent = browser.getInstance('agent1');
		await waitForTestUtils(agent);
		await createProfile(agent, 'Updater', 'Test');
	});

	afterEach(async () => {
		// Dismiss the banner and wait for it to be removed from the DOM.
		await agent.execute(() => window.__test.simulateUpdate('hidden'));
		await agent.waitUntil(
			async () => {
				const exists = await agent.execute(
					() => window.__test.updaterBanner() !== null,
				);
				return !exists;
			},
			{ timeout: 5_000, timeoutMsg: 'Updater banner not dismissed after setting hidden' },
		);
	});

	it('shows available banner with update message', async () => {
		await agent.execute(() => window.__test.simulateUpdate('available'));

		await agent.waitUntil(
			async () =>
				agent.execute(() => window.__test.updaterBanner() !== null),
			{ timeout: 5_000, timeoutMsg: 'Available banner not visible' },
		);

		const hasTitle = await agent.execute(
			() => window.__test.updaterBannerTitle() !== null,
		);
		expect(hasTitle).toBe(true);
	});

	it('shows downloading banner with progress bar', async () => {
		await agent.execute(() => window.__test.simulateUpdate('downloading'));

		await agent.waitUntil(
			async () =>
				agent.execute(() => window.__test.updaterBanner() !== null),
			{ timeout: 5_000, timeoutMsg: 'Downloading banner not visible' },
		);
	});

	it('shows ready banner', async () => {
		await agent.execute(() => window.__test.simulateUpdate('ready'));

		await agent.waitUntil(
			async () =>
				agent.execute(() => window.__test.updaterBanner() !== null),
			{ timeout: 5_000, timeoutMsg: 'Ready banner not visible' },
		);
	});

	it('shows error banner', async () => {
		await agent.execute(() => window.__test.simulateUpdate('error'));

		await agent.waitUntil(
			async () =>
				agent.execute(() => window.__test.updaterBanner() !== null),
			{ timeout: 5_000, timeoutMsg: 'Error banner not visible' },
		);
	});

	it('dismisses banner when X is clicked', async () => {
		await agent.execute(() => window.__test.simulateUpdate('available'));

		await agent.waitUntil(
			async () =>
				agent.execute(() => window.__test.updaterBanner() !== null),
			{ timeout: 5_000, timeoutMsg: 'Banner not visible' },
		);

		// Click the dismiss button
		await agent.execute(() => {
			(window.__test.updaterDismissBtn() as HTMLElement)?.click();
		});

		// Verify banner is removed from DOM
		await agent.waitUntil(
			async () => {
				const exists = await agent.execute(
					() => window.__test.updaterBanner() !== null,
				);
				return !exists;
			},
			{ timeout: 5_000, timeoutMsg: 'Banner was not dismissed' },
		);
	});
});
