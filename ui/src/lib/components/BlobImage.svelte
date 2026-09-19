<script lang="ts">
	import { getContext, untrack } from 'svelte';
	import type {
		BlobStore,
		FileAttachment,
		PhotoAttachment,
	} from 'dash-chat-stores';
	import { formatFileSize, mediaSrc } from '$lib/utils/media';
	import { onScreen } from '$lib/utils/on-screen';
	import { useReactiveValue } from '$lib/stores/use-signal';
	import {
		acquireBlob,
		blobToken,
		releaseBlob,
		retryBlob,
	} from '$lib/stores/blob-load-store.svelte';
	import BlobProgressRing from '$lib/components/BlobProgressRing.svelte';
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
		/** Small surfaces (filmstrip thumbs, reply quotes): a 20px ring and no byte pill. */
		compact?: boolean;
	}

	let {
		item,
		alt,
		imgClass = '',
		imgStyle = '',
		lazy = false,
		compact = false,
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

	const blobStore: BlobStore = getContext('blob-store');
	// Only a surface in the viewport asks about its blob, so a long chat polls
	// for the attachments on screen and no others.
	let visible = $state(false);
	const download = $derived(
		visible ? useReactiveValue(blobStore.progress, item.hash) : undefined,
	);
	// Unknown until the first snapshot resolves, and a loaded image is never
	// covered: the ring only ever overlays a photo known to still be downloading.
	const downloading = $derived(
		$download !== undefined && !$download.complete && status !== 'loaded',
	);
	const stalled = $derived(downloading && $download?.stalled === true);

	// A fresh mount or a new blob re-attempts from scratch, which also self-heals
	// a blob that failed only because it hadn't synced yet. A retry (token bump)
	// re-fetches every surface via the changed `src`, but an already-loaded,
	// healthy surface keeps showing its image instead of flushing to the spinner —
	// only surfaces that still need the blob reset visibly.
	let previousHash = untrack(() => item.hash);
	$effect(() => {
		void token;
		const hash = item.hash;
		untrack(() => {
			const isNewBlob = hash !== previousHash;
			previousHash = hash;
			if (isNewBlob || status !== 'loaded') status = 'loading';
		});
	});

	// The scheme handler gives up on an <img> after 30s, which a slow download
	// can outlast; once the blob lands, load the image again without a tap.
	let reloadOnComplete = false;
	$effect(() => {
		if (status === 'error' && $download?.complete !== true)
			reloadOnComplete = true;
	});
	$effect(() => {
		if ($download?.complete !== true || !reloadOnComplete) return;
		reloadOnComplete = false;
		untrack(() => retryBlob(item.hash));
	});

	/** If this image is stalled or showing its reload placeholder, re-fetch the
	 * blob on every surface and report that the click was handled. Lets a parent
	 * decide a click means "retry" vs. its normal action without tracking load
	 * state itself. */
	export function retryIfErrored(): boolean {
		if (stalled) {
			blobStore.retry(item.hash);
			retryBlob(item.hash);
			return true;
		}
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

{#if status !== 'error'}
	<!-- Mounted while downloading too: its request is what asks the node to
	     fetch the blob now rather than on the background loop's next pass. -->
	<img
		{src}
		{alt}
		class={imgClass}
		style={imgStyle}
		loading={lazy ? 'lazy' : 'eager'}
		data-testid="blob-image"
		onload={() => (status = 'loaded')}
		onerror={() => (status = 'error')}
		{@attach onScreen(v => (visible = v))}
	/>
{/if}
{#if downloading}
	<div
		class="pointer-events-none absolute inset-0 flex items-center justify-center text-black/60 dark:text-white/70 {imgClass}"
		style={imgStyle}
		data-testid="blob-image-downloading"
		{@attach onScreen(v => (visible = v))}
	>
		<BlobProgressRing
			bytes={$download?.bytes ?? 0}
			total={item.size}
			{stalled}
			size={compact ? 20 : 40}
		/>
		{#if !compact}
			<span
				class="absolute start-1 top-1 rounded-full bg-black/50 px-1.5 py-0.5 text-[10px] leading-tight text-white"
				data-testid="blob-progress-bytes"
				>{formatFileSize($download?.bytes ?? 0)} / {formatFileSize(
					item.size,
				)}</span
			>
		{/if}
	</div>
{:else if status === 'error'}
	<span
		class="absolute inset-0 flex cursor-pointer items-center justify-center border-none p-0 text-black/50 dark:text-white/60 {imgClass}"
		style={imgStyle}
		title={m.imageLoadFailedRetry()}
		data-testid="blob-image-retry"
		{@attach onScreen(v => (visible = v))}
	>
		<svg viewBox="0 0 24 24" width="28" height="28" aria-hidden="true">
			<path fill="currentColor" d={mdiReload} />
		</svg>
	</span>
{:else if status === 'loading'}
	<div
		class="pointer-events-none absolute inset-0 flex items-center justify-center"
		aria-busy="true"
		data-testid="blob-image-loading"
	>
		<Preloader class="w-6 h-6" />
	</div>
{/if}
