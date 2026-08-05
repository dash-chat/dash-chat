import { AppiumNotificationHelper } from './notification-helper';

/**
 * iOS (XCUITest) notification observation via SpringBoard. The top-edge swipe
 * and the `XCUIElementTypeCell … label CONTAINS` predicate target iOS 17+
 * Notification Center and are the parts most likely to need per-version tuning.
 */
export class IosNotifications extends AppiumNotificationHelper {
	async background(): Promise<void> {
		await this.switchToNative();
		await this.agent.execute('mobile: pressButton', { name: 'home' });
	}

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

	/** A notification cell whose combined label (title + body + app + time)
	 * contains `textIncludes`. */
	private cellFor(textIncludes: string) {
		const escaped = textIncludes.replace(/"/g, '\\"');
		return this.agent.$(
			`-ios predicate string:type == "XCUIElementTypeCell" AND label CONTAINS "${escaped}"`,
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
