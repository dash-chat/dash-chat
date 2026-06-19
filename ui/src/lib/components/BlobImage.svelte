<script lang="ts">
	import type { FileAttachment, Photo } from 'dash-chat-stores';
	import { mediaSrc } from '$lib/utils/media';
	import { m } from '$lib/paraglide/messages.js';
	import { Preloader } from 'konsta/svelte';
	import { mdiReload } from '@mdi/js';

	interface Props {
		item: Photo | FileAttachment;
		alt: string;
		/** Forwarded to the inner <img> (e.g. object-fit / sizing classes). */
		imgClass?: string;
		/** Forwarded to the inner <img> (e.g. zoom transform-origin). */
		imgStyle?: string;
	}

	let { item, alt, imgClass = '', imgStyle = '' }: Props = $props();

	let status = $state<'loading' | 'loaded' | 'error'>('loading');
	// 0 keeps the first request query-free so the cached 200 is reused; a retry
	// uses Date.now() so every attempt is a fresh URL even across app restarts.
	let buster = $state(0);
	const src = $derived(
		buster === 0 ? mediaSrc(item) : `${mediaSrc(item)}?t=${buster}`,
	);

	function retry() {
		status = 'loading';
		buster = Date.now();
	}

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
	<button
		type="button"
		class="blob-image-retry {imgClass}"
		style={imgStyle}
		aria-label={m.imageLoadFailedRetry()}
		data-testid="blob-image-retry"
		onclick={retry}
	>
		<svg viewBox="0 0 24 24" width="28" height="28" aria-hidden="true">
			<path fill="currentColor" d={mdiReload} />
		</svg>
	</button>
{:else}
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

	:global(.dark) .blob-image-retry {
		color: rgba(255, 255, 255, 0.6);
	}
</style>
