/**
 * Full messaging flow E2E test.
 *
 * Uses two Tauri instances (agent1 & agent2) via WebdriverIO multiremote.
 * Calls window.__test functions registered by ui/tests/setup-utils.ts.
 *
 * Async test functions (createProfile, navigateToAddContact, etc.) return
 * Promises, which the W3C WebDriver "execute/sync" endpoint cannot serialize.
 * We use executeAsync with a done callback so the driver waits for resolution.
 */

describe('Full messaging flow', () => {
	before(async () => {
		// Wait for the app to load and register window.__test on both agents
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');

		await Promise.all([
			agent1.waitUntil(
				async () => agent1.execute(() => typeof window.__test !== 'undefined'),
				{ timeout: 30_000, interval: 500, timeoutMsg: 'agent1: window.__test not registered' },
			),
			agent2.waitUntil(
				async () => agent2.execute(() => typeof window.__test !== 'undefined'),
				{ timeout: 30_000, interval: 500, timeoutMsg: 'agent2: window.__test not registered' },
			),
		]);
	});

	it('creates profiles on both agents', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');

		const err1 = await agent1.executeAsync(
			(name: string, surname: string, done: (r: string | null) => void) => {
				window.__test.createProfile(name, surname).then(() => done(null), (e) => done(String(e)));
			},
			'Alice',
			'Test',
		);
		expect(err1).toBeNull();

		const err2 = await agent2.executeAsync(
			(name: string, surname: string, done: (r: string | null) => void) => {
				window.__test.createProfile(name, surname).then(() => done(null), (e) => done(String(e)));
			},
			'Bob',
			'Test',
		);
		expect(err2).toBeNull();
	});

	it('exchanges contact codes between agents', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');

		// Agent 1: navigate to add-contact and get code
		const navErr1 = await agent1.executeAsync((done: (r: string | null) => void) => {
			window.__test.navigateToAddContact().then(() => done(null), (e) => done(String(e)));
		});
		expect(navErr1).toBeNull();

		const aliceCode = await agent1.execute(() => window.__test.getContactCode());
		expect(aliceCode).toBeTruthy();

		// Agent 2: navigate to add-contact and get code
		const navErr2 = await agent2.executeAsync((done: (r: string | null) => void) => {
			window.__test.navigateToAddContact().then(() => done(null), (e) => done(String(e)));
		});
		expect(navErr2).toBeNull();

		const bobCode = await agent2.execute(() => window.__test.getContactCode());
		expect(bobCode).toBeTruthy();

		// Exchange codes — each agent adds the other's code
		const addErr1 = await agent1.executeAsync(
			(code: string, done: (r: string | null) => void) => {
				window.__test.addContact(code).then(() => done(null), (e) => done(String(e)));
			},
			bobCode as string,
		);
		expect(addErr1).toBeNull();

		const addErr2 = await agent2.executeAsync(
			(code: string, done: (r: string | null) => void) => {
				window.__test.addContact(code).then(() => done(null), (e) => done(String(e)));
			},
			aliceCode as string,
		);
		expect(addErr2).toBeNull();
	});

	it('sends a message from Alice to Bob', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');

		// Agent 1 (Alice) sends a message
		const sendErr = await agent1.executeAsync(
			(text: string, done: (r: string | null) => void) => {
				window.__test.sendMessage(text).then(() => done(null), (e) => done(String(e)));
			},
			'Hello from Alice!',
		);
		expect(sendErr).toBeNull();

		// Agent 2 (Bob) waits for the message to appear
		const recvErr = await agent2.executeAsync(
			(text: string, done: (r: string | null) => void) => {
				window.__test.waitForMessage(text).then(() => done(null), (e) => done(String(e)));
			},
			'Hello from Alice!',
		);
		expect(recvErr).toBeNull();
	});

	it('sends a reply from Bob to Alice', async () => {
		const agent1 = browser.getInstance('agent1');
		const agent2 = browser.getInstance('agent2');

		const sendErr = await agent2.executeAsync(
			(text: string, done: (r: string | null) => void) => {
				window.__test.sendMessage(text).then(() => done(null), (e) => done(String(e)));
			},
			'Hello from Bob!',
		);
		expect(sendErr).toBeNull();

		const recvErr = await agent1.executeAsync(
			(text: string, done: (r: string | null) => void) => {
				window.__test.waitForMessage(text).then(() => done(null), (e) => done(String(e)));
			},
			'Hello from Bob!',
		);
		expect(recvErr).toBeNull();
	});
});
