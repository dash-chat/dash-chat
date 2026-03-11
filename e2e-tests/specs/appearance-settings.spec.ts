/**
 * Appearance settings E2E test.
 *
 * Tests the language selector and theme selector on the appearance page.
 * Only needs one agent — uses agent1.
 */

import { waitForTestUtils, createProfile } from '../helpers/setup-agents';

describe('Appearance settings', () => {
	let agent: WebdriverIO.Browser;

	before(async () => {
		agent = browser.getInstance('agent1');
		await waitForTestUtils(agent);
		await createProfile(agent, 'Settings', 'Test');

		// Navigate to appearance page
		const err = await agent.executeAsync((done: (r: string | null) => void) => {
			window.__test.goto('/settings/appearance').then(() => done(null), (e) => done(String(e)));
		});
		if (err) throw new Error(`Navigation failed: ${err}`);

		await agent.waitUntil(
			async () => agent.execute(
				() => !!document.querySelector('[data-testid="appearance-language"]'),
			),
			{ timeout: 10_000, timeoutMsg: 'Appearance page not loaded' },
		);
	});

	it('shows default language as English', async () => {
		const text = await agent.execute(
			() => document.querySelector('[data-testid="appearance-language"]')?.textContent,
		);
		expect(text).toContain('English');
	});

	it('changes language to Español via dialog', async () => {
		// Click language item to open dialog
		await agent.execute(() => {
			(document.querySelector('[data-testid="appearance-language"]') as HTMLElement)?.click();
		});

		// Wait for dialog option to appear
		await agent.waitUntil(
			async () => agent.execute(
				() => !!document.querySelector('[data-testid="appearance-lang-es"]'),
			),
			{ timeout: 5_000, timeoutMsg: 'Language dialog not opened' },
		);

		// Click the Español option
		await agent.execute(() => {
			(document.querySelector('[data-testid="appearance-lang-es"]') as HTMLElement)?.click();
		});

		// Wait for dialog to close and language to update
		await agent.waitUntil(
			async () => {
				const text = await agent.execute(
					() => document.querySelector('[data-testid="appearance-language"]')?.textContent,
				);
				return text?.includes('Español') ?? false;
			},
			{ timeout: 5_000, timeoutMsg: 'Language did not change to Español' },
		);

		// Navigate to settings and verify a translated string appears
		await agent.executeAsync((done: (r: string | null) => void) => {
			window.__test.goto('/settings').then(() => done(null), (e) => done(String(e)));
		});
		await agent.waitUntil(
			async () => agent.execute(
				() => !!document.querySelector('[data-testid="settings-profile-link"]'),
			),
			{ timeout: 5_000, timeoutMsg: 'Settings page not loaded' },
		);
		const profileText = await agent.execute(
			() => document.querySelector('[data-testid="settings-profile-link"]')?.textContent,
		);
		// "Profile" in Spanish is "Mi perfil"
		expect(profileText).toContain('Mi perfil');

		// Navigate back to appearance
		await agent.executeAsync((done: (r: string | null) => void) => {
			window.__test.goto('/settings/appearance').then(() => done(null), (e) => done(String(e)));
		});
		await agent.waitUntil(
			async () => agent.execute(
				() => !!document.querySelector('[data-testid="appearance-language"]'),
			),
			{ timeout: 5_000, timeoutMsg: 'Appearance page not loaded after nav back' },
		);
	});

	it('changes language back to English', async () => {
		// Click language item to open dialog
		await agent.execute(() => {
			(document.querySelector('[data-testid="appearance-language"]') as HTMLElement)?.click();
		});

		await agent.waitUntil(
			async () => agent.execute(
				() => !!document.querySelector('[data-testid="appearance-lang-en"]'),
			),
			{ timeout: 5_000, timeoutMsg: 'Language dialog not opened' },
		);

		await agent.execute(() => {
			(document.querySelector('[data-testid="appearance-lang-en"]') as HTMLElement)?.click();
		});

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

	it('changes theme to dark mode', async () => {
		// On desktop (wide screen), the theme uses a native <select>
		await agent.execute(() => {
			const select = document.querySelector('[data-testid="appearance-theme"] select') as HTMLSelectElement;
			if (select) {
				select.value = 'dark';
				select.dispatchEvent(new Event('change', { bubbles: true }));
			} else {
				// On mobile, click the theme item to open dialog
				(document.querySelector('[data-testid="appearance-theme"]') as HTMLElement)?.click();
			}
		});

		// If mobile, handle the dialog
		const hasDialog = await agent.execute(
			() => !!document.querySelector('[data-testid="appearance-theme-dark"]'),
		);
		if (hasDialog) {
			await agent.execute(() => {
				(document.querySelector('[data-testid="appearance-theme-dark"]') as HTMLElement)?.click();
			});
		}

		// Verify dark class is applied to <html>
		await agent.waitUntil(
			async () => agent.execute(
				() => document.documentElement.classList.contains('dark'),
			),
			{ timeout: 5_000, timeoutMsg: 'Dark mode class not applied' },
		);
	});

	it('changes theme to light mode', async () => {
		await agent.execute(() => {
			const select = document.querySelector('[data-testid="appearance-theme"] select') as HTMLSelectElement;
			if (select) {
				select.value = 'light';
				select.dispatchEvent(new Event('change', { bubbles: true }));
			} else {
				(document.querySelector('[data-testid="appearance-theme"]') as HTMLElement)?.click();
			}
		});

		const hasDialog = await agent.execute(
			() => !!document.querySelector('[data-testid="appearance-theme-light"]'),
		);
		if (hasDialog) {
			await agent.execute(() => {
				(document.querySelector('[data-testid="appearance-theme-light"]') as HTMLElement)?.click();
			});
		}

		await agent.waitUntil(
			async () => agent.execute(
				() => !document.documentElement.classList.contains('dark'),
			),
			{ timeout: 5_000, timeoutMsg: 'Light mode class not applied' },
		);
	});

	it('changes theme back to system default', async () => {
		await agent.execute(() => {
			const select = document.querySelector('[data-testid="appearance-theme"] select') as HTMLSelectElement;
			if (select) {
				select.value = 'system';
				select.dispatchEvent(new Event('change', { bubbles: true }));
			} else {
				(document.querySelector('[data-testid="appearance-theme"]') as HTMLElement)?.click();
			}
		});

		const hasDialog = await agent.execute(
			() => !!document.querySelector('[data-testid="appearance-theme-system"]'),
		);
		if (hasDialog) {
			await agent.execute(() => {
				(document.querySelector('[data-testid="appearance-theme-system"]') as HTMLElement)?.click();
			});
		}

		// System default should not have the dark class (CI/test environments default to light)
		await agent.waitUntil(
			async () => agent.execute(
				() => !document.documentElement.classList.contains('dark'),
			),
			{ timeout: 5_000, timeoutMsg: 'System default did not remove dark class' },
		);
	});
});
