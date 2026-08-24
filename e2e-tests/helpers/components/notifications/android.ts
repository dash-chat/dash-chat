import { AppiumNotificationHelper } from './notification-helper';

/** The app name as the shade shows it on every notification of ours. */
const APP_NAME = 'Dash Chat';

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

	async waitForAppNotification(timeout = 60_000): Promise<string> {
		await this.switchToNative();
		await this.agent.openNotifications();
		// Every shade notification carries the posting app's name; its title
		// and body are the standard android:id/title and android:id/text views
		// alongside it, so read those rather than the app label.
		const appLabel = this.elementFor(APP_NAME);
		await appLabel.waitForExist({
			timeout,
			timeoutMsg: `No ${APP_NAME} notification arrived within ${timeout}ms`,
		});
		const parts = await Promise.all(
			['android:id/title', 'android:id/text'].map(async id => {
				const el = this.agent.$(`android=new UiSelector().resourceId("${id}")`);
				return (await el.isExisting()) ? await el.getText() : '';
			}),
		);
		return parts.filter(p => p !== '').join(' ');
	}

	async tapNotification(textIncludes: string): Promise<void> {
		await this.elementFor(textIncludes).click();
	}
}
