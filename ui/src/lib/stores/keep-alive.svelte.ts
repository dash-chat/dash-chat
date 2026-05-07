import { type Readable } from 'svelte/store';

/**
 * Keep a list of Readable stores subscribed for the lifetime of the
 * surrounding component / effect. Useful in layouts that want to hold
 * signalium-backed reactive promises warm across child route remounts —
 * without an active subscriber, signalium drops the cached value and the
 * child flashes blank while it re-resolves.
 *
 * Pass a getter so reactive store references (e.g. `$derived` rebuilt on
 * route param changes) are tracked and resubscribed.
 */
export function keepAlive(stores: () => Readable<unknown>[]): void {
	$effect(() => {
		const unsubs = stores().map(s => s.subscribe(() => {}));
		return () => unsubs.forEach(u => u());
	});
}
