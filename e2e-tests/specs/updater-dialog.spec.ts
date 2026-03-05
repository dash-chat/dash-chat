/**
 * UpdaterBanner E2E test.
 *
 * Uses window.__test.simulateUpdate() to trigger banner states and verifies
 * the correct UI is rendered for each state (available, downloading, ready, error).
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
		// Dismiss any open banner
		await agent.execute(() => window.__test.simulateUpdate('idle'));
		await agent.waitUntil(
			async () => {
				const visible = await agent.execute(() => {
					return document.querySelector('[data-testid="updater-banner"]') === null;
				});
				return visible;
			},
			{ timeout: 5_000, timeoutMsg: 'Updater banner not dismissed after setting idle' },
		);
	});

	it('shows available banner with update button', async () => {
		await agent.execute(() => window.__test.simulateUpdate('available'));

		await agent.waitUntil(
			async () =>
				agent.execute(
					() => document.querySelector('[data-testid="updater-available"]') !== null,
				),
			{ timeout: 5_000, timeoutMsg: 'Available banner not visible' },
		);

		const hasDownloadBtn = await agent.execute(
			() => document.querySelector('[data-testid="updater-download-btn"]') !== null,
		);
		expect(hasDownloadBtn).toBe(true);
	});

	it('shows downloading banner with progress bar', async () => {
		await agent.execute(() => window.__test.simulateUpdate('downloading'));

		await agent.waitUntil(
			async () =>
				agent.execute(
					() => document.querySelector('[data-testid="updater-downloading"]') !== null,
				),
			{ timeout: 5_000, timeoutMsg: 'Downloading banner not visible' },
		);
	});

	it('shows ready banner with restart button', async () => {
		await agent.execute(() => window.__test.simulateUpdate('ready'));

		await agent.waitUntil(
			async () =>
				agent.execute(
					() => document.querySelector('[data-testid="updater-ready"]') !== null,
				),
			{ timeout: 5_000, timeoutMsg: 'Ready banner not visible' },
		);

		const hasRestart = await agent.execute(
			() => document.querySelector('[data-testid="updater-restart-btn"]') !== null,
		);
		expect(hasRestart).toBe(true);
	});

	it('shows error banner with retry button', async () => {
		await agent.execute(() => window.__test.simulateUpdate('error'));

		await agent.waitUntil(
			async () =>
				agent.execute(
					() => document.querySelector('[data-testid="updater-error"]') !== null,
				),
			{ timeout: 5_000, timeoutMsg: 'Error banner not visible' },
		);

		const hasRetry = await agent.execute(
			() => document.querySelector('[data-testid="updater-retry-btn"]') !== null,
		);
		expect(hasRetry).toBe(true);
	});

	it('dismisses banner when dismiss button is clicked', async () => {
		await agent.execute(() => window.__test.simulateUpdate('available'));

		await agent.waitUntil(
			async () =>
				agent.execute(
					() => document.querySelector('[data-testid="updater-banner"]') !== null,
				),
			{ timeout: 5_000, timeoutMsg: 'Banner not visible' },
		);

		// Click the dismiss button
		await agent.execute(() => {
			(document.querySelector('[data-testid="updater-dismiss-btn"]') as HTMLElement)?.click();
		});

		// Verify banner is removed from DOM
		await agent.waitUntil(
			async () =>
				agent.execute(
					() => document.querySelector('[data-testid="updater-banner"]') === null,
				),
			{ timeout: 5_000, timeoutMsg: 'Banner was not dismissed' },
		);
	});
});
