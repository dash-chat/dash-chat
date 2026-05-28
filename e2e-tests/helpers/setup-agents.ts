/**
 * Shared helpers for E2E test setup.
 *
 * `makeAgent(browser.getInstance('agent1'))` wraps a WDIO browser so that
 * `window.__test` methods are accessible directly on it as awaited calls
 * (`agent.createProfile('A', 'B')`). Browser methods like `agent.execute`,
 * `agent.waitUntil` keep working. Errors surface as `<method> failed: <error>`.
 *
 * Methods returning DOM elements (homeLoaded, versionItem, …) don't serialize
 * cleanly across the bridge — call those via `agent.execute(...)` directly so
 * the element stays in the browser context.
 */
import { HomePage } from './pages/home';
import { NewGroupPage } from './pages/new-group';
import { NewMessagePage } from './pages/new-message';

type TestUtils = Window['__test'];

type Asyncified<T> = {
	[K in keyof T]: T[K] extends (...args: infer A) => infer R
		? (...args: A) => Promise<Awaited<R>>
		: never;
};

export type Agent = WebdriverIO.Browser &
	Asyncified<TestUtils> & {
		onHomePage(): HomePage;
		onNewMessagePage(): NewMessagePage;
		onNewGroupPage(): NewGroupPage;
	};

type Result = { ok: true; value: unknown } | { ok: false; error: string };

async function callTestUtil(
	b: WebdriverIO.Browser,
	method: string,
	args: unknown[],
): Promise<unknown> {
	const result = (await b.execute(
		async (m: string, a: unknown[]): Promise<Result> => {
			const fn = (
				window.__test as unknown as Record<string, (...a: unknown[]) => unknown>
			)[m];
			if (typeof fn !== 'function') {
				return { ok: false, error: `window.__test.${m} is not a function` };
			}
			try {
				const value = await Promise.resolve(fn(...a));
				return { ok: true, value };
			} catch (e) {
				return { ok: false, error: String(e) };
			}
		},
		method,
		args,
	)) as Result;
	if (!result.ok) throw new Error(`${method} failed: ${result.error}`);
	return result.value;
}

// Promise-protocol keys: if the proxy returns a function for these, the JS
// engine treats the agent as a thenable and tries to await it (e.g. when
// mocha returns it from a hook).
const PROMISE_KEYS = new Set(['then', 'catch', 'finally']);

export function makeAgent(b: WebdriverIO.Browser): Agent {
	let agent: Agent;
	const pageObjectFactories: Record<string, () => unknown> = {
		onHomePage: () => new HomePage(agent),
		onNewMessagePage: () => new NewMessagePage(agent),
		onNewGroupPage: () => new NewGroupPage(agent),
	};
	agent = new Proxy(b, {
		get(target, prop) {
			if (typeof prop === 'string' && prop in pageObjectFactories) {
				return pageObjectFactories[prop];
			}
			const value = (target as unknown as Record<string | symbol, unknown>)[
				prop
			];
			if (value !== undefined) {
				return typeof value === 'function' ? value.bind(target) : value;
			}
			if (typeof prop !== 'string' || PROMISE_KEYS.has(prop)) return undefined;
			return (...args: unknown[]) => callTestUtil(target, prop, args);
		},
	}) as Agent;
	return agent;
}

/** Wait for window.__test to be registered on a single agent. */
export async function waitForTestUtils(
	agent: WebdriverIO.Browser,
): Promise<void> {
	await agent.waitUntil(
		async () => agent.execute(() => typeof window.__test !== 'undefined'),
		{
			timeout: 30_000,
			interval: 500,
			timeoutMsg: 'window.__test not registered',
		},
	);
}

/** Build an agent by capability name and wait for window.__test to be ready. */
export async function setupAgent(agentName: string): Promise<Agent> {
	const agent = makeAgent(browser.getInstance(agentName));
	await waitForTestUtils(agent);
	return agent;
}

/** Exchange contact codes between two agents. */
export async function exchangeContacts(
	agent1: Agent,
	agent2: Agent,
): Promise<void> {
	await agent1.navigateToAddContact();
	await agent2.navigateToAddContact();
	const code1 = await agent1.getContactCode();
	const code2 = await agent2.getContactCode();
	if (!code1) throw new Error('agent1 contact code missing');
	if (!code2) throw new Error('agent2 contact code missing');
	await agent1.addContact(code2);
	await agent2.addContact(code1);
}

/**
 * Switch the agent's UI locale. setLocale triggers a full page reload at the
 * locale-prefixed URL, which wipes and re-registers `window.__test`. We
 * atomically delete the existing `__test` inside the same execute() block so
 * that waitForTestUtils blocks until the new page has re-registered, rather
 * than returning immediately against the stale (old-page) registry.
 */
export async function setLocale(agent: Agent, locale: string): Promise<void> {
	await agent.execute((loc: string) => {
		const setLocaleFn = window.__test.setLocale;
		delete (window as unknown as { __test?: unknown }).__test;
		setLocaleFn(loc);
	}, locale);
	await waitForTestUtils(agent);
}
