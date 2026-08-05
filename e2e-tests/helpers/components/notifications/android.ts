import { AppiumNotificationHelper } from './notification-helper';

/** Android (UiAutomator2) notification observation via the notification shade. */
export class AndroidNotifications extends AppiumNotificationHelper {
	/** A shade element whose text contains `textIncludes`. */
	private elementFor(textIncludes: string) {
		const escaped = textIncludes.replace(/"/g, '\\"');
		return this.agent.$(`android=new UiSelector().textContains("${escaped}")`);
	}

	async waitForNotification(
		textIncludes: string,
		timeout = 60_000,
	): Promise<string> {
		await this.switchToNative();
		await this.agent.openNotifications();
		const el = this.elementFor(textIncludes);
		await el.waitForExist({
			timeout,
			timeoutMsg: `No notification containing "${textIncludes}" arrived within ${timeout}ms`,
		});
		return (await el.getText()) ?? '';
	}

	async tapNotification(textIncludes: string): Promise<void> {
		await this.elementFor(textIncludes).click();
	}
}
