/**
 * UpdaterDialog E2E test.
 *
 * Uses window.__test.simulateUpdate() to trigger dialog states and verifies
 * the correct UI is rendered for each state (downloading, ready, error).
 *
 * Konsta Dialog keeps elements in the DOM at all times, using opacity: 0
 * and pointer-events: none when closed. The helper isDialogVisible() checks
 * computed opacity to determine open/closed state.
 *
 * Only needs one agent — uses agent1.
 */

import { waitForTestUtils, createProfile } from '../helpers/setup-agents';

/** Check if a data-testid element is visible (opacity > 0). */
function isVisible(selector: string): boolean {
	const el = document.querySelector(selector);
	if (!el) return false;
	return getComputedStyle(el).opacity !== '0';
}

describe('UpdaterDialog', () => {
	let agent: WebdriverIO.Browser;

	before(async () => {
		agent = browser.getInstance('agent1');
		await waitForTestUtils(agent);
		await createProfile(agent, 'Updater', 'Test');
	});

	afterEach(async () => {
		// Dismiss any open dialog and wait for the opacity transition to complete.
		await agent.execute(() => window.__test.simulateUpdate('idle'));
		await agent.waitUntil(
			async () => {
				const anyVisible = await agent.execute(() => {
					for (const id of ['updater-downloading', 'updater-ready', 'updater-error']) {
						const el = document.querySelector(`[data-testid="${id}"]`);
						if (el && getComputedStyle(el).opacity !== '0') return true;
					}
					return false;
				});
				return !anyVisible;
			},
			{ timeout: 5_000, timeoutMsg: 'Updater dialog not dismissed after setting idle' },
		);
	});

	it('shows downloading dialog with progress bar', async () => {
		await agent.execute(() => window.__test.simulateUpdate('downloading'));

		await agent.waitUntil(
			async () => agent.execute(isVisible, '[data-testid="updater-downloading"]'),
			{ timeout: 5_000, timeoutMsg: 'Downloading dialog not visible' },
		);

		// Verify the percentage text is present (progress = contentLength → 100%)
		const hasPercent = await agent.execute(
			() => document.body.innerText.includes('100%'),
		);
		expect(hasPercent).toBe(true);
	});

	it('shows ready dialog with restart and later buttons', async () => {
		await agent.execute(() => window.__test.simulateUpdate('ready'));

		await agent.waitUntil(
			async () => agent.execute(isVisible, '[data-testid="updater-ready"]'),
			{ timeout: 5_000, timeoutMsg: 'Ready dialog not visible' },
		);

		const hasLater = await agent.execute(
			() => document.querySelector('[data-testid="updater-later-btn"]') !== null,
		);
		expect(hasLater).toBe(true);

		const hasRestart = await agent.execute(
			() => document.querySelector('[data-testid="updater-restart-btn"]') !== null,
		);
		expect(hasRestart).toBe(true);
	});

	it('shows error dialog with OK button', async () => {
		await agent.execute(() => window.__test.simulateUpdate('error'));

		await agent.waitUntil(
			async () => agent.execute(isVisible, '[data-testid="updater-error"]'),
			{ timeout: 5_000, timeoutMsg: 'Error dialog not visible' },
		);

		const hasOk = await agent.execute(
			() => document.querySelector('[data-testid="updater-ok-btn"]') !== null,
		);
		expect(hasOk).toBe(true);
	});

	it('dismisses error dialog when OK is clicked', async () => {
		await agent.execute(() => window.__test.simulateUpdate('error'));

		await agent.waitUntil(
			async () => agent.execute(isVisible, '[data-testid="updater-error"]'),
			{ timeout: 5_000, timeoutMsg: 'Error dialog not visible' },
		);

		// Click the OK button
		await agent.execute(() => {
			(document.querySelector('[data-testid="updater-ok-btn"]') as HTMLElement)?.click();
		});

		// Verify dialog is hidden (opacity transitions to 0)
		await agent.waitUntil(
			async () => {
				const visible = await agent.execute(isVisible, '[data-testid="updater-error"]');
				return !visible;
			},
			{ timeout: 5_000, timeoutMsg: 'Error dialog was not dismissed' },
		);
	});

	it('dismisses ready dialog when Later is clicked', async () => {
		await agent.execute(() => window.__test.simulateUpdate('ready'));

		await agent.waitUntil(
			async () => agent.execute(isVisible, '[data-testid="updater-ready"]'),
			{ timeout: 5_000, timeoutMsg: 'Ready dialog not visible' },
		);

		// Click the Later button
		await agent.execute(() => {
			(document.querySelector('[data-testid="updater-later-btn"]') as HTMLElement)?.click();
		});

		// Verify dialog is hidden
		await agent.waitUntil(
			async () => {
				const visible = await agent.execute(isVisible, '[data-testid="updater-ready"]');
				return !visible;
			},
			{ timeout: 5_000, timeoutMsg: 'Ready dialog was not dismissed' },
		);
	});
});
