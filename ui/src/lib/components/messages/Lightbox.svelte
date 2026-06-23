<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import {
		mdiChevronLeft,
		mdiChevronRight,
		mdiClose,
		mdiTrayArrowDown,
	} from '@mdi/js';
	import type { Photo } from 'dash-chat-stores';
	import { objectUrl } from '$lib/actions/object-url';
	import { savePhoto } from '$lib/utils/media';
	import { showToast } from '$lib/utils/toasts';
	import { setSystemBarsStyle, applyThemeSystemBars } from '$lib/utils/theme';
	import IconButton from '$lib/components/IconButton.svelte';
	import MessageTimestamp from './MessageTimestamp.svelte';

	interface Props {
		photos: Photo[];
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
	let stageEl: HTMLElement | undefined = $state();

	let zoomed = $state(false);
	let originX = $state(50);
	let originY = $state(50);

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

	// The overlay's top is always a dark image, so force a light status bar while
	// it is open (the navigation bar follows the theme), and restore on close.
	$effect(() => {
		const dark = document.documentElement.classList.contains('dark');
		setSystemBarsStyle('light', dark ? 'light' : 'dark').catch(() => {});
		return () => {
			applyThemeSystemBars().catch(() => {});
		};
	});

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
		if (!stageEl) return;
		const rect = stageEl.getBoundingClientRect();
		originX = ((event.clientX - rect.left) / rect.width) * 100;
		originY = ((event.clientY - rect.top) / rect.height) * 100;
	}

	function onStageDoubleClick(event: MouseEvent) {
		updateOrigin(event);
		zoomed = !zoomed;
	}

	function onStageMouseMove(event: MouseEvent) {
		if (zoomed) updateOrigin(event);
	}

	function onStageClick(event: MouseEvent) {
		if (event.target === stageEl && !zoomed) onClose();
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
	class="fixed inset-0 z-30 flex flex-col bg-black text-white"
	role="dialog"
	aria-modal="true"
	aria-label={photo.name}
	bind:this={rootEl}
	data-testid="lightbox"
>
	<div
		class="lightbox-header flex shrink-0 items-center justify-between px-3"
		class:faded={zoomed}
	>
		<div class="flex min-w-0 flex-col">
			<span
				class="overflow-hidden text-[13px] font-bold text-ellipsis whitespace-nowrap text-white"
				>{senderName}</span
			>
			<MessageTimestamp {timestamp} class="lightbox-time" />
		</div>
		<div class="flex items-center gap-2">
			<IconButton
				icon={mdiTrayArrowDown}
				onClick={handleSave}
				label={m.saveFile()}
				testid="lightbox-save"
				class="!p-2 opacity-85 hover:opacity-100"
			/>
			<IconButton
				icon={mdiClose}
				onClick={onClose}
				label={m.closeLightbox()}
				testid="lightbox-close"
				class="!p-2 opacity-85 hover:opacity-100"
			/>
		</div>
	</div>

	<button
		type="button"
		class="flex min-h-0 flex-1 cursor-default items-center justify-center overflow-hidden border-none bg-transparent p-0"
		bind:this={stageEl}
		aria-label={m.closeLightbox()}
		onclick={onStageClick}
		ondblclick={onStageDoubleClick}
		onmousemove={onStageMouseMove}
	>
		<img
			class="lightbox-image max-h-full max-w-full object-contain"
			class:zoomed
			style="transform-origin: {originX}% {originY}%"
			use:objectUrl={{ data: photo.data, mimeType: photo.mime_type }}
			alt={photo.name}
			data-testid="lightbox-image"
		/>
	</button>

	<!-- Physical left/right positioning: photo navigation keeps reading order
	     even in RTL, matching platform image-viewer conventions. -->
	{#if index > 0}
		<IconButton
			icon={mdiChevronLeft}
			onClick={() => select(index - 1)}
			label={m.previousPhoto()}
			testid="lightbox-prev"
			class="absolute top-1/2 left-3 -translate-y-1/2 !bg-white/10 !p-2 opacity-85 hover:!bg-white/20 hover:opacity-100 {zoomed
				? '!opacity-0 pointer-events-none'
				: ''}"
		/>
	{/if}
	{#if index < photos.length - 1}
		<IconButton
			icon={mdiChevronRight}
			onClick={() => select(index + 1)}
			label={m.nextPhoto()}
			testid="lightbox-next"
			class="absolute top-1/2 right-3 -translate-y-1/2 !bg-white/10 !p-2 opacity-85 hover:!bg-white/20 hover:opacity-100 {zoomed
				? '!opacity-0 pointer-events-none'
				: ''}"
		/>
	{/if}

	{#if photos.length > 1}
		<div
			class="lightbox-filmstrip flex shrink-0 justify-center gap-2 overflow-x-auto px-3 pt-2.5"
			class:faded={zoomed}
			data-testid="lightbox-filmstrip"
		>
			{#each photos as p, i (i)}
				<button
					type="button"
					class="lightbox-thumb h-11 w-11 shrink-0 overflow-hidden p-0"
					class:selected={i === index}
					data-testid="lightbox-thumb-{i}"
					aria-label={p.name}
					onclick={() => select(i)}
				>
					<img
						use:objectUrl={{ data: p.data, mimeType: p.mime_type }}
						alt={p.name}
						class="block h-full w-full object-cover"
					/>
				</button>
			{/each}
		</div>
	{/if}
</div>

<style>
	.lightbox-header {
		height: calc(52px + env(safe-area-inset-top, 0px));
		padding-top: env(safe-area-inset-top, 0px);
		transition: opacity 0.15s ease;
	}

	.lightbox-header :global(.lightbox-time) {
		color: #b8b8b8;
		font-size: 11px;
	}

	.lightbox-image {
		transition: transform 0.15s ease;
	}
	.lightbox-image.zoomed {
		transform: scale(3);
		cursor: zoom-out;
	}
	.lightbox-image:not(.zoomed) {
		cursor: zoom-in;
	}

	.lightbox-filmstrip {
		padding-bottom: calc(0.625rem + env(safe-area-inset-bottom, 0px));
		transition: opacity 0.15s ease;
	}

	.lightbox-thumb {
		border: none;
		border-radius: 6px;
		cursor: pointer;
		background: transparent;
		opacity: 0.7;
		transition: opacity 0.15s ease;
	}
	.lightbox-thumb:hover {
		opacity: 1;
	}
	.lightbox-thumb.selected {
		opacity: 1;
		outline: 2px solid white;
		outline-offset: -2px;
	}

	.faded {
		opacity: 0;
		pointer-events: none;
	}
</style>
