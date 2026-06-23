<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { tick } from 'svelte';
	import { Sheet, Block } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import {
		mdiClose,
		mdiPlus,
		mdiTrashCanOutline,
		mdiArrowRight,
	} from '@mdi/js';
	import { type DraftMedia, MAX_STAGED_PHOTOS } from '$lib/utils/media';
	import { objectUrl } from '$lib/actions/object-url';
	import IconButton from '$lib/components/IconButton.svelte';
	import ExtensionSheet from '$lib/components/ExtensionSheet.svelte';
	import MessageInput from '$lib/components/messages/composer/MessageInput.svelte';
	import SendButton from '$lib/components/messages/composer/SendButton.svelte';
	import EmojiPickerWrapper from '$lib/components/messages/EmojiPickerWrapper.svelte';

	interface Props {
		media: DraftMedia | undefined;
		value?: string;
		/** Name of the chat the media will be sent to, shown in the header. */
		destinationName?: string;
		onSend: () => void;
		onAddMore: () => void;
		onClose: () => void;
	}

	let {
		media = $bindable(),
		value = $bindable(''),
		destinationName,
		onSend,
		onAddMore,
		onClose,
	}: Props = $props();

	let index = $state(0);
	let carouselEl: HTMLElement | undefined = $state();
	let showEmojiPicker = $state(false);

	const photos = $derived(media?.kind === 'photos' ? media.items : []);
	const ariaLabel = $derived(
		media?.kind === 'file' ? media.file.name : (photos[index]?.name ?? ''),
	);

	// Keep the selected index in range as photos are added or removed.
	$effect(() => {
		if (index > photos.length - 1) index = Math.max(0, photos.length - 1);
	});

	function scrollToIndex(i: number, smooth = true) {
		if (!carouselEl) return;
		carouselEl.scrollTo({
			left: i * carouselEl.clientWidth,
			behavior: smooth ? 'smooth' : 'auto',
		});
	}

	function select(i: number) {
		index = Math.max(0, Math.min(photos.length - 1, i));
		scrollToIndex(index);
	}

	function onCarouselScroll() {
		if (!carouselEl || carouselEl.clientWidth === 0) return;
		const i = Math.round(carouselEl.scrollLeft / carouselEl.clientWidth);
		if (i !== index && i >= 0 && i < photos.length) index = i;
	}

	async function removePhoto(i: number) {
		if (media?.kind !== 'photos') return;
		const remaining = media.items.filter((_, j) => j !== i);
		if (remaining.length === 0) {
			onClose();
			return;
		}
		media = { kind: 'photos', items: remaining };
		await tick();
		index = Math.min(index, remaining.length - 1);
		scrollToIndex(index, false);
	}

	function onKeydown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			event.preventDefault();
			onClose();
			return;
		}
		// Don't hijack cursor movement while typing the caption.
		if (event.target instanceof HTMLTextAreaElement) return;
		if (event.key === 'ArrowLeft') {
			event.preventDefault();
			select(index - 1);
		} else if (event.key === 'ArrowRight') {
			event.preventDefault();
			select(index + 1);
		}
	}
</script>

<svelte:window onkeydown={onKeydown} />

<div
	class="staged-root fixed inset-0 z-30 flex flex-col bg-black text-white"
	role="dialog"
	aria-modal="true"
	aria-label={ariaLabel}
	data-testid="staged-media-page"
