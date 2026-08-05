import { AppiumNotificationHelper } from './notification-helper';

/** Android KEYCODE_HOME — sends the app to the launcher (backgrounds it). */
const KEYCODE_HOME = 3;

/**
 * Android (UiAutomator2) notification observation via the notification shade
 * (`openNotifications`) and a `textContains` selector over its TextViews.
 */
export class AndroidNotifications extends AppiumNotificationHelper {
	async background(): Promise<void> {
		await this.switchToNative();
		await this.agent.pressKeyCode(KEYCODE_HOME);
	}

	/** A shade element whose text contains `textIncludes` (the notification
	 * body TextView). */
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
