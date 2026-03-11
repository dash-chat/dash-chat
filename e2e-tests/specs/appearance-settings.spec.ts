/**
 * Appearance settings E2E test.
 *
 * Tests the language selector and theme selector on the appearance page.
 * Only needs one agent — uses agent1.
 */

import {
	waitForTestUtils,
	createProfile,
	gotoAppearance,
	selectLanguage,
	selectTheme,
} from '../helpers/setup-agents';

describe('Appearance settings', () => {
	let agent: WebdriverIO.Browser;

	before(async () => {
		agent = browser.getInstance('agent1');
		await waitForTestUtils(agent);
		await createProfile(agent, 'Settings', 'Test');
		await gotoAppearance(agent);
	});

	it('shows default language as English', async () => {
		const text = await agent.execute(
			() => document.querySelector('[data-testid="appearance-language"]')?.textContent,
		);
		expect(text).toContain('English');
	});

	it('language dialog shows all options', async () => {
		await agent.execute(() => {
			(document.querySelector('[data-testid="appearance-language"]') as HTMLElement)?.click();
		});

		await agent.waitUntil(
			async () => agent.execute(
				() => !!document.querySelector('[data-testid="appearance-lang-es"]'),
			),
			{ timeout: 5_000, timeoutMsg: 'Language dialog did not open' },
		);

		const hasAllOptions = await agent.execute(() => {
			return (
				!!document.querySelector('[data-testid="appearance-lang-en"]') &&
				!!document.querySelector('[data-testid="appearance-lang-es"]') &&
				!!document.querySelector('[data-testid="appearance-lang-de-de"]') &&
				!!document.querySelector('[data-testid="appearance-lang-fa-ir"]')
			);
		});
		expect(hasAllOptions).toBe(true);

		// Close dialog by navigating away and back
		await gotoAppearance(agent);
	});

	it('changes language to Español', async () => {
		await selectLanguage(agent, 'es');

		await agent.waitUntil(
			async () => {
				const text = await agent.execute(
					() => document.querySelector('[data-testid="appearance-language"]')?.textContent,
				);
				return text?.includes('Español') ?? false;
			},
			{ timeout: 5_000, timeoutMsg: 'Language did not change to Español' },
		);
	});

	it('changes language back to English', async () => {
		await selectLanguage(agent, 'en');

		await agent.waitUntil(
			async () => {
				const text = await agent.execute(
					() => document.querySelector('[data-testid="appearance-language"]')?.textContent,
				);
				return text?.includes('English') ?? false;
			},
			{ timeout: 5_000, timeoutMsg: 'Language did not change back to English' },
		);
	});

	it('selecting Farsi sets document to RTL', async () => {
		await selectLanguage(agent, 'fa-ir');

		await agent.waitUntil(
			async () => agent.execute(
				() => document.documentElement.dir === 'rtl',
			),
			{ timeout: 5_000, timeoutMsg: 'Document did not switch to RTL for Farsi' },
		);

		// Change back to English
		await selectLanguage(agent, 'en');

		await agent.waitUntil(
			async () => agent.execute(
				() => document.documentElement.dir !== 'rtl',
			),
			{ timeout: 5_000, timeoutMsg: 'Document did not switch back to LTR' },
		);
	});

	it('changes theme to dark mode', async () => {
		await selectTheme(agent, 'dark');

		await agent.waitUntil(
			async () => agent.execute(
				() => document.documentElement.classList.contains('dark'),
			),
			{ timeout: 5_000, timeoutMsg: 'Dark mode class not applied' },
		);
	});

	it('changes theme to light mode', async () => {
		await selectTheme(agent, 'light');

		await agent.waitUntil(
			async () => agent.execute(
				() => !document.documentElement.classList.contains('dark'),
			),
			{ timeout: 5_000, timeoutMsg: 'Light mode class not applied' },
		);
	});

	it('changes theme back to system default', async () => {
		await selectTheme(agent, 'system');

		await agent.waitUntil(
			async () => agent.execute(
				() => !document.documentElement.classList.contains('dark'),
			),
			{ timeout: 5_000, timeoutMsg: 'System default did not remove dark class' },
		);
	});
});
