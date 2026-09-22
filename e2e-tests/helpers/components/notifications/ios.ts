import {
	AppiumNotificationHelper,
	type DeliveredNotification,
} from './notification-helper';

/** Notification Center belongs to SpringBoard, and WebDriverAgent only sees
 * the elements of the app it is pointed at. */
const SPRINGBOARD = 'com.apple.springboard';

/** Our notifications as Notification Center lists them: one `ListCell` each,
 * labelled "<APP>, <when>, <title>, <body>". Every cell has a
 * `NotificationShortLookView` twin, and the app's own switcher card carries
 * the app name too, so matching on the name alone finds those as well. */
const OUR_CELLS = 'name == "ListCell" AND label BEGINSWITH[c] "Dash Chat,"';

/** A collapsed stack of several of our notifications, which says so after the
 * app name and exposes only the newest. */
const STACK = `${OUR_CELLS} AND label CONTAINS ", Grouped, "`;

/** Stacks to expand before giving up on the rest. */
const MAX_STACKS = 10;

/** What Notification Center gets to open, or to render a cell, after a
 * gesture. */
const RENDER_TIMEOUT = 5_000;

/** What an opened notification gets to bring the app in front. */
const OPEN_TIMEOUT = 10_000;

/** A notification arriving outside Notification Center shows as a banner at
 * the top of the screen, which takes a pull from the top edge for itself. */
const BANNER = 'name == "NotificationShortLookView"';

/** What a banner gets to leave on its own: about 8s, measured. */
const BANNER_TIMEOUT = 15_000;

/** Pulls to try before giving up: another banner can land just as one left. */
const PULL_ATTEMPTS = 3;

/** Parse a cell's label. Only the first separators are structural: a body of
 * its own may contain commas, so it is whatever follows them. */
function parseCell(label: string): DeliveredNotification | null {
	const parts = label.split(', ');
	if (parts[1] === 'Grouped') parts.splice(1, 1);
	if (parts.length < 3) return null;
	const title = parts[2];
	const body = parts.slice(3).join(', ');
	return { title, texts: body === '' ? [title] : [title, body] };
}

/** Appium's app state for an app that owns the screen. */
const FOREGROUND = 4;

/** iOS (XCUITest) notification observation via SpringBoard Notification Center.
 *
 * Notification Center is the lock screen reached without unlocking, and
 * behaves like it: the device reports itself locked while it is showing, and a
 * tap on a notification only nudges it aside, where a swipe to the right opens
 * it. */
export class IosNotifications extends AppiumNotificationHelper {
	readonly readingResumesApp = true;

	protected async switchToNative(): Promise<void> {
		await super.switchToNative();
		await this.agent.updateSettings({ defaultActiveApplication: SPRINGBOARD });
	}

	protected async switchToWebview(): Promise<void> {
		await this.agent.updateSettings({ defaultActiveApplication: 'auto' });
		await super.switchToWebview();
	}

	/** Pull Notification Center down from the top edge of the screen, unless it
	 * is already showing: a second pull on it scrolls it instead. */
	private async openNotificationCenter(): Promise<void> {
		const { width, height } = await this.agent.getWindowSize();
		const x = Math.round(width / 2);
		for (let attempt = 0; attempt < PULL_ATTEMPTS; attempt++) {
			if (await this.agent.isLocked()) return;
			// Outside Notification Center, every short-look view is a banner.
			await this.agent.$(`-ios predicate string:${BANNER}`).waitForExist({
				reverse: true,
				timeout: BANNER_TIMEOUT,
				timeoutMsg: 'A notification banner never left the top of the screen',
			});
			await this.swipe(x, 2, x, Math.round(height * 0.7));
			const opened = await this.agent
				.waitUntil(() => this.agent.isLocked(), { timeout: RENDER_TIMEOUT })
				.then(
					() => true,
					() => false,
				);
			if (opened) return;
		}
		throw new Error('Notification Center did not open');
	}

	/** Swipe up from the bottom edge, which closes Notification Center onto
	 * whatever was under it. With it closed that is the home gesture, which
	 * would send the app away. */
	protected async dismissNotificationUi(): Promise<void> {
		if (!(await this.agent.isLocked())) return;
		const { width, height } = await this.agent.getWindowSize();
		const x = Math.round(width / 2);
		await this.swipe(x, height - 2, x, Math.round(height * 0.3));
	}

	private async swipe(
		fromX: number,
		fromY: number,
		toX: number,
		toY: number,
	): Promise<void> {
		await this.agent.performActions([
			{
				type: 'pointer',
				id: 'finger1',
				parameters: { pointerType: 'touch' },
				actions: [
					{ type: 'pointerMove', duration: 0, x: fromX, y: fromY },
					{ type: 'pointerDown', button: 0 },
					{ type: 'pointerMove', duration: 600, x: toX, y: toY },
					{ type: 'pointerUp', button: 0 },
				],
			},
		]);
		await this.agent.releaseActions();
	}

