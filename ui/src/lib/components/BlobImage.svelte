<script lang="ts">
	import type { FileAttachment, PhotoAttachment } from 'dash-chat-stores';
	import { mediaSrc } from '$lib/utils/media';
	import {
		blobStatus,
		blobToken,
		reportBlobStatus,
		retryBlob,
	} from '$lib/stores/blob-load-store.svelte';
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

	// The cache-busting token is shared per content hash, so a retry from any
	// surface (grid cell, lightbox stage, filmstrip thumb) re-fetches them all.
	const token = $derived(blobToken(item.hash));
	const src = $derived(
		token === 0 ? mediaSrc(item) : `${mediaSrc(item)}?t=${token}`,
	);

	// Once this instance has rendered the blob at the current token it pins to
	// `loaded` and ignores an `error` another surface reports for the same hash:
	// a transient failure on the eager lightbox image must not blank a grid
	// thumbnail that already loaded fine. A genuinely unfetchable blob still
	// shows the reload icon on every surface that never loaded, and a retry (new
	// token) drops the pin so each image re-decides its own status.
	let loaded = $state<{ hash: string; token: number } | undefined>();
	const status = $derived(
		loaded?.hash === item.hash && loaded.token === token
			? 'loaded'
			: blobStatus(item.hash),
	);

	/** Re-attempt the download with a fresh, cache-busting URL. Called by the
	 * parent when a missing image's placeholder is clicked. */
	export function retry() {
		retryBlob(item.hash);
	}

	$effect(() => {
		onStatus?.(status);
	});

	$effect(() => {
		function onForceError(e: Event) {
			// Simulates the blob becoming unfetchable everywhere, so drop this
			// instance's pin too — otherwise an already-loaded image would ignore it.
			if ((e as CustomEvent<string>).detail === alt) {
				loaded = undefined;
				reportBlobStatus(item.hash, 'error');
			}
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
		onload={() => {
			loaded = { hash: item.hash, token };
			reportBlobStatus(item.hash, 'loaded');
		}}
		onerror={() => reportBlobStatus(item.hash, 'error')}
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
