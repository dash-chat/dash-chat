import { APP_PACKAGE, adbShell } from '../../../setup/platforms/android';
import {
	AppiumNotificationHelper,
	type DeliveredNotification,
} from './notification-helper';

/** The app name as the shade shows it on every notification of ours. */
const APP_NAME = 'Dash Chat';

const TEXT_EXTRA =
	/android\.(title|text|bigText)=(?:Spannable)?String \((.*)\)$/gm;

/** What a just-opened shade gets to put its entries in the view tree. The
 * notification is already confirmed posted when this runs, so this covers
 * UiAutomator's lag behind the open, not the notification's arrival. */
const SHADE_RENDER_TIMEOUT = 5_000;

/** The shade's clear-all affordance, by the ids the stock UI and the OEM
 * skins give it. */
const CLEAR_ALL =
	'new UiSelector().resourceIdMatches(".*(clear_all|dismiss_text).*")';

/** This app's active notifications, parsed from `dumpsys notification
 * --noredact`. A record with no title is a group summary — the plugin posts
 * one per group with nothing but an icon — and is left out, since it shows
 * the user no content of its own. The shade's view tree is NOT a reliable
 * content source: OEM skins (MIUI) render only the title while a notification
 * sits collapsed, so bodies never appear to UiAutomator. dumpsys reads the
 * posted notification itself. */
function dashChatNotifications(dump: string): DeliveredNotification[] {
	const notifications: DeliveredNotification[] = [];
	const records = dump
		.split(/^\s*NotificationRecord\(/m)
		.slice(1)
		.filter(record => record.includes(`pkg=${APP_PACKAGE}`));
	for (const record of records) {
		const extras = [...record.matchAll(TEXT_EXTRA)];
		const title = extras.find(([, key]) => key === 'title')?.[2];
		if (title === undefined) continue;
		notifications.push({ title, texts: extras.map(([, , value]) => value) });
	}
	return notifications;
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
		return this.notifications().flatMap(n => n.texts);
	}

	private notifications(): DeliveredNotification[] {
		return dashChatNotifications(
			adbShell(this.udid(), 'dumpsys notification --noredact'),
		);
	}

	delivered(): Promise<DeliveredNotification[]> {
		return Promise.resolve(this.notifications());
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

	/** Bring the entry containing `textIncludes` into the shade's view tree.
	 * Android bundles an app's notifications under one group entry, and while
	 * that is collapsed the individual ones are not in the tree at all — so a
	 * device holding two of ours renders neither by title until the bundle is
	 * expanded. */
	private async revealEntry(textIncludes: string): Promise<void> {
		const expanders = [
			'new UiSelector().resourceId("android:id/expand_button")',
			`new UiSelector().textContains("${APP_NAME}")`,
		];
		await this.agent.waitUntil(
			async () =>
				(await this.elementFor(textIncludes).isExisting()) ||
				(await this.anyExists(expanders)),
			{
				timeout: SHADE_RENDER_TIMEOUT,
				timeoutMsg: `the shade rendered neither "${textIncludes}" nor a way to expand it`,
			},
		);
		if (await this.elementFor(textIncludes).isExisting()) return;
		for (const selector of expanders) {
			const expander = this.agent.$(`android=${selector}`);
			if (!(await expander.isExisting())) continue;
			await expander.click();
			if (await this.elementFor(textIncludes).isExisting()) return;
		}
		throw new Error(
			`the shade has no entry containing "${textIncludes}"; it renders: ` +
				(await this.shadeTexts()),
		);
	}

	/** Whether the shade is rendering any of `selectors`. */
	private async anyExists(selectors: string[]): Promise<boolean> {
		for (const selector of selectors) {
			if (await this.agent.$(`android=${selector}`).isExisting()) return true;
		}
		return false;
	}

	/** Clear the shade. Tapping clear-all is the only route the OS offers:
	 * the notification plugin registers no command for it and adb exposes
	 * none, so this also takes down other apps' notifications — which on a
	 * test device is what is wanted anyway. */
	async clear(): Promise<void> {
		if (this.notifications().length === 0) return;
		await this.restoringWebviewOnFailure(async () => {
			await this.switchToNative();
			await this.agent.openNotifications();
			const clearAll = this.agent.$(`android=${CLEAR_ALL}`);
			await clearAll.waitForExist({
				timeout: SHADE_RENDER_TIMEOUT,
				timeoutMsg: `the shade has no clear-all button; it renders: ${await this.shadeTexts()}`,
			});
			await clearAll.click();
			await this.agent.waitUntil(() => this.notifications().length === 0, {
				timeout: SHADE_RENDER_TIMEOUT,
				timeoutMsg:
					'the device still holds notifications of ours after clearing the shade',
			});
			await this.restoreWebview();
		});
	}

	/** Every text the shade is rendering, for a failure to name. */
	private async shadeTexts(): Promise<string> {
		const texts = await this.agent
			.$$('android=new UiSelector().textMatches(".+")')
			.map(el => el.getText());
		return texts.join(' | ');
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
			// Tapping is a native gesture on the shade, whether or not a wait
			// opened it first: from the webview context an Android selector is
			// not even a valid strategy.
			await this.switchToNative();
			await this.agent.openNotifications();
			// A shade tap that nothing handles (MIUI sometimes expands the entry
			// instead of firing its content intent) leaves the app backgrounded,
			// where the next webview context lookup can block chromedriver far
			// past any wdio timeout. Confirm via adb that the app actually came
			// to the foreground, retrying the tap, and fail fast otherwise.
			for (let attempt = 1; attempt <= 3; attempt++) {
				await this.revealEntry(textIncludes);
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
