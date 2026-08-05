/**
 * Cross-platform push-notification observation for real-device e2e tests. iOS
 * and Android both drive their device through Appium but read notifications
 * differently (SpringBoard cells vs the notification shade), so the spec talks
 * to this interface and a factory picks the implementation by platform.
 */
export interface NotificationHelper {
	/** Send the app to the background so an incoming push is handled by the OS
	 * (the NSE on iOS) and posted as a notification, not delivered to the
	 * foreground webview. Also records the webview context to return to. */
	background(): Promise<void>;
	/** Wait for a delivered notification whose text contains `textIncludes` and
	 * return its full text (title + body). Opens the notification surface as
	 * needed. The default timeout allows for FCM→APNs delivery + the op fetch. */
	waitForNotification(textIncludes: string, timeout?: number): Promise<string>;
	/** Tap the matching notification, foregrounding the app to its route. */
	tapNotification(textIncludes: string): Promise<void>;
	/** Return to the app's webview context so page objects work again. */
	returnToApp(): Promise<void>;
}

/**
 * Shared Appium plumbing: switching between the app's WebView context (where
 * page objects operate) and the NATIVE_APP context (where the OS notification
 * UI lives). Subclasses implement the platform-specific gestures/queries.
 */
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

	/** Switch back to the app's WebView, waiting for it to (re)appear — after a
	 * background/foreground round-trip the context can briefly vanish or be
	 * renamed. */
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

	abstract background(): Promise<void>;
	abstract waitForNotification(
		textIncludes: string,
		timeout?: number,
	): Promise<string>;
	abstract tapNotification(textIncludes: string): Promise<void>;
}
