/**
 * Cross-platform push-notification observation for real-device e2e tests: iOS
 * and Android read notifications differently (SpringBoard cells vs the
 * notification shade), so a factory picks the implementation by platform.
 */
export interface NotificationHelper {
	/** Wait for a delivered notification whose text contains `textIncludes`;
	 * returns its full text (title + body). */
	waitForNotification(textIncludes: string, timeout?: number): Promise<string>;
	/** Wait for any notification from this app and return its full text — for
	 * asserting *what* was delivered. Matching on the expected content instead
	 * would make a wrong body (the generic "You have a new message" fallback)
	 * indistinguishable from no delivery at all: both just time out. */
	waitForAppNotification(timeout?: number): Promise<string>;
	/** Tap the matching notification. */
	tapNotification(textIncludes: string): Promise<void>;
	/** Return to the app's webview context. */
	returnToApp(): Promise<void>;
	/** Best-effort: close the notification UI and return to the webview, for
	 * cleanup after a failure between native-context steps. */
	recover(): Promise<void>;
}

/** Shared Appium plumbing for switching between the app's WebView and the
 * NATIVE_APP context. */
export abstract class AppiumNotificationHelper implements NotificationHelper {
	protected webviewContext: string | undefined;

	constructor(protected agent: WebdriverIO.Browser) {}

	/** Switch to NATIVE_APP, remembering the current WebView context first. */
	protected async switchToNative(): Promise<void> {
		const current = await this.agent.getContext();
		if (typeof current === 'string' && current.startsWith('WEBVIEW')) {
			this.webviewContext = current;
		}
		await this.agent.switchContext('NATIVE_APP');
	}

	/** Switch back to the app's WebView, waiting for it to (re)appear. */
	protected async switchToWebview(): Promise<void> {
		await this.agent.waitUntil(
			async () => {
				const contexts =
					(await this.agent.getContexts()) as unknown as string[];
				const target =
					this.webviewContext !== undefined &&
					contexts.includes(this.webviewContext)
						? this.webviewContext
						: contexts.find(
								c => typeof c === 'string' && c.startsWith('WEBVIEW'),
							);
				if (target === undefined) return false;
				await this.agent.switchContext(target);
				return true;
			},
			{
				timeout: 30_000,
				interval: 500,
				timeoutMsg: 'No WEBVIEW context to return to',
			},
		);
	}

	async returnToApp(): Promise<void> {
		await this.switchToWebview();
	}

	/** Bring the app to the front. The recovery paths below run with the app
	 * wherever the failing test left it, which for a push test is usually
	 * backgrounded or quit — and a backgrounded app publishes no WEBVIEW
	 * context, so switching back to one is impossible until it is resumed.
	 * `activateApp` is idempotent: it launches a stopped app and merely
	 * foregrounds a running one. */
	private async foregroundApp(): Promise<void> {
		const caps = this.agent.requestedCapabilities as Record<string, unknown>;
		const appId = caps['appium:appPackage'] ?? caps['appium:bundleId'];
		if (typeof appId !== 'string') return;
		await this.agent.activateApp(appId);
	}

	/** Close the notification UI, resume the app and return to its webview, so
	 * the session is driveable again for whatever runs next. */
	private async restoreWebview(): Promise<void> {
		await this.dismissNotificationUi();
		await this.foregroundApp();
		await this.switchToWebview();
	}

	/** Best-effort recovery for spec-level cleanup: close the notification UI
	 * and return to the webview. For when a test fails *between* native-context
	 * helper calls (e.g. a content assertion after a successful wait), which
	 * [`restoringWebviewOnFailure`] cannot see. */
	async recover(): Promise<void> {
		try {
			await this.restoreWebview();
		} catch (err) {
			// best-effort: the original test failure is what should surface, but
			// a session left in NATIVE_APP fails every later test on an
			// unrelated-looking "invalid selector", so name the real reason.
			console.warn(
				`[notifications] could not restore the webview: ${String(err)}`,
			);
		}
	}

	/** Close the platform's notification UI (shade / Notification Center). */
	protected abstract dismissNotificationUi(): Promise<void>;

	/** Run `fn` (which works in the native context); when it fails, close the
	 * notification UI and restore the webview context before rethrowing, so a
	 * timed-out wait doesn't strand the session in NATIVE_APP and cascade
	 * "invalid selector" failures into every following test. */
	protected async restoringWebviewOnFailure<T>(
		fn: () => Promise<T>,
	): Promise<T> {
		try {
			return await fn();
		} catch (err) {
			try {
				await this.restoreWebview();
			} catch (restoreErr) {
				// surface the original failure, not the recovery's — but name the
				// recovery's too, since it is what turns one failure into a run of
				// "invalid selector" cascades.
				console.warn(
					`[notifications] could not restore the webview: ${String(restoreErr)}`,
				);
			}
			throw err;
		}
	}

	abstract waitForNotification(
		textIncludes: string,
		timeout?: number,
	): Promise<string>;
	abstract waitForAppNotification(timeout?: number): Promise<string>;
	abstract tapNotification(textIncludes: string): Promise<void>;
}
