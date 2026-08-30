let resumedAt = $state(Date.now());

if (typeof window !== 'undefined') {
	document.addEventListener('visibilitychange', () => {
		if (document.visibilityState === 'visible') resumedAt = Date.now();
	});
}

/** When the page last became visible, as epoch milliseconds.
 *
 *  Anything measured before this describes a connection the user was not in a
 *  position to see: Android denies network to backgrounded apps, so polls made
 *  while away fail regardless of what is waiting on the other side, and
 *  reporting them on return is how a verdict about a connection nobody had
 *  reaches the screen. */
export const renderingResumedAt = {
	get value() {
		return resumedAt;
	},
};
