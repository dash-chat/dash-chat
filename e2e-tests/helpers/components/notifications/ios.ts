import { AppiumNotificationHelper } from './notification-helper';

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
	 * lock-screen banner or Notification Center cell. */
	private cellFor(textIncludes: string) {
		const escaped = textIncludes.replace(/"/g, '\\"');
		return this.agent.$(
			`-ios predicate string:label CONTAINS "${escaped}" OR value CONTAINS "${escaped}"`,
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

	async tapNotification(textIncludes: string): Promise<void> {
		await this.cellFor(textIncludes).click();
	}
}
