<script lang="ts">
	import { watcher } from 'signalium';
	import { hashValue } from 'signalium/utils';
	import { setContext, type Snippet } from 'svelte';

	import {
		KEEP_ALIVE_SCOPE_KEY,
		type KeepAliveScope,
		trackRpVersion,
	} from './keep-alive.svelte';

	let { children }: { children: Snippet } = $props();

	type Entry = { unsub: () => void };
	const cache = new Map<Function, Map<number, Entry>>();

	const scope: KeepAliveScope = {
		keepAlive(fn, args) {
			let inner = cache.get(fn);
			if (!inner) {
				inner = new Map();
				cache.set(fn, inner);
			}
			const argsKey = hashValue(args);
			if (inner.has(argsKey)) return;

			const w = watcher(() => {
				trackRpVersion(fn(...args));
				return undefined;
			});
			inner.set(argsKey, { unsub: w.addListener(() => {}) });
		},
	};

	setContext(KEEP_ALIVE_SCOPE_KEY, scope);

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