>
	<div class="staged-header flex shrink-0 items-center gap-2 px-2">
		<IconButton
			icon={mdiClose}
			onClick={onClose}
			label={m.close()}
			testid="staged-media-close"
			class="!p-2 !text-white opacity-85 hover:opacity-100"
		/>
		{#if destinationName}
			<div
				class="ms-auto me-2 flex min-w-0 items-center gap-1.5 rounded-full bg-white/15 px-3 py-1.5"
				data-testid="staged-media-destination"
			>
				<wa-icon
					class="dir-arrow shrink-0 text-lg"
					src={wrapPathInSvg(mdiArrowRight)}
				></wa-icon>
				<span class="truncate text-sm font-medium">{destinationName}</span>
			</div>
		{/if}
	</div>

	<div class="flex min-h-0 flex-1 flex-col overflow-hidden">
		{#if media?.kind === 'photos'}
			<div
				class="carousel flex min-h-0 flex-1 snap-x snap-mandatory overflow-x-auto"
				bind:this={carouselEl}
				onscroll={onCarouselScroll}
			>
				{#each photos as photo (photo)}
					<div
						class="flex w-full shrink-0 snap-center items-center justify-center px-4 py-2"
					>
						<img
							class="max-h-full max-w-full rounded-2xl object-contain"
							use:objectUrl={photo}
							alt={photo.name}
						/>
					</div>
				{/each}
			</div>
		{:else if media?.kind === 'file'}
			<div
				class="flex min-h-0 flex-1 flex-col items-center justify-center gap-3 px-8 text-center"
			>
				<ExtensionSheet name={media.file.name} width={72} height={90} />
				<span
					class="break-all text-sm text-white"
					data-testid="staged-media-file-name">{media.file.name}</span
				>
			</div>
		{/if}
	</div>

	<div class="staged-footer shrink-0 pb-safe">
		{#if media?.kind === 'photos'}
			<div
				class="flex justify-start gap-3 overflow-x-auto px-4 pt-3 pb-3"
				data-testid="staged-media-strip"
			>
				{#each photos as photo, i (photo)}
					<div
						class="staged-thumb relative h-14 w-14 shrink-0 overflow-hidden"
						class:selected={i === index}
					>
						<button
							type="button"
							class="block h-full w-full p-0"
							data-testid="staged-media-thumb-{i}"
							aria-label={photo.name}
							onclick={() => select(i)}
						>
							<img
								use:objectUrl={photo}
								alt={photo.name}
								class="block h-full w-full object-cover"
							/>
						</button>
						{#if i === index}
							<div
								class="absolute inset-0 flex items-center justify-center bg-black/45"
							>
								<IconButton
									icon={mdiTrashCanOutline}
									onClick={() => removePhoto(i)}
									label={m.removeAttachment()}
									testid="staged-media-remove-{i}"
									iconClass="text-xl"
									class="!text-white opacity-100"
								/>
							</div>
						{/if}
					</div>
				{/each}
				{#if photos.length < MAX_STAGED_PHOTOS}
					<button
						type="button"
						class="staged-add-more flex h-14 w-14 shrink-0 items-center justify-center"
						data-testid="staged-media-add-more"
						aria-label={m.addMoreAttachments()}
						onclick={onAddMore}
					>
						<wa-icon src={wrapPathInSvg(mdiPlus)}></wa-icon>
					</button>
				{/if}
			</div>
		{/if}

		<div class="row gap-3 px-4 pb-3" style="align-items: center;">
			<div
				class="input-container flex min-h-[42px] min-w-0 flex-1 items-center ps-2"
			>
				<MessageInput
					bind:value
					placeholder={m.typeMessage()}
					{onSend}
					onEmojiClick={() => (showEmojiPicker = true)}
				/>
			</div>
			<SendButton disabled={false} onClick={onSend} />
		</div>
	</div>

	<Sheet
		class="pb-safe text-lg"
		opened={showEmojiPicker}
		onBackdropClick={() => (showEmojiPicker = false)}
	>
		<div class="flex flex-col items-center">
			<div class="sheet-handle"></div>
		</div>
		<Block>
			<EmojiPickerWrapper
				onEmojiSelected={emoji => {
					value += emoji;
					showEmojiPicker = false;
				}}
			></EmojiPickerWrapper>
		</Block>
	</Sheet>
</div>

<style>
	.staged-root {
		/* The overlay is always dark (Signal-style), independent of app theme;
		   pin the Konsta text colors the reused MessageInput reads. */
		--k-text-color: #fff;
		--k-list-input-placeholder-color: rgba(255, 255, 255, 0.55);
	}

	.staged-header {
		height: calc(52px + env(safe-area-inset-top, 0px));
		padding-top: env(safe-area-inset-top, 0px);
	}

	.dir-arrow {
		display: inline-flex;
	}
	:global([dir='rtl']) .dir-arrow {
		transform: scaleX(-1);
	}

	.carousel {
		scrollbar-width: none;
	}
	.carousel::-webkit-scrollbar {
		display: none;
	}

	.input-container {
		border: 1px solid rgba(255, 255, 255, 0.16);
		border-radius: 22px;
		background: rgba(255, 255, 255, 0.1);
		transition: border-color 0.15s ease;
	}
	.input-container:focus-within {
		border-color: var(--color-brand-primary);
	}

	.staged-thumb {
		border-radius: 12px;
		background: rgba(255, 255, 255, 0.08);
	}
	.staged-thumb.selected {
		outline: 2px solid var(--color-brand-primary);
		outline-offset: -2px;
	}

	.staged-add-more {
		border-radius: 12px;
		border: none;
		background: rgba(255, 255, 255, 0.1);
		cursor: pointer;
		color: white;
		opacity: 0.75;
		transition: opacity 0.15s ease;
	}
	.staged-add-more:hover {
		opacity: 1;
	}
	.staged-add-more :global(wa-icon) {
		width: 24px;
		height: 24px;
	}
</style>
