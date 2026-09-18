import {
	AppiumNotificationHelper,
	type DeliveredNotification,
} from './notification-helper';

/** The app name as SpringBoard shows it on every notification of ours. */
const APP_NAME = 'Dash Chat';

/** Notification Center's clear affordance. Tapping it may reveal a second
 * button to confirm with, which carries the same word. */
const CLEAR = `-ios predicate string:type == "XCUIElementTypeButton" AND label CONTAINS[c] "Clear"`;

/** What Notification Center gets to render its cells after the pull. */
const RENDER_TIMEOUT = 5_000;

/** SpringBoard gives a cell one flat label, "<app>, <when>, <title>, <body>".
 * Only the first three separators are structural: a body of its own may
 * contain commas, so it is whatever follows them. */
function parseCell(label: string): DeliveredNotification | null {
	const parts = label.split(', ');
	if (parts.length < 3) return null;
	const title = parts[2];
	const body = parts.slice(3).join(', ');
	return { title, texts: body === '' ? [title] : [title, body] };
}

/** Appium's app state for an app that owns the screen. */
const FOREGROUND = 4;

/** iOS (XCUITest) notification observation via SpringBoard Notification Center. */
export class IosNotifications extends AppiumNotificationHelper {
	readonly readingResumesApp = true;

	/** Pull Notification Center down from the top edge of the screen. */
	private async openNotificationCenter(): Promise<void> {
		const { width, height } = await this.agent.getWindowSize();
		const x = Math.round(width / 2);
		await this.agent.performActions([
			{
				type: 'pointer',
				id: 'finger1',
				parameters: { pointerType: 'touch' },
				actions: [
					{ type: 'pointerMove', duration: 0, x, y: 2 },
					{ type: 'pointerDown', button: 0 },
					{
						type: 'pointerMove',
						duration: 600,
						x,
						y: Math.round(height * 0.7),
					},
					{ type: 'pointerUp', button: 0 },
				],
			},
		]);
		await this.agent.releaseActions();
	}

