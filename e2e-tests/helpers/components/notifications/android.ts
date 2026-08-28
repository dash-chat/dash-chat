import { APP_PACKAGE, adbShell } from '../../../setup/platforms/android';
import { AppiumNotificationHelper } from './notification-helper';

/** The app name as the shade shows it on every notification of ours. */
const APP_NAME = 'Dash Chat';

/** Every title/text extra of this app's active notifications, parsed from
 * `dumpsys notification --noredact`. The shade's view tree is NOT a reliable
 * content source: OEM skins (MIUI) render only the title while a notification
 * sits collapsed, so bodies never appear to UiAutomator. dumpsys reads the
 * posted notification itself. */
function dashChatNotificationTexts(dump: string): string[] {
	return dump
		.split(/^\s*NotificationRecord\(/m)
		.slice(1)
		.filter(record => record.includes(`pkg=${APP_PACKAGE}`))
		.flatMap(record =>
			[
				...record.matchAll(
					/android\.(?:title|text|bigText)=(?:Spannable)?String \((.*)\)$/gm,
				),
			].map(m => m[1]),
		);
}

/** Android (UiAutomator2) notification observation: content is read from the
 * notification service (dumpsys), taps go through the shade UI. */
export class AndroidNotifications extends AppiumNotificationHelper {
	/** A shade element whose text contains `textIncludes`. */
	private elementFor(textIncludes: string) {
		const escaped = textIncludes.replace(/"/g, '\\"');
		return this.agent.$(`android=new UiSelector().textContains("${escaped}")`);
	}

	private udid(): string {
		const udid = this.agent.requestedCapabilities['appium:udid'];
		if (udid === undefined) {
			throw new Error('Android session is missing its appium:udid capability');
		}
		return udid as string;
	}

	private notificationTexts(): string[] {
		return dashChatNotificationTexts(
			adbShell(this.udid(), 'dumpsys notification --noredact'),
		);
	}

	/** A back-key press closes the shade (and is harmless when it is closed). */
	protected async dismissNotificationUi(): Promise<void> {
		await this.agent.back();
	}

	waitForNotification(textIncludes: string, timeout = 60_000): Promise<string> {
		return this.restoringWebviewOnFailure(async () => {
			await this.switchToNative();
			await this.agent.openNotifications();
			let texts: string[] = [];
			await this.agent.waitUntil(
				() => {
					texts = this.notificationTexts();
					return texts.some(t => t.includes(textIncludes));
				},
				{
					timeout,
					timeoutMsg: `No notification containing "${textIncludes}" arrived within ${timeout}ms`,
				},
			);
			return texts.join('\n');
		});
	}

	waitForAppNotification(timeout = 60_000): Promise<string> {
		return this.restoringWebviewOnFailure(async () => {
			await this.switchToNative();
			await this.agent.openNotifications();
			// Wait on the notification service, not on shade elements: MIUI
			// renders MessagingStyle notifications without any element matching
			// the app name, so a shade-based wait never fires for chat messages.
			let texts: string[] = [];
			await this.agent.waitUntil(
				() => {
					texts = this.notificationTexts();
					return texts.length > 0;
				},
				{
					timeout,
					timeoutMsg: `No ${APP_NAME} notification arrived within ${timeout}ms`,
				},
			);
			return texts.join('\n');
		});
	}

	/** Whether the app owns the resumed (foreground) activity, per adb. */
	private async appIsForeground(timeoutMs: number): Promise<boolean> {
		const deadline = Date.now() + timeoutMs;
		for (;;) {
			const resumed = adbShell(
				this.udid(),
				'dumpsys activity activities | grep -m1 -E "mResumedActivity|topResumedActivity" || true',
			);
			if (resumed.includes(APP_PACKAGE)) return true;
			if (Date.now() >= deadline) return false;
			await new Promise(resolve => setTimeout(resolve, 500));
		}
	}

	tapNotification(textIncludes: string): Promise<void> {
		return this.restoringWebviewOnFailure(async () => {
			// A shade tap that nothing handles (MIUI sometimes expands the entry
			// instead of firing its content intent) leaves the app backgrounded,
			// where the next webview context lookup can block chromedriver far
			// past any wdio timeout. Confirm via adb that the app actually came
			// to the foreground, retrying the tap, and fail fast otherwise.
			for (let attempt = 1; attempt <= 3; attempt++) {
				await this.elementFor(textIncludes).click();
				if (await this.appIsForeground(10_000)) return;
				await this.agent.openNotifications();
			}
			throw new Error(
				`tapped the notification containing "${textIncludes}" 3 times and ` +
					'the app never came to the foreground',
			);
		});
	}
}
