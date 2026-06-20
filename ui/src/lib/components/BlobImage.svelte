<script lang="ts">
	import type { FileAttachment, PhotoAttachment } from 'dash-chat-stores';
	import { mediaSrc } from '$lib/utils/media';
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
	// buster===0 keeps the first load query-free (cacheable); a retry uses Date.now() so it never reuses a cached failure, even across restarts.
	let buster = $state(0);
	const src = $derived(
		buster === 0 ? mediaSrc(item) : `${mediaSrc(item)}?t=${buster}`,
	);

	/** Re-attempt the download with a fresh, cache-busting URL. Called by the
	 * parent when a missing image's placeholder is clicked. */
	export function retry() {
		status = 'loading';
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
		class="blob-image-retry {imgClass}"
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
		<span
			class="blob-image-loading"
			aria-busy="true"
			data-testid="blob-image-loading"
		>
			<Preloader class="w-6 h-6" />
		</span>
	{/if}
{/if}

<style>
	.blob-image-loading {
		position: absolute;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		background: rgba(128, 128, 128, 0.08);
		pointer-events: none;
	}

	.blob-image-retry {
		position: absolute;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		min-width: 64px;
		min-height: 64px;
		border: none;
		padding: 0;
		background: rgba(128, 128, 128, 0.12);
		color: rgba(0, 0, 0, 0.5);
		cursor: pointer;
	}

	:global(.dark) .blob-image-loading {
		background: rgba(255, 255, 255, 0.06);
	}

	:global(.dark) .blob-image-retry {
		color: rgba(255, 255, 255, 0.6);
	}
</style>