	/** Any SpringBoard element whose label or value contains `textIncludes` — the
	 * lock-screen banner or Notification Center cell. `CONTAINS[c]` is
	 * case-insensitive: SpringBoard renders the source app name upper-cased
	 * ("DASH CHAT"), so a case-sensitive match on "Dash Chat" never fires. */
	private cellFor(textIncludes: string) {
		const escaped = textIncludes.replace(/"/g, '\\"');
		return this.agent.$(
			`-ios predicate string:label CONTAINS[c] "${escaped}" OR value CONTAINS[c] "${escaped}"`,
		);
	}

	/** Swipe up from the bottom edge — the home gesture, which also closes
	 * Notification Center (and is harmless when it is closed). */
	protected async dismissNotificationUi(): Promise<void> {
		const { width, height } = await this.agent.getWindowSize();
		const x = Math.round(width / 2);
		await this.agent.performActions([
			{
				type: 'pointer',
				id: 'finger1',
				parameters: { pointerType: 'touch' },
				actions: [
					{ type: 'pointerMove', duration: 0, x, y: height - 2 },
					{ type: 'pointerDown', button: 0 },
					{
						type: 'pointerMove',
						duration: 600,
						x,
						y: Math.round(height * 0.3),
					},
					{ type: 'pointerUp', button: 0 },
				],
			},
		]);
		await this.agent.releaseActions();
	}

	/** The cells Notification Center is showing. Expects it open and the
	 * driver in the native context, which [`readingDelivered`] arranges. */
	private async readCells(): Promise<DeliveredNotification[]> {
		const labels = await this.agent
			.$$(`-ios predicate string:label CONTAINS[c] "${APP_NAME}"`)
			.map(async cell => (await cell.getAttribute('label')) ?? '');
		return labels
			.map(parseCell)
			.filter((n): n is DeliveredNotification => n !== null);
	}

	/** Whether the app owns the screen, so that a read knows whether to put it
	 * back there afterwards. */
	private async isFrontmost(): Promise<boolean> {
		const appId = this.appId();
		if (appId === undefined) return true;
		return (await this.agent.queryAppState(appId)) === FOREGROUND;
	}

	/** Opens Notification Center once, reads inside it for as long as `fn`
	 * runs, and closes it at the end — so a poll loop costs one resume, and
	 * with it one clearing of the route the app resumes onto, rather than one
	 * per read. An app that was not on screen is left off it: resuming it
	 * would undo the very state the read is there to check. */
	readingDelivered<T>(
		fn: (read: () => Promise<DeliveredNotification[]>) => Promise<T>,
	): Promise<T> {
		return this.restoringWebviewOnFailure(async () => {
			const frontmost = await this.isFrontmost();
			await this.switchToNative();
			await this.openNotificationCenter();
			const result = await fn(() => this.readCells());
			if (frontmost) await this.restoreWebview();
			else await this.dismissNotificationUi();
			return result;
		});
	}

	delivered(): Promise<DeliveredNotification[]> {
		return this.readingDelivered(read => read());
	}

	async clear(): Promise<void> {
		await this.restoringWebviewOnFailure(async () => {
			await this.switchToNative();
			await this.openNotificationCenter();
			// Tapping the first one may only reveal the one that confirms.
			for (let tap = 0; tap < 2; tap++) {
				const clear = this.agent.$(CLEAR);
				if (!(await clear.isExisting())) break;
				await clear.click();
			}
			await this.agent.waitUntil(
				async () => (await this.readCells()).length === 0,
				{
					timeout: RENDER_TIMEOUT,
					timeoutMsg:
						'Notification Center still holds notifications of ours after clearing it',
				},
			);
			await this.restoreWebview();
		});
	}

	waitForNotification(textIncludes: string, timeout = 60_000): Promise<string> {
		return this.restoringWebviewOnFailure(async () => {
			await this.switchToNative();
			await this.openNotificationCenter();
			const cell = this.cellFor(textIncludes);
			await cell.waitForExist({
				timeout,
				timeoutMsg: `No notification containing "${textIncludes}" arrived within ${timeout}ms`,
			});
			return (await cell.getAttribute('label')) ?? '';
		});
	}

	waitForAppNotification(timeout = 60_000): Promise<string> {
		return this.restoringWebviewOnFailure(async () => {
			await this.switchToNative();
			await this.openNotificationCenter();
			// SpringBoard labels a cell "<app>, <when>, <title>, <body>", so the app
			// name is the one thing every notification of ours carries.
			const cell = this.cellFor(APP_NAME);
			try {
				await cell.waitForExist({ timeout });
			} catch {
				// Nothing of ours matched by label. Before concluding no push arrived,
				// show what Notification Center actually holds: the label format
				// differs across iOS versions, and a notification can be present under
				// a shape this predicate does not match.
				const source = await this.agent.getPageSource();
				const cells = source
					.split('\n')
					.filter(line => /XCUIElementTypeCell|label="/.test(line))
					.slice(0, 40)
					.join('\n');
				throw new Error(
					`No ${APP_NAME} notification matched within ${timeout}ms. ` +
						`Notification Center contents:\n${cells || '(no cells)'}`,
				);
			}
			// Notification Center lists newest first, and the first match is what
			// `$` returns; older ones from earlier steps (a contact request, say) sit
			// below it. Return every label so the caller sees them all if the
			// content assertion fails.
			const labels = await this.agent
				.$$(`-ios predicate string:label CONTAINS[c] "${APP_NAME}"`)
				.map(async c => (await c.getAttribute('label')) ?? '');
			return labels.join('\n');
		});
	}

	tapNotification(textIncludes: string): Promise<void> {
		return this.restoringWebviewOnFailure(async () => {
			// Tapping is a SpringBoard gesture, whether or not a wait opened
			// Notification Center first: from the webview context its cells are
			// not reachable at all.
			await this.switchToNative();
			// Only when nothing of ours is on screen already: a second pull on an
			// open Notification Center scrolls it instead.
			if (!(await this.cellFor(APP_NAME).isExisting())) {
				await this.openNotificationCenter();
			}
			await this.cellFor(textIncludes).click();
		});
	}
}
