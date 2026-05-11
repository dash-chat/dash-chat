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
	type Agent,
	exchangeContacts,
	setupAgent,
	waitForTestUtils,
} from '../helpers/setup-agents';

type PageResult = { page: string; overflow?: string[]; darkMode?: { issues?: string[] } };
type VisitResult = { ok: boolean; result?: { pages: PageResult[]; summary: { totalIssues: number; pagesVisited: number } }; error?: string };

/** Format all issues from a visit result into a readable failure message. */
function formatIssues(res: VisitResult): string {
	const lines: string[] = [];
	for (const p of res.result?.pages ?? []) {
		const issues: string[] = [];
		if (p.overflow?.length) issues.push(...p.overflow.map((o) => `  overflow: ${o}`));
		if (p.darkMode?.issues?.length) issues.push(...p.darkMode.issues.map((d) => `  dark-mode: ${d}`));
		if (issues.length) lines.push(`[${p.page}]\n${issues.join('\n')}`);
	}
	return lines.join('\n');
}

/** Assert that visitAllPages completed without errors or issues. */
function assertNoIssues(res: VisitResult): void {
	if (!res.ok) {
		throw new Error(`visitAllPages failed: ${res.error}`);
	}
	const total = res.result?.summary.totalIssues ?? 0;
	if (total > 0) {
		throw new Error(`Found ${total} issue(s):\n${formatIssues(res)}`);
	}
}

/** Helper: call visitAllPages on an agent and return the result.
 *  Uses sync execute to start the async function in the browser, then polls
 *  for completion via waitUntil — avoiding executeAsync's 30s hard timeout. */
async function runVisit(
	agent: Agent,
	options: {
		/** Include direct-chat and chat-settings pages (requires prior contact exchange + messaging). */
		hasChat?: boolean;
		checkDarkMode?: boolean;
		checkRTL?: boolean;
	},
): Promise<VisitResult> {
	// Wait for HOME elements before starting (switchCombo layout changes can
	// cause {#await} blocks to re-enter pending state temporarily).
	await agent.waitUntil(async () => !!(await agent.homeLoaded()), {
		timeout: 30_000,
		interval: 500,
		timeoutMsg: 'runVisit: HOME elements not found before starting visitAllPages',
	});

	// Start visitAllPages in the browser context (fire-and-forget via sync execute).
	await agent.execute(
		(opts: { hasChat?: boolean; checkDarkMode?: boolean; checkRTL?: boolean }) => {
			(window as any).__visitResult = undefined;
			(window as any).__visitProgress = 'starting';

			// Hard timeout: guarantee __visitResult is set even if the chain hangs.
			const hardTimer = setTimeout(() => {
				if (typeof (window as any).__visitResult !== 'string') {
					const progress = (window as any).__visitProgress ?? 'unknown';
					(window as any).__visitResult = JSON.stringify({
						ok: false, error: `Hard timeout (100s) — last progress: ${progress}`,
					});
				}
			}, 100_000);

			window.__test
				.visitAllPages(opts)
				.then(
					(r: any) => {
						clearTimeout(hardTimer);
						(window as any).__visitResult = JSON.stringify({ ok: true, result: r });
					},
					(e: any) => {
						clearTimeout(hardTimer);
						(window as any).__visitResult = JSON.stringify({ ok: false, error: String(e) });
					},
				)
				.catch((e: any) => {
					clearTimeout(hardTimer);
					(window as any).__visitResult = JSON.stringify({ ok: false, error: 'catch: ' + String(e) });
				});
		},
		options,
	);

	// Poll until the result is available (up to 120s).
	try {
		await agent.waitUntil(
			async () => agent.execute(() => typeof (window as any).__visitResult === 'string'),
			{ timeout: 120_000, interval: 2_000, timeoutMsg: 'Timeout waiting for visitAllPages to complete' },
		);
	} catch (e) {
		// Read diagnostic info before throwing.
		try {
			const diag = await agent.execute(() => ({
				progress: (window as any).__visitProgress,
				hasResult: typeof (window as any).__visitResult,
				hasTest: typeof (window as any).__test !== 'undefined',
				url: window.location.href,
			}));
			console.log('[runVisit] TIMEOUT diagnostics:', JSON.stringify(diag));
		} catch (diagErr) {
			console.log('[runVisit] TIMEOUT — could not read diagnostics:', String(diagErr));
		}
		throw e;
	}

	// Retrieve and parse the result.
	const raw: string = await agent.execute(() => (window as any).__visitResult);
	const res = JSON.parse(raw) as VisitResult;
	if (!res.ok) {
		console.log('[runVisit] visitAllPages error:', res.error);
	}
	return res;
}

