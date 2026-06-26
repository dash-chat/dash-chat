<script lang="ts">
	import type { FileAttachment, PhotoAttachment } from 'dash-chat-stores';
	import { inView } from '$lib/actions/in-view';
	import {
		acquireMediaUrl,
		cacheMediaUrl,
		cachedMediaUrl,
		invalidateMediaUrl,
		releaseMediaUrl,
	} from '$lib/utils/media-cache';
	import { m } from '$lib/paraglide/messages.js';
	import { Preloader } from 'konsta/svelte';
	import { mdiReload } from '@mdi/js';

	interface Props {
		item: PhotoAttachment | FileAttachment;
		alt: string;
		/** Forwarded to the inner <img> (e.g. object-fit / sizing classes). */
		imgClass?: string;
		/** Forwarded to the inner <img> (e.g. zoom transform-origin). */
		imgStyle?: string;
		/** Defer loading until near the viewport (grid cells); the lightbox loads eagerly. */
		lazy?: boolean;
		/** Notified whenever the load state changes, so a parent can decide what
		 * a click should do (open vs. retry). */
		onStatus?: (status: 'loading' | 'loaded' | 'error') => void;
	}

	let {
		item,
		alt,
		imgClass = '',
		imgStyle = '',
		lazy = false,
		onStatus,
	}: Props = $props();

	// Always render from a cached blob: URL. The bytes are fetched once (one IPC
	// round-trip) and reused on every later mount; we never point the <img> at
	// irohblob:// directly, which would re-read the blob a second time to fill
	// the cache. `src` is empty until the bytes resolve (set in the effect below).
	let src = $state('');
	let status = $state<'loading' | 'loaded' | 'error'>('loading');

	function load() {
		const hash = item.hash;
		const hit = cachedMediaUrl(hash);
		if (hit) {
			src = hit;
			return;
		}
		cacheMediaUrl(item)
			.then(url => {
				if (item.hash === hash) src = url;
			})
			.catch(() => {
				if (item.hash === hash) status = 'error';
			});
	}

	// Resolve from the cache whenever the item changes. A hit shows instantly; on
	// a miss, eager cells (e.g. the focused lightbox image) fetch immediately
	// while lazy cells wait for `inView`. Reading `lazy` re-runs this when a
	// lightbox slide becomes focused, so it loads even if it never neared the
	// viewport. Writes (not reads) src/status, so it can't loop on itself.
	$effect(() => {
		const hit = cachedMediaUrl(item.hash);
		src = hit ?? '';
		status = 'loading';
		if (!hit && !lazy) load();
	});

	// While the <img> points at a cached blob: URL, hold a reference so the cache
	// won't revoke it out from under a still-mounted image during eviction.
	$effect(() => {
		if (!src.startsWith('blob:')) return;
		const url = src;
		acquireMediaUrl(url);
		return () => releaseMediaUrl(url);
	});

	/** Re-attempt the download after a failed load (e.g. the blob hadn't synced
	 * yet). Drops any cached entry so the bytes are re-fetched from the store. */
	export function retry() {
		invalidateMediaUrl(item.hash);
		status = 'loading';
		src = '';
		load();
	}

	$effect(() => {
		onStatus?.(status);
	});

	$effect(() => {
		function onForceError(e: Event) {
			if ((e as CustomEvent<string>).detail === alt) status = 'error';
		}
		window.addEventListener('test-blob-force-error', onForceError);
		return () =>
			window.removeEventListener('test-blob-force-error', onForceError);
	});
</script>

{#if status === 'error'}
	<span
		class="absolute inset-0 flex cursor-pointer items-center justify-center border-none p-0 text-black/50 dark:text-white/60 {imgClass}"
		style={imgStyle}
		title={m.imageLoadFailedRetry()}
		data-testid="blob-image-retry"
	>
		<svg viewBox="0 0 24 24" width="28" height="28" aria-hidden="true">
			<path fill="currentColor" d={mdiReload} />
		</svg>
	</span>
{:else if src}
	<img
		{src}
		{alt}
		class={imgClass}
		style={imgStyle}
		data-testid="blob-image"
		onload={() => (status = 'loaded')}
		onerror={() => (status = 'error')}
	/>
	{#if status === 'loading'}
		<div
			class="pointer-events-none absolute inset-0 flex items-center justify-center"
			aria-busy="true"
			data-testid="blob-image-loading"
		>
			<Preloader class="w-6 h-6" />
		</div>
	{/if}
{:else}
	<div
		class="absolute inset-0 flex items-center justify-center"
		aria-busy="true"
		data-testid="blob-image-loading"
		use:inView={lazy ? { onEnter: load } : undefined}
	>
		<Preloader class="w-6 h-6" />
	</div>
{/if}
