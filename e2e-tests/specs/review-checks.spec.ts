/**
 * Review checks E2E test.
 *
 * Exercises all 16 theme × layout × color × locale combinations by calling
 * window.__test.visitAllPages(). Each combo navigates through ~15 pages,
 * running overflow/dark-mode/RTL checks at each stop.
 *
 * Uses sync execute + waitUntil polling to avoid executeAsync timeout issues
 * with tauri-driver (WebKitWebDriver has a fixed 30s script timeout).
 *
 * Uses the same window.__test functions as the review-app skill.
 */

import {
	waitForTestUtils,
	waitForBothAgents,
	createProfile,
	exchangeContacts,
} from '../helpers/setup-agents';

type VisitResult = { ok: boolean; result?: { pages: unknown[]; summary: { totalIssues: number; pagesVisited: number } }; error?: string };

/** Helper: call visitAllPages on an agent and return the result.
 *  Uses sync execute to start the async function in the browser, then polls
 *  for completion via waitUntil — avoiding executeAsync's 30s hard timeout. */
async function runVisit(
	agent: WebdriverIO.Browser,
	options: { hasChat?: boolean; checkDarkMode?: boolean; checkRTL?: boolean },
): Promise<VisitResult> {
	// Wait for HOME elements before starting (switchCombo layout changes can
	// cause {#await} blocks to re-enter pending state temporarily).
	await agent.waitUntil(
		async () => agent.execute(
			() => !!document.querySelector('[data-testid="all-chats-list"]') || !!document.querySelector('[data-testid="all-chats-empty"]'),
		),
		{ timeout: 30_000, interval: 500, timeoutMsg: 'runVisit: HOME elements not found before starting visitAllPages' },
	);

	// Start visitAllPages in the browser context (fire-and-forget via sync execute).
	await agent.execute(
		(opts: { hasChat?: boolean; checkDarkMode?: boolean; checkRTL?: boolean }) => {
			(window as any).__visitResult = undefined;
			window.__test
				.visitAllPages(opts)
				.then(
					(r: any) => { (window as any).__visitResult = JSON.stringify({ ok: true, result: r }); },
					(e: any) => { (window as any).__visitResult = JSON.stringify({ ok: false, error: String(e) }); },
				);
		},
		options,
	);

	// Poll until the result is available (up to 120s).
	await agent.waitUntil(
		async () => agent.execute(() => typeof (window as any).__visitResult === 'string'),
		{ timeout: 120_000, interval: 2_000, timeoutMsg: 'Timeout waiting for visitAllPages to complete' },
	);

	// Retrieve and parse the result.
	const raw: string = await agent.execute(() => (window as any).__visitResult);
	const res = JSON.parse(raw) as VisitResult;
	if (!res.ok) {
		console.log('[runVisit] visitAllPages error:', res.error);
	}
	return res;
}

/** Helper: switch theme + layout on an agent.
 *  Always reloads the page for a clean DOM/store state, then applies settings.
 *  This avoids cumulative issues from multiple layout switches where
 *  Signalium watchers may not fire after repeated component remounts. */
