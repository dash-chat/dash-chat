/**
 * Shared helpers for E2E test setup.
 *
 * Wraps the executeAsync/execute dance needed to call window.__test functions
 * from WebdriverIO, with proper error handling. All helpers throw on failure.
 */

/** Wait for window.__test to be registered on a single agent. */
export async function waitForTestUtils(agent: WebdriverIO.Browser): Promise<void> {
	await agent.waitUntil(
		async () => agent.execute(() => typeof window.__test !== 'undefined'),
		{ timeout: 30_000, interval: 500, timeoutMsg: 'window.__test not registered' },
	);
}

/** Wait for window.__test to be registered on both agents. */
export async function waitForBothAgents(): Promise<void> {
	const agent1 = browser.getInstance('agent1');
	const agent2 = browser.getInstance('agent2');
	await Promise.all([waitForTestUtils(agent1), waitForTestUtils(agent2)]);
}

/** Create a profile on an agent. Throws if creation fails. */
export async function createProfile(
	agent: WebdriverIO.Browser,
	name: string,
	surname: string,
): Promise<void> {
	const err = await agent.executeAsync(
		(n: string, s: string, done: (r: string | null) => void) => {
			window.__test.createProfile(n, s).then(() => done(null), (e) => done(String(e)));
		},
		name,
		surname,
	);
	if (err) throw new Error(`Profile creation failed: ${err}`);
}

/** Navigate to add-contact page and return the contact code. */
export async function getContactCode(agent: WebdriverIO.Browser): Promise<string> {
	const navErr = await agent.executeAsync((done: (r: string | null) => void) => {
		window.__test.navigateToAddContact().then(() => done(null), (e) => done(String(e)));
	});
	if (navErr) throw new Error(`Navigate to add-contact failed: ${navErr}`);

	const code = await agent.execute(() => window.__test.getContactCode());
	if (!code) throw new Error('Failed to get contact code');
	return code as string;
}

/** Add a contact by code. Throws if it fails. */
export async function addContact(agent: WebdriverIO.Browser, code: string): Promise<void> {
	const err = await agent.executeAsync(
		(c: string, done: (r: string | null) => void) => {
			window.__test.addContact(c).then(() => done(null), (e) => done(String(e)));
		},
		code,
	);
	if (err) throw new Error(`Add contact failed: ${err}`);
}

/** Exchange contact codes between two agents. */
export async function exchangeContacts(
	agent1: WebdriverIO.Browser,
	agent2: WebdriverIO.Browser,
): Promise<void> {
	const code1 = await getContactCode(agent1);
	const code2 = await getContactCode(agent2);
	await addContact(agent1, code2);
	await addContact(agent2, code1);
}

/** Send a message from an agent. Throws if it fails. */
export async function sendMessage(agent: WebdriverIO.Browser, text: string): Promise<void> {
	const err = await agent.executeAsync(
		(t: string, done: (r: string | null) => void) => {
			window.__test.sendMessage(t).then(() => done(null), (e) => done(String(e)));
		},
		text,
	);
	if (err) throw new Error(`Send message failed: ${err}`);
}

/**
 * Wait for a message to appear on an agent.
 * Uses WDIO waitUntil with sync execute polling — avoids executeAsync with
 * long-running scripts which can hang in tauri-driver.
 */
export async function waitForMessage(agent: WebdriverIO.Browser, text: string, timeout = 60_000): Promise<void> {
	await agent.waitUntil(
		async () => agent.execute(
			(t: string) => !!document.querySelector('[data-testid="direct-chat-messages"]')?.textContent?.includes(t),
			text,
		),
		{ timeout, interval: 1_000, timeoutMsg: `Message "${text}" not received within ${timeout}ms` },
	);
}

/** Send a message and wait for it to be received by another agent. */
export async function sendAndReceiveMessage(
	sender: WebdriverIO.Browser,
	receiver: WebdriverIO.Browser,
	text: string,
): Promise<void> {
	await sendMessage(sender, text);
	await waitForMessage(receiver, text);
}

/** Open a direct chat by contact name. Throws if it fails. */
export async function openDirectChat(
	agent: WebdriverIO.Browser,
	contactName: string,
): Promise<void> {
	const err = await agent.executeAsync(
		(name: string, done: (r: string | null) => void) => {
			window.__test.openDirectChat(name).then(() => done(null), (e) => done(String(e)));
		},
		contactName,
	);
	if (err) throw new Error(`Open direct chat failed: ${err}`);
}
