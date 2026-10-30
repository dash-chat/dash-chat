import { afterNavigate, goto } from '$app/navigation';

/**
 * Bookkeeping for how many in-app history entries are behind the current one.
 * Pure (no browser APIs) so the random-walk property test can drive it; the
 * app uses the singleton below, fed from `afterNavigate`.
 */
export function createBackTracker() {
	let depth = 0;
	let replacePending = false;

	return {
		get canGoBack() {
			return depth > 0;
		},
		/** Flag the next push-like navigation as a replace (no depth change). */
		markReplace() {
			replacePending = true;
		},
		clearReplace() {
			replacePending = false;
		},
		handleNavigation(type: 'enter' | 'popstate' | 'push', delta?: number) {
			if (type === 'enter') {
				depth = 0;
				replacePending = false;
			} else if (type === 'popstate') {
				depth = Math.max(0, depth + (delta ?? -1));
			} else if (replacePending) {
				replacePending = false;
			} else {
				depth += 1;
			}
		},
	};
}

const tracker = createBackTracker();

/**
 * Feed every navigation into the back tracker. Call once during root layout
 * initialization (it registers an `afterNavigate` lifecycle callback).
 */
export function trackHistoryDepth() {
	afterNavigate(navigation => {
		const type =
			navigation.type === 'enter' || navigation.type === 'popstate'
				? navigation.type
				: 'push';
		tracker.handleNavigation(type, navigation.delta);
	});
}

/**
 * `goto` with `replaceState: true` that keeps the back tracker accurate. Use
 * this instead of passing `replaceState` to `goto` directly.
 */
export async function replaceGoto(
	url: string,
	opts: Omit<NonNullable<Parameters<typeof goto>[1]>, 'replaceState'> = {},
): Promise<void> {
	tracker.markReplace();
	try {
		await goto(url, { ...opts, replaceState: true });
	} finally {
		tracker.clearReplace();
	}
}

/**
 * Unified Back: pop the history stack so Back always returns to the page the
 * user actually came from. When there is no in-app history behind this page
 * (fresh launch, deep link, reload), navigate to `fallback` instead —
 * replacing rather than pushing, so pressing Back never grows the stack.
 */
export function goBack(fallback: string) {
	if (tracker.canGoBack) {
		window.history.back();
	} else {
		void replaceGoto(fallback);
	}
}
