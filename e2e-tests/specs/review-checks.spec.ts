/**
 * Review checks E2E test.
 *
 * Exercises all 16 theme × layout × color × locale combinations by calling
 * window.__test.visitAllPages() via executeAsync. Each combo navigates
 * through ~15 pages, running overflow/dark-mode/RTL checks at each stop.
 *
 * Uses the same window.__test functions as the review-app skill.
 */

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

/** Helper: wait for window.__test to be registered after a page reload. */
async function waitForTestUtils(agent: WebdriverIO.Browser): Promise<void> {
	await agent.waitUntil(
		async () => agent.execute(() => typeof window.__test !== 'undefined'),
		{ timeout: 30_000, interval: 500, timeoutMsg: 'window.__test not registered after reload' },
	);
}

describe('Review checks', function () {
	before(async function () {
		this.timeout(180_000);

		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');

		// Wait for window.__test on both agents
		await Promise.all([waitForTestUtils(agent1), waitForTestUtils(agent2)]);

		// Create profiles
		const err1 = await agent1.executeAsync(
			(name: string, surname: string, done: (r: string | null) => void) => {
				window.__test.createProfile(name, surname).then(() => done(null), (e) => done(String(e)));
			},
			'Alice',
			'Test',
		);
		if (err1) throw new Error(`Agent 1 profile creation failed: ${err1}`);

		const err2 = await agent2.executeAsync(
			(name: string, surname: string, done: (r: string | null) => void) => {
				window.__test.createProfile(name, surname).then(() => done(null), (e) => done(String(e)));
			},
			'Bob',
			'Tester',
		);
		if (err2) throw new Error(`Agent 2 profile creation failed: ${err2}`);

		// Exchange contacts
		const navErr1 = await agent1.executeAsync((done: (r: string | null) => void) => {
			window.__test.navigateToAddContact().then(() => done(null), (e) => done(String(e)));
		});
		if (navErr1) throw new Error(`Agent 1 nav to add-contact failed: ${navErr1}`);

		const aliceCode = await agent1.execute(() => window.__test.getContactCode());
		if (!aliceCode) throw new Error('Failed to get Alice contact code');

		const navErr2 = await agent2.executeAsync((done: (r: string | null) => void) => {
			window.__test.navigateToAddContact().then(() => done(null), (e) => done(String(e)));
		});
		if (navErr2) throw new Error(`Agent 2 nav to add-contact failed: ${navErr2}`);

		const bobCode = await agent2.execute(() => window.__test.getContactCode());
		if (!bobCode) throw new Error('Failed to get Bob contact code');

		const addErr1 = await agent1.executeAsync(
			(code: string, done: (r: string | null) => void) => {
				window.__test.addContact(code).then(() => done(null), (e) => done(String(e)));
			},
			bobCode as string,
		);
		if (addErr1) throw new Error(`Agent 1 add contact failed: ${addErr1}`);

		const addErr2 = await agent2.executeAsync(
			(code: string, done: (r: string | null) => void) => {
				window.__test.addContact(code).then(() => done(null), (e) => done(String(e)));
			},
			aliceCode as string,
		);
		if (addErr2) throw new Error(`Agent 2 add contact failed: ${addErr2}`);

		// Send messages
		const sendErr = await agent1.executeAsync(
			(text: string, done: (r: string | null) => void) => {
				window.__test.sendMessage(text).then(() => done(null), (e) => done(String(e)));
			},
			'Hello from Alice!',
		);
		if (sendErr) throw new Error(`Agent 1 send message failed: ${sendErr}`);

		const recvErr = await agent2.executeAsync(
			(text: string, done: (r: string | null) => void) => {
				window.__test.waitForMessage(text).then(() => done(null), (e) => done(String(e)));
			},
			'Hello from Alice!',
		);
		if (recvErr) throw new Error(`Agent 2 receive message failed: ${recvErr}`);

		const replyErr = await agent2.executeAsync(
			(text: string, done: (r: string | null) => void) => {
				window.__test.sendMessage(text).then(() => done(null), (e) => done(String(e)));
			},
			'Hello from Bob!',
		);
		if (replyErr) throw new Error(`Agent 2 reply failed: ${replyErr}`);

		const recvReplyErr = await agent1.executeAsync(
			(text: string, done: (r: string | null) => void) => {
				window.__test.waitForMessage(text).then(() => done(null), (e) => done(String(e)));
			},
			'Hello from Bob!',
		);
		if (recvReplyErr) throw new Error(`Agent 1 receive reply failed: ${recvReplyErr}`);

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
