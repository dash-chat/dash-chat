<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { mdiArrowBack } from '$lib/utils/icon';
	import {
		mdiChevronLeft,
		mdiChevronRight,
		mdiClose,
		mdiTrayArrowDown,
	} from '@mdi/js';
	import { darkOverlay } from '$lib/actions/dark-overlay';
	import type { PhotoAttachment } from 'dash-chat-stores';
	import { savePhoto, loadMediaBytes } from '$lib/utils/media';
	import { shareFile } from '$lib/utils/files';
	import { isMobile, isAndroid } from '$lib/utils/environment';
	import { showToast } from '$lib/utils/toasts';
	import BlobImage from '$lib/components/BlobImage.svelte';
	import IconButton from '$lib/components/IconButton.svelte';
	import ShareButton from '$lib/components/ShareButton.svelte';
	import ImageCarousel from '$lib/components/ImageCarousel.svelte';
	import LightboxThumbnailStrip from './LightboxThumbnailStrip.svelte';
	import MessageTimestamp from './MessageTimestamp.svelte';

	interface Props {
		photos: PhotoAttachment[];
		/** Index of the currently shown photo; updated as the user navigates. */
		index?: number;
		senderName: string;
		timestamp: number;
		onClose: () => void;
	}

	let {
		photos,
		index = $bindable(0),
		senderName,
		timestamp,
		onClose,
	}: Props = $props();

	const photo = $derived(photos[index]);

	let rootEl: HTMLElement | undefined = $state();

	// Desktop double-click zoom; on mobile, tapping the photo instead toggles
	// `immersive` (chrome hidden). Both hide the surrounding UI via `chromeHidden`.
	let zoomed = $state(false);
	let immersive = $state(false);
	const chromeHidden = $derived(isMobile ? immersive : zoomed);
	let originX = $state(50);
	let originY = $state(50);

	let blobImages = $state<Array<{ retry: () => void } | undefined>>([]);
	let statuses = $state<Record<number, 'loading' | 'loaded' | 'error'>>({});
	const imgStatus = $derived(statuses[index] ?? 'loading');

	function select(i: number) {
		index = Math.max(0, Math.min(photos.length - 1, i));
	}

	async function handleSave() {
		try {
			if (await savePhoto(photo)) showToast(m.mediaSaved());
		} catch (e) {
			showToast(m.errorUnexpected(), 'unexpected', e);
			console.error(e);
		}
	}

	async function handleShare() {
		try {
			const data = await loadMediaBytes(photo);
			await shareFile(data, photo.name, photo.mime_type);
		} catch (e) {
			showToast(m.errorUnexpected(), 'unexpected', e);
			console.error(e);
		}
	}

	// Reset zoom when switching photos.
	$effect(() => {
		void index;
		zoomed = false;
	});

	$effect(() => {
		rootEl
			?.querySelector<HTMLButtonElement>('[data-testid="lightbox-close"]')
			?.focus();
	});

	function updateOrigin(event: MouseEvent) {
		const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
		originX = ((event.clientX - rect.left) / rect.width) * 100;
		originY = ((event.clientY - rect.top) / rect.height) * 100;
	}

	function onStageDoubleClick(event: MouseEvent) {
		if (isMobile) return;
		updateOrigin(event);
		zoomed = !zoomed;
	}

	function onStageMouseMove(event: MouseEvent) {
		if (zoomed) updateOrigin(event);
	}

	function onStageClick(event: MouseEvent) {
		if (imgStatus === 'error') {
			blobImages[index]?.retry();
			return;
		}
		// Mobile: a tap toggles immersive mode (hide all chrome). Desktop: tapping
		// the letterbox around the image (anything but the photo) closes.
		if (isMobile) {
			immersive = !immersive;
		} else if (!zoomed && !(event.target instanceof HTMLImageElement)) {
			onClose();
		}
	}

	function trapFocus(event: KeyboardEvent) {
		if (!rootEl) return;
		const focusables = Array.from(
			rootEl.querySelectorAll<HTMLElement>('button'),
		);
		if (focusables.length === 0) return;
		const first = focusables[0];
		const last = focusables[focusables.length - 1];
		const active = document.activeElement;
		const inside = active instanceof HTMLElement && rootEl.contains(active);
		if (event.shiftKey && (!inside || active === first)) {
			event.preventDefault();
			last.focus();
		} else if (!event.shiftKey && (!inside || active === last)) {
			event.preventDefault();
			first.focus();
		}
	}

	function onKeydown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			event.preventDefault();
			if (zoomed) {
				zoomed = false;
			} else {
				onClose();
			}
		} else if (event.key === 'ArrowLeft') {
			event.preventDefault();
			select(index - 1);
		} else if (event.key === 'ArrowRight') {
			event.preventDefault();
			select(index + 1);
		} else if (event.key === 'Tab') {
			trapFocus(event);
		}
	}
</script>

