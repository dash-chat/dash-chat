<script lang="ts">
	import { getContext, untrack } from 'svelte';
	import type {
		BlobStore,
		FileAttachment,
		PhotoAttachment,
	} from 'dash-chat-stores';
	import { formatFileSize, mediaSrc } from '$lib/utils/media';
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
		/** Small surfaces (filmstrip thumbs): a 20px ring and no byte pill. */
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

	const blobStore: BlobStore = getContext('blob-store');
	const download = $derived(useReactiveValue(blobStore.progress, item.hash));
	// Until the first snapshot resolves the download state is unknown; showing
	// the ring then would flash it on every already-local photo.
	const known = $derived($download !== undefined);
	const downloading = $derived(known && $download?.complete !== true);
	const stalled = $derived($download?.stalled === true);
	const bytes = $derived($download?.bytes ?? 0);

	// Load status is this element's own — each <img> fetches independently, so a
	// failure here never blanks another surface of the same blob. Only the
	// cache-busting token is shared per hash: a retry from any surface bumps it,
	// and every mounted image re-attempts because its `src` changes.
	let status = $state<'loading' | 'loaded' | 'error'>('loading');
	const token = $derived(blobToken(item.hash));
	const src = $derived(
		token === 0 ? mediaSrc(item) : `${mediaSrc(item)}?t=${token}`,
	);

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

	/** If this image is stalled or errored, retry and report that the click was
	 * handled, so a parent can tell "retry" from its normal click action. */
	export function retryIfErrored(): boolean {
		if (stalled) {
			void blobStore.retry(item.hash);
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

{#if downloading}
	<div
		class="absolute inset-0 flex items-center justify-center text-black/60 dark:text-white/70 {imgClass}"
		style={imgStyle}
		data-testid="blob-image-downloading"
	>
		<BlobProgressRing
			{bytes}
			total={item.size}
			{stalled}
			size={compact ? 20 : 40}
		/>
		{#if !compact}
			<span
				class="absolute start-1 top-1 rounded-full bg-black/50 px-1.5 py-0.5 text-[10px] leading-tight text-white"
				data-testid="blob-progress-bytes"
				>{formatFileSize(bytes)} / {formatFileSize(item.size)}</span
			>
		{/if}
	</div>
{:else if status === 'error'}
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