async function switchCombo(
	agent: WebdriverIO.Browser,
	theme: 'material' | 'ios',
	wideScreen: boolean,
	dark?: boolean,
): Promise<void> {
	// Full reload for clean state.
	await agent.execute(() => { window.location.href = '/'; });
	await agent.pause(3000);
	await waitForTestUtils(agent);

	// Apply theme, layout, and dark mode (all lost on reload).
	await agent.execute(
		(t: string, w: boolean, d: boolean) => {
			window.dispatchEvent(new CustomEvent('theme-change', { detail: { theme: t } }));
			window.dispatchEvent(new CustomEvent('set-wide-screen', { detail: w }));
			if (d) {
				window.dispatchEvent(new CustomEvent('set-dark-mode', { detail: true }));
			}
		},
		theme,
		wideScreen,
		!!dark,
	);

	// Wait for HOME elements to appear after layout change.
	await agent.waitUntil(
		async () => agent.execute(
			() => !!document.querySelector('[data-testid="all-chats-list"]') || !!document.querySelector('[data-testid="all-chats-empty"]'),
		),
		{ timeout: 30_000, interval: 500, timeoutMsg: 'switchCombo: HOME not found after reload + apply' },
	);
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

		// Let nodes settle after contact exchange (p2panda sync, mailbox subscribe).
		await agent1.pause(5_000);

		// Send messages using sync execute calls to avoid executeAsync hangs.
		await agent1.execute((t: string) => {
			const el = document.querySelector('[data-testid="message-input-textarea"]') as HTMLTextAreaElement;
			if (!el) throw new Error('message-input-textarea not found');
			const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')!.set!;
			setter.call(el, t);
			el.dispatchEvent(new Event('input', { bubbles: true }));
			el.dispatchEvent(new Event('change', { bubbles: true }));
			const btn = document.querySelector('[data-testid="message-input-send"]') as HTMLElement;
			if (!btn) throw new Error('message-input-send not found');
			btn.click();
		}, 'Hello from Alice!');

		// Wait for agent2 to receive the message.
		await new Promise((r) => setTimeout(r, 3_000));
		await agent2.waitUntil(
			async () => agent2.execute(
				(t: string) => !!document.querySelector('[data-testid="direct-chat-messages"]')?.textContent?.includes(t),
				'Hello from Alice!',
			),
			{ timeout: 30_000, interval: 2_000, timeoutMsg: 'Agent2 did not receive message from Alice' },
		);

		// Agent2 replies.
		await agent2.execute((t: string) => {
			const el = document.querySelector('[data-testid="message-input-textarea"]') as HTMLTextAreaElement;
			if (!el) throw new Error('message-input-textarea not found');
			const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')!.set!;
			setter.call(el, t);
			el.dispatchEvent(new Event('input', { bubbles: true }));
			el.dispatchEvent(new Event('change', { bubbles: true }));
			const btn = document.querySelector('[data-testid="message-input-send"]') as HTMLElement;
			if (!btn) throw new Error('message-input-send not found');
			btn.click();
		}, 'Hello from Bob!');

		await new Promise((r) => setTimeout(r, 3_000));
		await agent1.waitUntil(
			async () => agent1.execute(
				(t: string) => !!document.querySelector('[data-testid="direct-chat-messages"]')?.textContent?.includes(t),
				'Hello from Bob!',
			),
			{ timeout: 30_000, interval: 1_000, timeoutMsg: 'Agent1 did not receive message from Bob' },
		);

		// Navigate agent1 to home via full page reload (goto('/') can re-mount the
		// page and leave stores in a pending state).
		await agent1.execute(() => { window.location.href = '/'; });
		await agent1.pause(3000);
		await waitForTestUtils(agent1);
		await agent1.waitUntil(
			async () => agent1.execute(
				() => !!document.querySelector('[data-testid="all-chats-list"]') || !!document.querySelector('[data-testid="all-chats-empty"]'),
			),
			{ timeout: 30_000, interval: 1_000 },
		);
	});

	// Ensure agent1 is on the home page before each test — if a previous test
	// failed mid-navigation, the app could be on any page.
	beforeEach(async function () {
		this.timeout(60_000);
		const agent1 = browser.getInstance('agent1');
		const isHome = await agent1.execute(
			() => !!document.querySelector('[data-testid="all-chats-list"]') || !!document.querySelector('[data-testid="all-chats-empty"]'),
		);
		if (!isHome) {
			// Full page reload for reliable recovery (goto can fail if stores are broken).
			await agent1.execute(() => { window.location.href = '/'; });
			await agent1.pause(3000);
			await waitForTestUtils(agent1);
			await agent1.waitUntil(
				async () => agent1.execute(
					() => !!document.querySelector('[data-testid="all-chats-list"]') || !!document.querySelector('[data-testid="all-chats-empty"]'),
				),
				{ timeout: 30_000, interval: 1_000, timeoutMsg: 'beforeEach: HOME elements not found after reload' },
			);
		}
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
		it('Material Desktop', async function () {
			const agent1 = browser.getInstance('agent1');
			await switchCombo(agent1, 'material', true, true);
			const res = await runVisit(agent1, { hasChat: true, checkDarkMode: true });
			expect(res.ok).toBe(true);
			if (res.result && res.result.summary.totalIssues > 0) {
				console.log('Dark mode issues:', JSON.stringify(res.result.pages.filter((p: any) => (p.overflow?.length > 0) || (p.darkMode?.issues?.length > 0)), null, 2));
			}
			expect(res.result?.summary.totalIssues).toBe(0);
		});

		it('Material Mobile', async function () {
			const agent1 = browser.getInstance('agent1');
			await switchCombo(agent1, 'material', false, true);
			const res = await runVisit(agent1, { hasChat: true, checkDarkMode: true });
			expect(res.ok).toBe(true);
			expect(res.result?.summary.totalIssues).toBe(0);
		});

		it('iOS Desktop', async function () {
			const agent1 = browser.getInstance('agent1');
			await switchCombo(agent1, 'ios', true, true);
			const res = await runVisit(agent1, { hasChat: true, checkDarkMode: true });
			expect(res.ok).toBe(true);
			expect(res.result?.summary.totalIssues).toBe(0);
		});

		it('iOS Mobile', async function () {
			const agent1 = browser.getInstance('agent1');
			await switchCombo(agent1, 'ios', false, true);
			const res = await runVisit(agent1, { hasChat: true, checkDarkMode: true });
			expect(res.ok).toBe(true);
			expect(res.result?.summary.totalIssues).toBe(0);
		});
	});

	describe('German (de-de)', function () {
		before(async function () {
			this.timeout(60_000);
			const agent1 = browser.getInstance('agent1');
			// Navigate to home first: setLocale reloads the page at the current
			// URL (locale-prefixed), so we must be on '/' before changing locale.
			await agent1.execute(() => { window.location.href = '/'; });
			await agent1.pause(3000);
			await waitForTestUtils(agent1);
			await agent1.execute(() => window.__setLocale('de-de'));
			await agent1.pause(3000);
			await waitForTestUtils(agent1);
			// Wait for HOME elements after locale reload (stores need to settle).
			await agent1.waitUntil(
				async () => agent1.execute(
					() => !!document.querySelector('[data-testid="all-chats-list"]') || !!document.querySelector('[data-testid="all-chats-empty"]'),
				),
				{ timeout: 30_000, interval: 1_000 },
			);
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
			// Navigate to home first: setLocale reloads at current URL.
			await agent1.execute(() => { window.location.href = '/'; });
			await agent1.pause(3000);
			await waitForTestUtils(agent1);
			await agent1.execute(() => window.__setLocale('fa-ir'));
			await agent1.pause(3000);
			await waitForTestUtils(agent1);
			// Wait for HOME elements after locale reload (stores need to settle).
			await agent1.waitUntil(
				async () => agent1.execute(
					() => !!document.querySelector('[data-testid="all-chats-list"]') || !!document.querySelector('[data-testid="all-chats-empty"]'),
				),
				{ timeout: 30_000, interval: 1_000 },
			);
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