/** Trigger a full page reload and wait for the app to be ready.
 *  Clears window.__test before reloading so waitForTestUtils correctly
 *  waits for re-registration instead of finding the stale reference.
 *  Disables CSS transitions/animations after reload so static layout checks
 *  (dark-mode bg, overflow) don't race against in-flight color transitions. */
async function reloadToHome(agent: Agent): Promise<void> {
	await agent.execute(() => {
		delete (window as any).__test;
		window.location.href = '/';
	});
	await waitForTestUtils(agent);
	await agent.execute(() => {
		const id = '__e2e-no-transitions';
		if (document.getElementById(id)) return;
		const style = document.createElement('style');
		style.id = id;
		style.textContent = '*, *::before, *::after { transition: none !important; animation: none !important; }';
		document.head.appendChild(style);
	});
	await agent.waitUntil(async () => !!(await agent.homeLoaded()), {
		timeout: 30_000,
		interval: 500,
		timeoutMsg: 'reloadToHome: HOME elements not found after reload',
	});
}

/** Helper: switch theme + layout on an agent.
 *  Always reloads the page for a clean DOM/store state, then applies settings.
 *  This avoids cumulative issues from multiple layout switches where
 *  Signalium watchers may not fire after repeated component remounts. */
async function switchCombo(
	agent: Agent,
	theme: 'material' | 'ios',
	wideScreen: boolean,
	dark?: boolean,
): Promise<void> {
	await reloadToHome(agent);

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

	// Wait for HOME elements to re-appear after layout change (switching to
	// desktop causes DesktopLayout to mount fresh, re-rendering AllChats).
	await agent.waitUntil(async () => !!(await agent.homeLoaded()), {
		timeout: 30_000,
		interval: 500,
		timeoutMsg: 'switchCombo: HOME not found after theme/layout apply',
	});
}

