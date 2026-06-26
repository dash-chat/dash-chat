<script lang="ts">
	import type { FileAttachment, PhotoAttachment } from 'dash-chat-stores';
	import { mediaSrc } from '$lib/utils/media';
	import {
		cacheMediaUrl,
		cachedMediaUrl,
		invalidateMediaUrl,
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

	let status = $state<'loading' | 'loaded' | 'error'>('loading');
	// buster busts a failed retry's URL so it never reuses a cached failure.
	let buster = $state(0);

	// Prefer a cached blob: URL (set once the bytes were fetched on an earlier
	// render); otherwise point the <img> at irohblob:// so its native lazy
	// loading defers the store read until it nears the viewport.
	function resolveSrc(): string {
		const cached = cachedMediaUrl(item.hash);
		if (cached) return cached;
		return buster === 0 ? mediaSrc(item) : `${mediaSrc(item)}?t=${buster}`;
	}

	let src = $state(resolveSrc());

	// Recompute the src only on item/retry changes; the background cache fill
	// (in onLoaded) is intentionally non-reactive so it never swaps a live src.
	// Track both explicitly: a cache hit makes resolveSrc return before reading
	// them, which would otherwise drop the dependency and miss a later retry.
	$effect(() => {
		void item;
		void buster;
		src = resolveSrc();
		status = 'loading';
	});

	function onLoaded() {
		status = 'loaded';
		// Keep the bytes as a blob: URL so the next mount (re-opening the chat,
		// scrolling back) skips the store read. No-op once cached.
		void cacheMediaUrl(item).catch(() => {});
	}

	/** Re-attempt the download after a failed load (e.g. the blob hadn't synced
	 * yet). Drops any cached entry so the bytes are re-fetched from the store. */
	export function retry() {
		invalidateMediaUrl(item.hash);
		buster = Date.now();
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
{:else}
	<img
		{src}
		{alt}
		class={imgClass}
		style={imgStyle}
		loading={lazy ? 'lazy' : 'eager'}
		data-testid="blob-image"
		onload={onLoaded}
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
{/if}
