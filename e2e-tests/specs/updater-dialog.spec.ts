/**
 * UpdaterBanner E2E test.
 *
 * Uses window.__test.simulateUpdate() to trigger banner states and verifies
 * the correct UI is rendered for each state (available, downloading, ready, error).
 *
 * Only needs one agent — uses agent1.
 */

import { waitForTestUtils, createProfile } from '../helpers/setup-agents';

/** Check if a data-testid element exists in the DOM. */
function isPresent(selector: string): boolean {
	return document.querySelector(selector) !== null;
}

describe('UpdaterBanner', () => {
	let agent: WebdriverIO.Browser;

	before(async () => {
		agent = browser.getInstance('agent1');
		await waitForTestUtils(agent);
		await createProfile(agent, 'Updater', 'Test');
	});

	afterEach(async () => {
		// Dismiss the banner
		await agent.execute(() => window.__test.simulateUpdate('idle'));
		await agent.waitUntil(
			async () => {
				const visible = await agent.execute(
					isPresent,
					'[data-testid="updater-banner"]',
				);
				return !visible;
			},
			{ timeout: 5_000, timeoutMsg: 'Updater banner not dismissed after setting idle' },
		);
	});

	it('shows available banner with download button', async () => {
		await agent.execute(() => window.__test.simulateUpdate('available'));

		await agent.waitUntil(
			async () => agent.execute(isPresent, '[data-testid="updater-available"]'),
			{ timeout: 5_000, timeoutMsg: 'Available banner not visible' },
		);

		const hasDownload = await agent.execute(
			() => document.querySelector('[data-testid="updater-download-btn"]') !== null,
		);
		expect(hasDownload).toBe(true);

		const hasDismiss = await agent.execute(
			() => document.querySelector('[data-testid="updater-dismiss-btn"]') !== null,
		);
		expect(hasDismiss).toBe(true);
	});

	it('shows downloading banner with progress bar', async () => {
		await agent.execute(() => window.__test.simulateUpdate('downloading'));

		await agent.waitUntil(
			async () => agent.execute(isPresent, '[data-testid="updater-downloading"]'),
			{ timeout: 5_000, timeoutMsg: 'Downloading banner not visible' },
		);

		const hasPercent = await agent.execute(
			() => document.body.innerText.includes('100%'),
		);
		expect(hasPercent).toBe(true);
	});

	it('shows ready banner with restart and dismiss buttons', async () => {
		await agent.execute(() => window.__test.simulateUpdate('ready'));

		await agent.waitUntil(
			async () => agent.execute(isPresent, '[data-testid="updater-ready"]'),
			{ timeout: 5_000, timeoutMsg: 'Ready banner not visible' },
		);

		const hasRestart = await agent.execute(
			() => document.querySelector('[data-testid="updater-restart-btn"]') !== null,
		);
		expect(hasRestart).toBe(true);

		const hasLater = await agent.execute(
			() => document.querySelector('[data-testid="updater-later-btn"]') !== null,
		);
		expect(hasLater).toBe(true);
	});

	it('shows error banner with retry and dismiss buttons', async () => {
		await agent.execute(() => window.__test.simulateUpdate('error'));

		await agent.waitUntil(
			async () => agent.execute(isPresent, '[data-testid="updater-error"]'),
			{ timeout: 5_000, timeoutMsg: 'Error banner not visible' },
		);

		const hasRetry = await agent.execute(
			() => document.querySelector('[data-testid="updater-retry-btn"]') !== null,
		);
		expect(hasRetry).toBe(true);

		const hasOk = await agent.execute(
			() => document.querySelector('[data-testid="updater-ok-btn"]') !== null,
		);
		expect(hasOk).toBe(true);
	});

	it('dismisses error banner when dismiss is clicked', async () => {
		await agent.execute(() => window.__test.simulateUpdate('error'));

		await agent.waitUntil(
			async () => agent.execute(isPresent, '[data-testid="updater-error"]'),
			{ timeout: 5_000, timeoutMsg: 'Error banner not visible' },
		);

		await agent.execute(() => {
			(document.querySelector('[data-testid="updater-ok-btn"]') as HTMLElement)?.click();
		});

		await agent.waitUntil(
			async () => {
				const visible = await agent.execute(isPresent, '[data-testid="updater-banner"]');
				return !visible;
			},
			{ timeout: 5_000, timeoutMsg: 'Error banner was not dismissed' },
		);
	});

	it('dismisses ready banner when dismiss is clicked', async () => {
		await agent.execute(() => window.__test.simulateUpdate('ready'));

		await agent.waitUntil(
			async () => agent.execute(isPresent, '[data-testid="updater-ready"]'),
			{ timeout: 5_000, timeoutMsg: 'Ready banner not visible' },
		);

		await agent.execute(() => {
			(document.querySelector('[data-testid="updater-later-btn"]') as HTMLElement)?.click();
		});

		await agent.waitUntil(
			async () => {
				const visible = await agent.execute(isPresent, '[data-testid="updater-banner"]');
				return !visible;
			},
			{ timeout: 5_000, timeoutMsg: 'Ready banner was not dismissed' },
		);
	});
});
