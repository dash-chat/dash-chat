import { afterNavigate, goto } from '$app/navigation';

export type NavigationKind = 'enter' | 'popstate' | 'push' | 'replace';

/**
 * Bookkeeping for how many in-app history entries are behind the current one.
 * Pure (no browser APIs) so the random-walk property test can drive it; the
 * app uses the singleton below, fed from `afterNavigate`.
 */
export function createBackTracker() {
	let depth = 0;

	return {
		get canGoBack() {
			return depth > 0;
		},
		handleNavigation(kind: NavigationKind, delta?: number) {
			if (kind === 'enter') {
				depth = 0;
			} else if (kind === 'popstate') {
				depth = Math.max(0, depth + (delta ?? -1));
			} else if (kind === 'push') {
				depth += 1;
			}
		},
	};
}

const tracker = createBackTracker();

// `afterNavigate` cannot tell a replace from a push, so `replaceGoto` raises
// this while its navigation is in flight.
let replacing = false;

function classify(type: string): NavigationKind {
	if (type === 'enter' || type === 'popstate') return type;
	return replacing ? 'replace' : 'push';
}

/**
 * Feed every navigation into the back tracker. Call once during root layout
 * initialization (it registers an `afterNavigate` lifecycle callback).
 */
export function trackHistoryDepth() {
	afterNavigate(navigation => {
		tracker.handleNavigation(classify(navigation.type), navigation.delta);
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
	replacing = true;
	try {
		await goto(url, { ...opts, replaceState: true });
	} finally {
		replacing = false;
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
