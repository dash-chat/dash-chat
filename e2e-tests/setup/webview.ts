/** Session-level helpers shared by the agent factory and the platform
 *  modules. */
import { APP_PACKAGE } from './platforms/android';
import type { AgentPlatformName } from './test-env';

const webviewPending = new WeakSet<WebdriverIO.Browser>();

/** Have the session go back to the app's webview the next time it drives it:
 *  a suspended iOS app publishes no webview context to switch to until it is
 *  on screen again. */
export function returnToWebviewOnNextUse(agent: WebdriverIO.Browser): void {
	webviewPending.add(agent);
}

/** Make the session's first webview command after [`returnToWebviewOnNextUse`]
 *  go back to the app's webview before it runs. Native commands — element
 *  lookups by predicate, class chain or accessibility id (the harness never
 *  looks up native elements by css or xpath), `mobile:` scripts — go through
 *  as they are. */
export function returnToWebviewBeforeDrivingIt(
	agent: WebdriverIO.Browser,
	platform: AgentPlatformName,
): void {
	const returnIfPending = async () => {
		if (!webviewPending.delete(agent)) return;
		if (!(await isOnWebview(agent))) await switchToWebview(agent, platform);
	};
	for (const command of ['findElement', 'findElements'] as const) {
		agent.overwriteCommand(
			// @ts-expect-error The typings only accept webdriverio's own commands,
			// but a WebDriver protocol command overwrites the same way.
			command,
			async (orig: (using: string, value: string) => unknown, using, value) => {
				if (using === 'css selector' || using === 'xpath')
					await returnIfPending();
				return await orig(using, value);
			},
		);
	}
	for (const command of ['executeScript', 'executeAsyncScript'] as const) {
		agent.overwriteCommand(
			// @ts-expect-error As above.
			command,
			async (
				orig: (script: string, args: unknown[]) => unknown,
				script,
				args,
			) => {
				if (!script.startsWith('mobile:')) await returnIfPending();
				return await orig(script, args);
			},
		);
	}
	agent.overwriteCommand(
		// @ts-expect-error As above.
		'getUrl',
		async (orig: () => unknown) => {
			await returnIfPending();
			return await orig();
		},
	);
}

/** Attach to the app's webview context, which a relaunch drops out of.
 *
 *  On Android the app's context must be matched by name: other apps' webviews
 *  can be listed too (a backgrounded Chrome appears as WEBVIEW_chrome, and it
 *  can be the only one listed while the app is still starting). iOS names
 *  WKWebView contexts WEBVIEW_<id> with no package, but the app's webview is
 *  the only one there. */
export async function switchToWebview(
	agent: WebdriverIO.Browser,
	platform: AgentPlatformName,
): Promise<void> {
	const isAppWebview = (id: string) =>
		platform === 'ios'
			? id.startsWith('WEBVIEW')
			: id === `WEBVIEW_${APP_PACKAGE}`;
	let webview: string | undefined;
	await agent.waitUntil(
		async () => {
			const contexts = await agent.getContexts();
			webview = contexts
				.map(context => (typeof context === 'string' ? context : context.id))
				.find(isAppWebview);
			return webview !== undefined;
		},
		{ timeoutMsg: 'no app WEBVIEW context after relaunch' },
	);
	await agent.switchContext(webview!);
}

/** Whether the session is driving one of the app's webviews rather than the
 *  native UI. */
export async function isOnWebview(
	agent: WebdriverIO.Browser,
): Promise<boolean> {
	const context = await agent.getContext();
	const contextId = typeof context === 'string' ? context : context.id;
	return contextId.startsWith('WEBVIEW');
}

/** Wait for window.__test to be registered on a single agent. */
export async function waitForTestUtils(
	agent: WebdriverIO.Browser,
): Promise<void> {
	// A cold start on a slow physical phone can take over 30s from webview
	// attach to the page's JS running, so give it well beyond the default.
	// The desktop WebDriver reads execute results back from a window global, which
	// the navigation away from about:blank wipes, stalling for the script timeout.
	await agent.waitUntil(
		async () =>
			(await agent.getUrl()) !== 'about:blank' &&
			(await agent.execute(() => typeof window.__test !== 'undefined')),
		{
			timeout: 120_000,
			interval: 500,
			timeoutMsg: 'window.__test not registered',
		},
	);
}

/** The device serial this Appium session was launched against. */
export function deviceUdid(b: WebdriverIO.Browser): string {
	const udid = b.requestedCapabilities['appium:udid'];
	if (udid === undefined) {
		throw new Error('Appium session is missing its appium:udid capability');
	}
	return udid;
}