<svelte:window onkeydown={onKeydown} />

<div
	class="fixed inset-0 z-50 bg-black"
	use:darkOverlay
	role="dialog"
	aria-modal="true"
	aria-label={photo.name}
	bind:this={rootEl}
	data-testid="lightbox"
>
	<!-- The stage fills the whole screen so the photo centres in the full
	     viewport; the header and footer controls overlay the letterbox gaps. -->
	<ImageCarousel
		bind:index
		items={photos}
		paused={zoomed}
		class="absolute inset-0 cursor-default"
		onclick={onStageClick}
		ondblclick={onStageDoubleClick}
		onmousemove={onStageMouseMove}
	>
		{#snippet slide(p, i)}
			<BlobImage
				bind:this={blobImages[i]}
				item={p}
				alt={p.name}
				lazy={i !== index}
				imgClass={`lightbox-image max-h-full max-w-full object-contain${zoomed && i === index ? ' zoomed' : ''}`}
				imgStyle={i === index
					? `transform-origin: ${originX}% ${originY}%`
					: ''}
				onStatus={s => (statuses[i] = s)}
			/>
		{/snippet}
	</ImageCarousel>

	<div
		class="lightbox-header absolute inset-x-0 top-0 flex items-center justify-between bg-black/40 px-3"
		class:faded={chromeHidden}
	>
		<div class="flex min-w-0 items-center gap-2">
			{#if isAndroid}
				<IconButton
					icon={mdiArrowBack}
					onClick={onClose}
					label={m.closeLightbox()}
					testid="lightbox-back"
					class="!p-2 opacity-85 hover:opacity-100"
				/>
			{/if}
			<div class="flex min-w-0 flex-col">
				<span
					class="overflow-hidden text-[16px] font-bold text-ellipsis whitespace-nowrap text-white"
					>{senderName}</span
				>
				<MessageTimestamp {timestamp} class="lightbox-time" />
			</div>
		</div>
		<div class="flex items-center gap-2">
			<IconButton
				icon={mdiTrayArrowDown}
				onClick={handleSave}
				label={m.saveFile()}
				testid="lightbox-save"
				class="!p-2 opacity-85 hover:opacity-100"
			/>
			{#if !isAndroid}
				<IconButton
					icon={mdiClose}
					onClick={onClose}
					label={m.closeLightbox()}
					testid="lightbox-close"
					class="!p-2 opacity-85 hover:opacity-100"
				/>
			{/if}
		</div>
	</div>

	<!-- Arrows are a desktop (mouse) affordance; on mobile you swipe between
	     photos. Physical left/right positioning keeps reading order even in RTL,
	     matching platform image-viewer conventions. -->
	{#if !isMobile && index > 0}
		<IconButton
			icon={mdiChevronLeft}
			onClick={() => select(index - 1)}
			label={m.previousPhoto()}
			testid="lightbox-prev"
			circle
			class="absolute top-1/2 left-3 -translate-y-1/2 opacity-85 hover:opacity-100 {zoomed
				? '!opacity-0 pointer-events-none'
				: ''}"
		/>
	{/if}
	{#if !isMobile && index < photos.length - 1}
		<IconButton
			icon={mdiChevronRight}
			onClick={() => select(index + 1)}
			label={m.nextPhoto()}
			testid="lightbox-next"
			circle
			class="absolute top-1/2 right-3 -translate-y-1/2 opacity-85 hover:opacity-100 {zoomed
				? '!opacity-0 pointer-events-none'
				: ''}"
		/>
	{/if}

	{#if isMobile || photos.length > 1}
		<div
			class="lightbox-bottom-bar absolute inset-x-0 bottom-0 bg-black/40 pb-[env(safe-area-inset-bottom)]"
			class:faded={chromeHidden}
		>
			{#if photos.length > 1}
				<LightboxThumbnailStrip {photos} bind:index />
			{/if}
			{#if isMobile}
				<div class="flex px-3 pt-3 pb-2">
					<ShareButton
						onClick={handleShare}
						testid="lightbox-share"
						class="!p-2 opacity-85 hover:opacity-100"
					/>
				</div>
			{/if}
		</div>
	{/if}
</div>

<style>
	.lightbox-header {
		height: calc(72px + env(safe-area-inset-top, 0px));
		padding-top: env(safe-area-inset-top, 0px);
		transition: opacity 0.15s ease;
	}

	.lightbox-header :global(.lightbox-time) {
		color: #b8b8b8;
		font-size: 11px;
	}

	:global(.lightbox-image) {
		transition: transform 0.15s ease;
	}
	:global(.lightbox-image.zoomed) {
		transform: scale(3);
		cursor: zoom-out;
	}
	:global(.lightbox-image:not(.zoomed)) {
		cursor: zoom-in;
	}

	.lightbox-bottom-bar {
		transition: opacity 0.15s ease;
	}

	.faded {
		opacity: 0;
		pointer-events: none;
	}
</style>
