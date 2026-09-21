import type { BlobState, BlobStore, Hash } from 'dash-chat-stores';

import { useReactiveValue } from './use-signal';

/** The download state of a blob, subscribed to (and so polled) only while
 * `visible()` holds, and unknown otherwise. Call once at component init; the
 * subscription follows `hash` and `visible` from there. */
export function useBlobProgress(
	blobStore: BlobStore,
	hash: () => Hash,
	visible: () => boolean,
): { readonly current: BlobState | undefined } {
	let current = $state<BlobState | undefined>();
	$effect(() => {
		if (!visible()) return;
		const progress = useReactiveValue(blobStore.progress, hash());
		const unsubscribe = progress.subscribe(state => (current = state));
		return () => {
			unsubscribe();
			current = undefined;
		};
	});
	return {
		get current() {
			return current;
		},
	};
}
