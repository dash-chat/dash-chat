<script lang="ts">
	import { untrack } from 'svelte';
	import PhotoSwipe, { type SlideData } from 'photoswipe';
	import 'photoswipe/style.css';
	import { Preloader } from 'konsta/svelte';
	import { mdiReload } from '@mdi/js';
	import type { Hash, PhotoAttachment } from 'dash-chat-stores';
	import { mediaSrc } from '$lib/utils/media';
	import {
		acquireBlob,
		blobToken,
		releaseBlob,
		retryBlob,
	} from '$lib/stores/blob-load-store.svelte';
	import { m } from '$lib/paraglide/messages.js';

	const MAX_ZOOM = 5;
	const DOUBLE_TAP_ZOOM = 2;

	interface Props {
		photos: PhotoAttachment[];
		/** Index of the currently shown photo; updated as the user swipes. */
		index?: number;
		/** Whether the shown photo is zoomed in. */
		zoomed?: boolean;
		/** A touch tap on the stage. */
		onTap: () => void;
		/** A mouse click beside the photo. */
		onBackgroundClick: () => void;
		class?: string;
	}

	let {
		photos,
		index = $bindable(0),
		zoomed = $bindable(false),
		onTap,
		onBackgroundClick,
		class: className = '',
	}: Props = $props();

	let hostEl: HTMLElement | undefined = $state();
	let viewer: PhotoSwipe | undefined = $state.raw();
	let slides: SlideData[] = [];
	let rightToLeft = false;
	let shownPhotoLoad = $state<'loading' | 'loaded' | 'error'>('loading');
	let forcedErrorHashes = $state<Hash[]>([]);

	const sources = $derived(
		photos.map(photo => {
			const token = blobToken(photo.hash);
			return token === 0 ? mediaSrc(photo) : `${mediaSrc(photo)}?t=${token}`;
		}),
	);
	const shownPhoto = $derived(photos[index]);
	const errored = $derived(
		shownPhotoLoad === 'error' || forcedErrorHashes.includes(shownPhoto.hash),
	);

	// PhotoSwipe always pages left to right; in RTL the photos are handed to it
	// in reverse so the first one sits rightmost, as the rest of the app lays
	// them out. The mapping is its own inverse.
	function viewerIndex(photoIndex: number): number {
		return rightToLeft ? photos.length - 1 - photoIndex : photoIndex;
	}

	function syncShownPhotoLoad(shown: PhotoSwipe) {
		const state = shown.currSlide?.content.state;
		shownPhotoLoad =
			state === 'error' ? 'error' : state === 'loaded' ? 'loaded' : 'loading';
	}

	export function zoomOut() {
		const slide = viewer?.currSlide;
		if (!slide || !viewer) return;
		slide.zoomTo(
			slide.zoomLevels.initial,
			undefined,
			viewer.options.zoomAnimationDuration,
		);
	}

	function retryShownPhoto() {
		retryBlob(shownPhoto.hash);
	}

	$effect(() => {
		if (!hostEl) return;
		rightToLeft = getComputedStyle(hostEl).direction === 'rtl';
		const currentSources = untrack(() => sources);
		const inPhotoOrder = photos.map((photo, i) => ({
			src: currentSources[i],
			width: photo.width,
			height: photo.height,
			alt: photo.name,
		}));
		slides = rightToLeft ? inPhotoOrder.reverse() : inPhotoOrder;
		const created = new PhotoSwipe({
			dataSource: slides,
			index: untrack(() => viewerIndex(index)),
			appendToEl: hostEl,
			bgOpacity: 0,
			showHideAnimationType: 'none',
			loop: false,
			secondaryZoomLevel: zoom => zoom.initial * DOUBLE_TAP_ZOOM,
			maxZoomLevel: zoom => zoom.initial * MAX_ZOOM,
			errorMsg: '',
			close: false,
			zoom: false,
			counter: false,
			arrowPrev: false,
			arrowNext: false,
			escKey: false,
			arrowKeys: false,
			trapFocus: false,
			returnFocus: false,
			closeOnVerticalDrag: false,
			pinchToClose: false,
			clickToCloseNonZoomable: false,
			tapAction: () => onTap(),
			// The default also closes the viewer, which only the lightbox may do.
			imageClickAction: 'zoom',
			bgClickAction: () => onBackgroundClick(),
		});
		created.on('change', () => {
			index = viewerIndex(created.currIndex);
			zoomed = false;
			syncShownPhotoLoad(created);
		});
		created.on('zoomPanUpdate', ({ slide }) => {
			if (slide !== created.currSlide) return;
			zoomed = slide.currZoomLevel > slide.zoomLevels.initial;
		});
		created.on('loadComplete', () => syncShownPhotoLoad(created));
		created.on('loadError', () => syncShownPhotoLoad(created));
		// PhotoSwipe applies a pinch from its own frame loop, a few frames behind
		// the fingers; applying it as each touch arrives keeps the photo under them.
		created.on('pointerMove', () => {
			if (created.gestures.isZooming) created.gestures.zoomLevels.change();
		});
		created.init();
		syncShownPhotoLoad(created);
		viewer = created;
		return () => created.destroy();
	});

	$effect(() => {
		if (viewer && viewer.currIndex !== viewerIndex(index)) {
			viewer.goTo(viewerIndex(index));
		}
	});

	// A retry from any surface bumps the blob's token, which changes its source.
	$effect(() => {
		const current = sources;
		const shown = viewer;
		if (!shown) return;
		current.forEach((src, i) => {
			const slideData = slides[viewerIndex(i)];
			if (slideData.src === src) return;
			slideData.src = src;
			const hash = photos[i].hash;
			untrack(() => {
				forcedErrorHashes = forcedErrorHashes.filter(h => h !== hash);
			});
			shown.refreshSlideContent(viewerIndex(i));
			syncShownPhotoLoad(shown);
		});
	});

	$effect(() => {
		const hashes = photos.map(photo => photo.hash);
		untrack(() => hashes.forEach(acquireBlob));
		return () => hashes.forEach(releaseBlob);
	});

	$effect(() => {
		function onForceError(e: Event) {
			const alt = (e as CustomEvent<string>).detail;
			const forced = photos.filter(p => p.name === alt).map(p => p.hash);
			forcedErrorHashes = [...forcedErrorHashes, ...forced];
		}
		window.addEventListener('test-blob-force-error', onForceError);
		return () =>
			window.removeEventListener('test-blob-force-error', onForceError);
	});
</script>

<div class={className}>
	<!-- PhotoSwipe mounts a fixed, top-layer root; the stacking context keeps it
	     under the lightbox's own bars. -->
	<div
		bind:this={hostEl}
		class="isolate h-full [--pswp-root-z-index:0] [&_.pswp__top-bar]:hidden"
		data-testid="lightbox-stage"
	></div>
	{#if errored}
		<button
			type="button"
			class="lightbox-image absolute inset-0 flex items-center justify-center bg-black text-white/60"
			title={m.imageLoadFailedRetry()}
			data-testid="blob-image-retry"
			onclick={retryShownPhoto}
		>
			<svg viewBox="0 0 24 24" width="28" height="28" aria-hidden="true">
				<path fill="currentColor" d={mdiReload} />
			</svg>
		</button>
	{:else if shownPhotoLoad === 'loading'}
		<div
			class="pointer-events-none absolute inset-0 flex items-center justify-center"
			aria-busy="true"
			data-testid="blob-image-loading"
		>
			<Preloader class="h-6 w-6" />
		</div>
	{/if}
</div>