describe('Review checks', function () {
	// Each combo does a full page reload + visit ~13 pages. With {#await}-based
	// rendering, promise resolution adds overhead that accumulates across combos.
	// Must be larger than runVisit's 120s poll timeout.
	this.timeout(180_000);

	let agent1: Agent;
	let agent2: Agent;

	before(async function () {
		this.timeout(180_000);

		[agent1, agent2] = await Promise.all([
			setupAgent('agent1'),
			setupAgent('agent2'),
		]);

		await agent1.createProfile('Alice', 'Test');
		await agent2.createProfile('Bob', 'Tester');

		await exchangeContacts(agent1, agent2);

		// Wait for both agents' chat pages to load after contact exchange.
		await Promise.all([
			agent1.waitUntil(async () => !!(await agent1.messageInput()), {
				timeout: 30_000, interval: 500, timeoutMsg: 'Agent1 message input not found after contact exchange',
			}),
			agent2.waitUntil(async () => !!(await agent2.messageInput()), {
				timeout: 30_000, interval: 500, timeoutMsg: 'Agent2 message input not found after contact exchange',
			}),
		]);

		await agent1.sendMessage('Hello from Alice!');
		await agent2.waitForMessage('Hello from Alice!');

		await agent2.sendMessage('Hello from Bob!');
		await agent1.waitForMessage('Hello from Bob!');

		await reloadToHome(agent1);
	});

	// Ensure agent1 is on the home page before each test — if a previous test
	// failed mid-navigation, the app could be on any page.
	beforeEach(async function () {
		this.timeout(120_000);
		if (!(await agent1.homeLoaded())) {
			await reloadToHome(agent1);
		}
	});

	describe('English - Light', function () {
		it('Material Desktop', async function () {
			await switchCombo(agent1, 'material', true);
			assertNoIssues(await runVisit(agent1, { hasChat: true }));
		});

		it('Material Mobile', async function () {
			await switchCombo(agent1, 'material', false);
			assertNoIssues(await runVisit(agent1, { hasChat: true }));
		});

		it('iOS Desktop', async function () {
			await switchCombo(agent1, 'ios', true);
			assertNoIssues(await runVisit(agent1, { hasChat: true }));
		});

		it('iOS Mobile', async function () {
			await switchCombo(agent1, 'ios', false);
			assertNoIssues(await runVisit(agent1, { hasChat: true }));
		});
	});

	describe('English - Dark', function () {
		it('Material Desktop', async function () {
			await switchCombo(agent1, 'material', true, true);
			assertNoIssues(await runVisit(agent1, { hasChat: true, checkDarkMode: true }));
		});

		it('Material Mobile', async function () {
			await switchCombo(agent1, 'material', false, true);
			assertNoIssues(await runVisit(agent1, { hasChat: true, checkDarkMode: true }));
		});

		it('iOS Desktop', async function () {
			await switchCombo(agent1, 'ios', true, true);
			assertNoIssues(await runVisit(agent1, { hasChat: true, checkDarkMode: true }));
		});

		it('iOS Mobile', async function () {
			await switchCombo(agent1, 'ios', false, true);
			assertNoIssues(await runVisit(agent1, { hasChat: true, checkDarkMode: true }));
		});
	});

	describe('German (de-de)', function () {
		before(async function () {
			this.timeout(60_000);
			// Navigate to home first: setLocale reloads at the current URL
			// (locale-prefixed), so we must be on '/' before changing locale.
			await reloadToHome(agent1);
			// setLocale triggers a full page reload with the new locale prefix.
			await agent1.execute(() => {
				delete (window as any).__test;
				window.__setLocale('de-de');
			});
			await waitForTestUtils(agent1);
			await agent1.waitUntil(async () => !!(await agent1.homeLoaded()), {
				timeout: 30_000, interval: 500, timeoutMsg: 'German locale: HOME not found after setLocale',
			});
		});

		it('Material Desktop', async function () {
			await switchCombo(agent1, 'material', true);
			assertNoIssues(await runVisit(agent1, { hasChat: true }));
		});

		it('Material Mobile', async function () {
			await switchCombo(agent1, 'material', false);
			assertNoIssues(await runVisit(agent1, { hasChat: true }));
		});

		it('iOS Desktop', async function () {
			await switchCombo(agent1, 'ios', true);
			assertNoIssues(await runVisit(agent1, { hasChat: true }));
		});

		it('iOS Mobile', async function () {
			await switchCombo(agent1, 'ios', false);
			assertNoIssues(await runVisit(agent1, { hasChat: true }));
		});
	});

	describe('Farsi RTL (fa-ir)', function () {
		before(async function () {
			this.timeout(60_000);
			// Navigate to home first: setLocale reloads at current URL.
			await reloadToHome(agent1);
			// setLocale triggers a full page reload with the new locale prefix.
			await agent1.execute(() => {
				delete (window as any).__test;
				window.__setLocale('fa-ir');
			});
			await waitForTestUtils(agent1);
			await agent1.waitUntil(async () => !!(await agent1.homeLoaded()), {
				timeout: 30_000, interval: 500, timeoutMsg: 'Farsi locale: HOME not found after setLocale',
			});
			await agent1.execute(() => { document.documentElement.dir = 'rtl'; });
		});

		it('Material Desktop', async function () {
			await switchCombo(agent1, 'material', true);
			await agent1.execute(() => { document.documentElement.dir = 'rtl'; });
			assertNoIssues(await runVisit(agent1, { hasChat: true, checkRTL: true }));
		});

		it('Material Mobile', async function () {
			await switchCombo(agent1, 'material', false);
			await agent1.execute(() => { document.documentElement.dir = 'rtl'; });
			assertNoIssues(await runVisit(agent1, { hasChat: true, checkRTL: true }));
		});

		it('iOS Desktop', async function () {
			await switchCombo(agent1, 'ios', true);
			await agent1.execute(() => { document.documentElement.dir = 'rtl'; });
			assertNoIssues(await runVisit(agent1, { hasChat: true, checkRTL: true }));
		});

		it('iOS Mobile', async function () {
			await switchCombo(agent1, 'ios', false);
			await agent1.execute(() => { document.documentElement.dir = 'rtl'; });
			assertNoIssues(await runVisit(agent1, { hasChat: true, checkRTL: true }));
		});
	});
});
