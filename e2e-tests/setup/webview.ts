/** Session-level webview helpers shared by the agent factory and the
 *  platform modules. */
import { APP_PACKAGE } from './platforms/android';
import type { AgentPlatformName } from './test-env';

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

/** Wait for window.__test to be registered on a single agent. */
export async function waitForTestUtils(
	agent: WebdriverIO.Browser,
): Promise<void> {
	// A cold start on a slow physical phone can take over 30s from webview
	// attach to the page's JS running, so give it well beyond the default.
	await agent.waitUntil(
		async () => agent.execute(() => typeof window.__test !== 'undefined'),
		{
			timeout: 120_000,
			interval: 500,
			timeoutMsg: 'window.__test not registered',
		},
	);
}