	/** Our cell whose label contains `textIncludes`, case-insensitively:
	 * SpringBoard upper-cases the app name ("DASH CHAT"). */
	private cellFor(textIncludes: string) {
		const escaped = textIncludes.replace(/"/g, '\\"');
		return this.agent.$(
			`-ios predicate string:${OUR_CELLS} AND label CONTAINS[c] "${escaped}"`,
		);
	}

	/** Expand every stack, so each notification in it is a cell of its own. A
	 * tap expands a stack, where on a single notification it only nudges it
	 * aside. */
	private async expandStacks(): Promise<void> {
		const stacks = async () =>
			(await this.agent.$$(`-ios predicate string:${STACK}`)).length;
		for (let expanded = 0; expanded < MAX_STACKS; expanded++) {
			const before = await stacks();
			if (before === 0) return;
			const stack = this.agent.$(`-ios predicate string:${STACK}`);
			const { x, y } = await stack.getLocation();
			const { width, height } = await stack.getSize();
			await this.agent.execute('mobile: tap', {
				x: Math.round(x + width / 2),
				y: Math.round(y + height / 2),
			});
			await this.agent.waitUntil(async () => (await stacks()) < before, {
				timeout: RENDER_TIMEOUT,
				timeoutMsg: 'A stack of notifications did not expand',
			});
		}
	}

	/** The cells Notification Center is showing. Expects it open and the
	 * driver in the native context, which [`readingDelivered`] arranges. */
	private async readCells(): Promise<DeliveredNotification[]> {
		const labels = await this.cellLabels();
		return labels
			.map(parseCell)
			.filter((n): n is DeliveredNotification => n !== null);
	}

	private async cellLabels(): Promise<string[]> {
		await this.expandStacks();
		return this.agent
			.$$(`-ios predicate string:${OUR_CELLS}`)
			.map(async cell => (await cell.getAttribute('label')) ?? '');
	}

	/** Wait for our cell containing `textIncludes`, expanding whatever stack it
	 * may have landed in. */
	private async waitForCell(
		textIncludes: string,
		timeout: number,
		timeoutMsg: string,
	) {
		const cell = this.cellFor(textIncludes);
		await this.agent.waitUntil(
			async () => {
				await this.expandStacks();
				return await cell.isExisting();
			},
			{ timeout, timeoutMsg },
		);
		return cell;
	}

	/** Whether the app owns the screen, so that a read knows whether to put it
	 * back there afterwards. Its state reads foreground even with Notification
	 * Center over it, so that is ruled out separately. */
	private async isFrontmost(): Promise<boolean> {
		const appId = this.appId();
		if (appId === undefined) return true;
		if ((await this.agent.queryAppState(appId)) !== FOREGROUND) return false;
		return !(await this.agent.isLocked());
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

	/** Through the app itself: Notification Center on the lock screen has no
	 * clear-all, only a swipe per notification. */
	async clear(): Promise<void> {
		await this.restoreWebview();
		await this.agent.execute(() => window.__test.clearNotifications());
	}

	waitForNotification(textIncludes: string, timeout = 60_000): Promise<string> {
		return this.restoringWebviewOnFailure(async () => {
			await this.switchToNative();
			await this.openNotificationCenter();
			const cell = await this.waitForCell(
				textIncludes,
				timeout,
				`No notification containing "${textIncludes}" arrived within ${timeout}ms`,
			);
			return (await cell.getAttribute('label')) ?? '';
		});
	}

	waitForAppNotification(timeout = 60_000): Promise<string> {
		return this.restoringWebviewOnFailure(async () => {
			await this.switchToNative();
			await this.openNotificationCenter();
			await this.agent.$(`-ios predicate string:${OUR_CELLS}`).waitForExist({
				timeout,
				timeoutMsg: `No notification of ours arrived within ${timeout}ms`,
			});
			// Every one, not just the newest: a caller asserting on content sees
			// what else was there when its assertion fails.
			return (await this.cellLabels()).join('\n');
		});
	}

	tapNotification(textIncludes: string): Promise<void> {
		return this.restoringWebviewOnFailure(async () => {
			await this.switchToNative();
			await this.openNotificationCenter();
			const cell = await this.waitForCell(
				textIncludes,
				RENDER_TIMEOUT,
				`No notification containing "${textIncludes}" to open`,
			);
			const { x, y } = await cell.getLocation();
			const { width, height } = await cell.getSize();
			const rowY = Math.round(y + height / 2);
			await this.agent.execute('mobile: dragFromToForDuration', {
				duration: 0.3,
				fromX: Math.round(x + 20),
				fromY: rowY,
				toX: Math.round(x + width),
				toY: rowY,
			});
			await this.agent.waitUntil(() => this.isFrontmost(), {
				timeout: OPEN_TIMEOUT,
				timeoutMsg: `Opening the "${textIncludes}" notification did not bring the app in front`,
			});
		});
	}
}
