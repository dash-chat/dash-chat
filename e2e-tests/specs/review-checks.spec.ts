/**
 * Review checks E2E test.
 *
 * Exercises all 16 theme × layout × color × locale combinations by calling
 * window.__test.visitAllPages() via executeAsync. Each combo navigates
 * through ~15 pages, running overflow/dark-mode/RTL checks at each stop.
 *
 * Uses the same window.__test functions as the review-app skill.
 */

import {
	waitForTestUtils,
	waitForBothAgents,
	createProfile,
	exchangeContacts,
	sendAndReceiveMessage,
} from '../helpers/setup-agents';

/** Helper: call visitAllPages on an agent and return the result. */
async function runVisit(
	agent: WebdriverIO.Browser,
	options: { hasChat?: boolean; checkDarkMode?: boolean; checkRTL?: boolean },
): Promise<{ ok: boolean; result?: { pages: unknown[]; summary: { totalIssues: number; pagesVisited: number } }; error?: string }> {
	return agent.executeAsync(
		(opts: { hasChat?: boolean; checkDarkMode?: boolean; checkRTL?: boolean }, done: (r: unknown) => void) => {
			window.__test
				.visitAllPages(opts)
				.then(
					(r) => done({ ok: true, result: r }),
					(e) => done({ ok: false, error: String(e) }),
				);
		},
		options,
	) as Promise<{ ok: boolean; result?: { pages: unknown[]; summary: { totalIssues: number; pagesVisited: number } }; error?: string }>;
}

/** Helper: switch theme + layout on an agent. */
async function switchCombo(
	agent: WebdriverIO.Browser,
	theme: 'material' | 'ios',
	wideScreen: boolean,
): Promise<void> {
	await agent.execute(
		(t: string, w: boolean) => {
			window.dispatchEvent(new CustomEvent('theme-change', { detail: { theme: t } }));
			window.dispatchEvent(new CustomEvent('set-wide-screen', { detail: w }));
		},
		theme,
		wideScreen,
	);
	// Let theme/layout changes settle
	await agent.pause(500);
}

