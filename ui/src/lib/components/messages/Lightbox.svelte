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
	import { bytesToBlobUrl } from '$lib/types/media';
	import { saveAttachment } from '$lib/utils/save-file';
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

	// Own object URLs — minted and revoked in the same pre-effect; see
	// MessageAttachment.
	let photoUrls = $state<string[]>([]);

	$effect.pre(() => {
		const urls = photos.map(p => bytesToBlobUrl(p.data, p.mime_type));
		photoUrls = urls;
		return () => urls.forEach(u => URL.revokeObjectURL(u));
	});

	let rootEl: HTMLElement | undefined = $state();
	let stageEl: HTMLElement | undefined = $state();
	let closeButton: HTMLButtonElement | undefined = $state();

	let zoomed = $state(false);
	let originX = $state(50);
	let originY = $state(50);

	function select(i: number) {
		index = Math.max(0, Math.min(photos.length - 1, i));
	}

	// Reset zoom when switching photos.
	$effect(() => {
		void index;
		zoomed = false;
	});

	$effect(() => {
		closeButton?.focus();
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
	class="lightbox"
	role="dialog"
	aria-modal="true"
	aria-label={photo.name}
	bind:this={rootEl}
	data-testid="lightbox"
>
	<div class="lightbox-header" class:faded={zoomed}>
		<div class="lightbox-header-info">
			<span class="lightbox-sender">{senderName}</span>
			<MessageTimestamp {timestamp} class="lightbox-time" />
		</div>
		<div class="lightbox-header-actions">
			<button
				type="button"
				class="lightbox-button"
				data-testid="lightbox-save"
				aria-label={m.saveFile()}
				onclick={() => saveAttachment(photo)}
			>
				<wa-icon src={wrapPathInSvg(mdiTrayArrowDown)}></wa-icon>
			</button>
			<button
				type="button"
				class="lightbox-button"
				data-testid="lightbox-close"
				aria-label={m.closeLightbox()}
				bind:this={closeButton}
				onclick={onClose}
			>
				<wa-icon src={wrapPathInSvg(mdiClose)}></wa-icon>
			</button>
		</div>
	</div>

	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="lightbox-stage"
		bind:this={stageEl}
		onclick={onStageClick}
		ondblclick={onStageDoubleClick}
		onmousemove={onStageMouseMove}
	>
		<img
			class="lightbox-image"
			class:zoomed
			style="transform-origin: {originX}% {originY}%"
			src={photoUrls[index]}
			alt={photo.name}
			data-testid="lightbox-image"
		/>
	</div>

	{#if index > 0}
		<button
			type="button"
			class="lightbox-button lightbox-nav lightbox-prev"
			class:faded={zoomed}
			data-testid="lightbox-prev"
			aria-label={m.previousPhoto()}
			onclick={() => select(index - 1)}
		>
			<wa-icon src={wrapPathInSvg(mdiChevronLeft)}></wa-icon>
		</button>
	{/if}
	{#if index < photos.length - 1}
		<button
			type="button"
			class="lightbox-button lightbox-nav lightbox-next"
			class:faded={zoomed}
			data-testid="lightbox-next"
			aria-label={m.nextPhoto()}
			onclick={() => select(index + 1)}
		>
			<wa-icon src={wrapPathInSvg(mdiChevronRight)}></wa-icon>
		</button>
	{/if}

	{#if photos.length > 1}
		<div
			class="lightbox-filmstrip"
			class:faded={zoomed}
			data-testid="lightbox-filmstrip"
		>
			{#each photos as p, i (photoUrls[i])}
				<button
					type="button"
					class="lightbox-thumb"
					class:selected={i === index}
					data-testid="lightbox-thumb-{i}"
					aria-label={p.name}
					onclick={() => select(i)}
				>
					<img src={photoUrls[i]} alt={p.name} />
				</button>
			{/each}
		</div>
	{/if}
</div>

<style>
	.lightbox {
		position: fixed;
		inset: 0;
		z-index: 30;
		background: black;
		display: flex;
		flex-direction: column;
	}

	.lightbox-header {
		flex-shrink: 0;
		height: 52px;
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding-inline: 12px;
		transition: opacity 0.15s ease;
	}

	.lightbox-header-info {
		display: flex;
		flex-direction: column;
		min-width: 0;
	}

	.lightbox-sender {
		color: white;
		font-size: 13px;
		font-weight: 700;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.lightbox-header :global(.lightbox-time) {
		color: #b8b8b8;
		font-size: 11px;
	}

	.lightbox-header-actions {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.lightbox-button {
		border: none;
		background: transparent;
		color: white;
		cursor: pointer;
		padding: 6px;
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: 6px;
		opacity: 0.85;
		transition: opacity 0.15s ease;
	}
	.lightbox-button:hover {
		opacity: 1;
	}
	.lightbox-button :global(wa-icon) {
		width: 24px;
		height: 24px;
	}

	.lightbox-stage {
		flex: 1;
		min-height: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		overflow: hidden;
	}

	.lightbox-image {
		max-width: 100%;
		max-height: 100%;
		object-fit: contain;
		transition: transform 0.15s ease;
	}
	.lightbox-image.zoomed {
		transform: scale(3);
		cursor: zoom-out;
	}
	.lightbox-image:not(.zoomed) {
		cursor: zoom-in;
	}

	/* Physical positioning: photo navigation keeps reading order even in
	 * RTL, matching platform image-viewer conventions. */
	.lightbox-nav {
		position: absolute;
		top: 50%;
		transform: translateY(-50%);
		background: rgba(255, 255, 255, 0.12);
		border-radius: 50%;
		padding: 8px;
	}
	.lightbox-nav:hover {
		background: rgba(255, 255, 255, 0.22);
	}
	.lightbox-prev {
		left: 12px;
	}
	.lightbox-next {
		right: 12px;
	}

	.lightbox-filmstrip {
		flex-shrink: 0;
		display: flex;
		justify-content: center;
		gap: 8px;
		padding: 10px 12px;
		overflow-x: auto;
		transition: opacity 0.15s ease;
	}

	.lightbox-thumb {
		flex-shrink: 0;
		width: 44px;
		height: 44px;
		padding: 0;
		border: none;
		border-radius: 6px;
		overflow: hidden;
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
	.lightbox-thumb img {
		width: 100%;
		height: 100%;
		object-fit: cover;
		display: block;
	}

	.faded {
		opacity: 0;
		pointer-events: none;
	}
</style>
