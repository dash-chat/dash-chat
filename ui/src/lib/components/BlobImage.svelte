<script lang="ts">
	import { untrack } from 'svelte';
	import type { FileAttachment, PhotoAttachment } from 'dash-chat-stores';
	import { mediaSrc } from '$lib/utils/media';
	import {
		acquireBlob,
		blobToken,
		releaseBlob,
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
	}

	let {
		item,
		alt,
		imgClass = '',
		imgStyle = '',
		lazy = false,
	}: Props = $props();

	// Load status is this element's own — each <img> fetches independently, so a
	// failure here never blanks another surface of the same blob. Only the
	// cache-busting token is shared per hash: a retry from any surface bumps it,
	// and every mounted image re-attempts because its `src` changes.
	let status = $state<'loading' | 'loaded' | 'error'>('loading');
	const token = $derived(blobToken(item.hash));
	const src = $derived(
		token === 0 ? mediaSrc(item) : `${mediaSrc(item)}?t=${token}`,
	);

	// A fresh mount (or a bumped token) re-attempts from scratch, which also
	// self-heals a blob that failed only because it hadn't synced yet.
	$effect(() => {
		void item.hash;
		void token;
		status = 'loading';
	});

	/** If this image is showing its reload placeholder, re-fetch the blob on every
	 * surface and report that the click was handled. Lets a parent decide a click
	 * means "retry" vs. its normal action without tracking load state itself. */
	export function retryIfErrored(): boolean {
		if (status !== 'error') return false;
		retryBlob(item.hash);
		return true;
	}

	// Bound the shared map to blobs on screen: keep the entry alive only while
	// mounted. `hash` is captured so teardown releases what it acquired even if
	// `item` changes.
	$effect(() => {
		const hash = item.hash;
		// untrack: the ref bookkeeping reads and writes the store entry, which must
		// not make this effect depend on (and re-run from) its own mutation.
		untrack(() => acquireBlob(hash));
		return () => releaseBlob(hash);
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
{/if}
