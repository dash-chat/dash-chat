<script lang="ts">
	import { watcher } from 'signalium';
	import { hashValue } from 'signalium/utils';
	import { setContext, type Snippet } from 'svelte';

	import { SIGNAL_CACHE_KEY, type SignalCache } from './signal-cache.svelte';

	let { children }: { children: Snippet } = $props();

	type Entry = { unsub: () => void };
	const cache = new Map<Function, Map<number, Entry>>();

	const scope: SignalCache = {
		keepAlive(fn, args) {
			let inner = cache.get(fn);
			if (!inner) {
				inner = new Map();
				cache.set(fn, inner);
			}
			const argsKey = hashValue(args);
			if (inner.has(argsKey)) return;

			const w = watcher(() => {
				const rp = fn(...args);
				(rp as any)['_version']?.['value'];
				return undefined;
			});
			inner.set(argsKey, { unsub: w.addListener(() => {}) });
		},
	};

	setContext(SIGNAL_CACHE_KEY, scope);

	$effect(() => () => {
		for (const inner of cache.values()) {
			for (const { unsub } of inner.values()) {
				unsub();
			}
		}
		cache.clear();
	});
</script>

{@render children()}