describe('Review checks', function () {
	before(async function () {
		this.timeout(180_000);

		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');

		await waitForBothAgents();

		await createProfile(agent1, 'Alice', 'Test');
		await createProfile(agent2, 'Bob', 'Tester');

		await exchangeContacts(agent1, agent2);

		await sendAndReceiveMessage(agent1, agent2, 'Hello from Alice!');
		await sendAndReceiveMessage(agent2, agent1, 'Hello from Bob!');

		// Navigate agent 1 back to home for the review
		await agent1.executeAsync((done: (r: string | null) => void) => {
			window.__test.click('[data-testid="direct-chat-back"]');
			window.__test
				.waitFor('[data-testid="all-chats-list"], [data-testid="all-chats-empty"]')
				.then(() => done(null), (e) => done(String(e)));
		});
	});

	describe('English - Light', function () {
		it('Material Desktop', async function () {
			const agent1 = browser.getInstance('agent1');
			await switchCombo(agent1, 'material', true);
			const res = await runVisit(agent1, { hasChat: true });
			expect(res.ok).toBe(true);
			if (res.result && res.result.summary.totalIssues > 0) {
				console.log('Issues found:', JSON.stringify(res.result.pages.filter((p: any) => p.overflow?.length > 0), null, 2));
			}
			expect(res.result?.summary.totalIssues).toBe(0);
		});

		it('Material Mobile', async function () {
			const agent1 = browser.getInstance('agent1');
			await switchCombo(agent1, 'material', false);
			const res = await runVisit(agent1, { hasChat: true });
			expect(res.ok).toBe(true);
			if (res.result && res.result.summary.totalIssues > 0) {
				console.log('Issues found:', JSON.stringify(res.result.pages.filter((p: any) => p.overflow?.length > 0), null, 2));
			}
			expect(res.result?.summary.totalIssues).toBe(0);
		});

		it('iOS Desktop', async function () {
			const agent1 = browser.getInstance('agent1');
			await switchCombo(agent1, 'ios', true);
			const res = await runVisit(agent1, { hasChat: true });
			expect(res.ok).toBe(true);
			if (res.result && res.result.summary.totalIssues > 0) {
				console.log('Issues found:', JSON.stringify(res.result.pages.filter((p: any) => p.overflow?.length > 0), null, 2));
			}
			expect(res.result?.summary.totalIssues).toBe(0);
		});

		it('iOS Mobile', async function () {
			const agent1 = browser.getInstance('agent1');
			await switchCombo(agent1, 'ios', false);
			const res = await runVisit(agent1, { hasChat: true });
			expect(res.ok).toBe(true);
			if (res.result && res.result.summary.totalIssues > 0) {
				console.log('Issues found:', JSON.stringify(res.result.pages.filter((p: any) => p.overflow?.length > 0), null, 2));
			}
			expect(res.result?.summary.totalIssues).toBe(0);
		});
	});

	describe('English - Dark', function () {
		before(async function () {
			const agent1 = browser.getInstance('agent1');
			await agent1.execute(() => {
				window.dispatchEvent(new CustomEvent('set-dark-mode', { detail: true }));
			});
			await agent1.pause(300);
		});

		it('Material Desktop', async function () {
			const agent1 = browser.getInstance('agent1');
			await switchCombo(agent1, 'material', true);
			const res = await runVisit(agent1, { hasChat: true, checkDarkMode: true });
			expect(res.ok).toBe(true);
			if (res.result && res.result.summary.totalIssues > 0) {
				console.log('Dark mode issues:', JSON.stringify(res.result.pages.filter((p: any) => (p.overflow?.length > 0) || (p.darkMode?.issues?.length > 0)), null, 2));
			}
			expect(res.result?.summary.totalIssues).toBe(0);
		});

		it('Material Mobile', async function () {
			const agent1 = browser.getInstance('agent1');
			await switchCombo(agent1, 'material', false);
			const res = await runVisit(agent1, { hasChat: true, checkDarkMode: true });
			expect(res.ok).toBe(true);
			expect(res.result?.summary.totalIssues).toBe(0);
		});

		it('iOS Desktop', async function () {
			const agent1 = browser.getInstance('agent1');
			await switchCombo(agent1, 'ios', true);
			const res = await runVisit(agent1, { hasChat: true, checkDarkMode: true });
			expect(res.ok).toBe(true);
			expect(res.result?.summary.totalIssues).toBe(0);
		});

		it('iOS Mobile', async function () {
			const agent1 = browser.getInstance('agent1');
			await switchCombo(agent1, 'ios', false);
			const res = await runVisit(agent1, { hasChat: true, checkDarkMode: true });
			expect(res.ok).toBe(true);
			expect(res.result?.summary.totalIssues).toBe(0);
		});

		after(async function () {
			const agent1 = browser.getInstance('agent1');
			await agent1.execute(() => {
				window.dispatchEvent(new CustomEvent('set-dark-mode', { detail: false }));
			});
		});
	});

	describe('German (de-de)', function () {
		before(async function () {
			this.timeout(60_000);
			const agent1 = browser.getInstance('agent1');
			// Locale change reloads the page, resets theme to Material
			await agent1.execute(() => window.__setLocale('de-de'));
			await agent1.pause(2000);
			await waitForTestUtils(agent1);
		});

		it('Material Desktop', async function () {
			const agent1 = browser.getInstance('agent1');
			await switchCombo(agent1, 'material', true);
			const res = await runVisit(agent1, { hasChat: true });
			expect(res.ok).toBe(true);
			if (res.result && res.result.summary.totalIssues > 0) {
				console.log('German issues:', JSON.stringify(res.result.pages.filter((p: any) => p.overflow?.length > 0), null, 2));
			}
			expect(res.result?.summary.totalIssues).toBe(0);
		});

		it('Material Mobile', async function () {
			const agent1 = browser.getInstance('agent1');
			await switchCombo(agent1, 'material', false);
			const res = await runVisit(agent1, { hasChat: true });
			expect(res.ok).toBe(true);
			expect(res.result?.summary.totalIssues).toBe(0);
		});

		it('iOS Desktop', async function () {
			const agent1 = browser.getInstance('agent1');
			await switchCombo(agent1, 'ios', true);
			const res = await runVisit(agent1, { hasChat: true });
			expect(res.ok).toBe(true);
			expect(res.result?.summary.totalIssues).toBe(0);
		});

		it('iOS Mobile', async function () {
			const agent1 = browser.getInstance('agent1');
			await switchCombo(agent1, 'ios', false);
			const res = await runVisit(agent1, { hasChat: true });
			expect(res.ok).toBe(true);
			expect(res.result?.summary.totalIssues).toBe(0);
		});
	});

	describe('Farsi RTL (fa-ir)', function () {
		before(async function () {
			this.timeout(60_000);
			const agent1 = browser.getInstance('agent1');
			// Locale change reloads the page, resets theme to Material
			await agent1.execute(() => window.__setLocale('fa-ir'));
			await agent1.pause(2000);
			await waitForTestUtils(agent1);
			// Set RTL direction
			await agent1.execute(() => {
				document.documentElement.dir = 'rtl';
			});
			await agent1.pause(300);
		});

		it('Material Desktop', async function () {
			const agent1 = browser.getInstance('agent1');
			await switchCombo(agent1, 'material', true);
			// Re-apply RTL after theme change
			await agent1.execute(() => {
				document.documentElement.dir = 'rtl';
			});
			const res = await runVisit(agent1, { hasChat: true, checkRTL: true });
			expect(res.ok).toBe(true);
			if (res.result && res.result.summary.totalIssues > 0) {
				console.log('Farsi issues:', JSON.stringify(res.result.pages.filter((p: any) => p.overflow?.length > 0), null, 2));
			}
			expect(res.result?.summary.totalIssues).toBe(0);
		});

		it('Material Mobile', async function () {
			const agent1 = browser.getInstance('agent1');
			await switchCombo(agent1, 'material', false);
			await agent1.execute(() => {
				document.documentElement.dir = 'rtl';
			});
			const res = await runVisit(agent1, { hasChat: true, checkRTL: true });
			expect(res.ok).toBe(true);
			expect(res.result?.summary.totalIssues).toBe(0);
		});

		it('iOS Desktop', async function () {
			const agent1 = browser.getInstance('agent1');
			await switchCombo(agent1, 'ios', true);
			await agent1.execute(() => {
				document.documentElement.dir = 'rtl';
			});
			const res = await runVisit(agent1, { hasChat: true, checkRTL: true });
			expect(res.ok).toBe(true);
			expect(res.result?.summary.totalIssues).toBe(0);
		});

		it('iOS Mobile', async function () {
			const agent1 = browser.getInstance('agent1');
			await switchCombo(agent1, 'ios', false);
			await agent1.execute(() => {
				document.documentElement.dir = 'rtl';
			});
			const res = await runVisit(agent1, { hasChat: true, checkRTL: true });
			expect(res.ok).toBe(true);
			expect(res.result?.summary.totalIssues).toBe(0);
		});
	});
});
