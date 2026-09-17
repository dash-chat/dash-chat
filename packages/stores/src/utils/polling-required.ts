/// Stopgap: re-fetch subscribed logs on an interval as a safety net for
/// `p2panda://new-operation` events that don't reach this process. Concretely,
/// when an iOS push notification arrives in foreground, the NSE writes
/// operations to the shared `op_store` but the main app's notification channel
/// never fires for them — UI would otherwise stay stale until a manual reload.
/// Only iOS is affected; on other platforms the channel events arrive
/// reliably, so we skip the polling there.
/// Replace with cross-process change detection on `op_store` (SQLite WAL +
/// `PRAGMA data_version`) when that lands.
export function pollingRequired(): boolean {
	return (
		typeof navigator !== 'undefined' &&
		(/iPhone|iPad|iPod/i.test(navigator.userAgent) ||
			// iPadOS 13+ reports a Mac user agent in WKWebView; fall back to the
			// touch-points heuristic so iPad users still get the polling safety net.
			(/Macintosh/i.test(navigator.userAgent) && navigator.maxTouchPoints > 1))
	);
}

export const POLL_INTERVAL_MS = 1_000;

/// Same stopgap as [`pollingRequired`], for state that comes from a backend
/// projection query rather than from subscribed logs (e.g. the group chat list
/// and group members): the polled log reactives don't cover it, so poll `fetch`
/// on an interval and call `onChange` when its serialized result differs from
/// the last. No-op off iOS. Returns a teardown that stops the poll.
export function pollForChanges<T>(
	fetch: () => Promise<T>,
	onChange: () => void,
): () => void {
	if (!pollingRequired()) return () => {};
	let last: string | null = null;
	const interval = setInterval(async () => {
		try {
			const key = JSON.stringify(await fetch());
			if (key !== last) {
				last = key;
				onChange();
			}
		} catch {
			/* transient; the next tick retries */
		}
	}, POLL_INTERVAL_MS);
	return () => clearInterval(interval);
}
