import { AppiumNotificationHelper } from './notification-helper';

/** The app name as SpringBoard shows it on every notification of ours. */
const APP_NAME = 'Dash Chat';

/** iOS (XCUITest) notification observation via SpringBoard Notification Center. */
export class IosNotifications extends AppiumNotificationHelper {
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

	async waitForNotification(
		textIncludes: string,
		timeout = 60_000,
	): Promise<string> {
		await this.switchToNative();
		await this.openNotificationCenter();
		const cell = this.cellFor(textIncludes);
		await cell.waitForExist({
			timeout,
			timeoutMsg: `No notification containing "${textIncludes}" arrived within ${timeout}ms`,
		});
		return (await cell.getAttribute('label')) ?? '';
	}

	async waitForAppNotification(timeout = 60_000): Promise<string> {
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
	}

	async tapNotification(textIncludes: string): Promise<void> {
		await this.cellFor(textIncludes).click();
	}
}
