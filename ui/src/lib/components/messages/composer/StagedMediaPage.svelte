<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { Sheet, Block } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiClose, mdiArrowRight } from '@mdi/js';
	import { type DraftMedia } from '$lib/utils/media';
	import { isAndroid } from '$lib/utils/environment';
	import { setLightSystemBars, applyThemeSystemBars } from '$lib/utils/theme';
	import IconButton from '$lib/components/IconButton.svelte';
	import ExtensionSheet from '$lib/components/ExtensionSheet.svelte';
	import StagedPhotosCarousel from '$lib/components/messages/composer/StagedPhotosCarousel.svelte';
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
	let showEmojiPicker = $state(false);

	const photos = $derived(media?.kind === 'photos' ? media.items : []);
	const ariaLabel = $derived(
		media?.kind === 'file' ? media.file.name : (photos[index]?.name ?? ''),
	);

	// The overlay's top is always a dark image, so force a light status bar while
	// it is open (the navigation bar follows the theme), and restore on close.
	$effect(() => {
		setLightSystemBars().catch(() => {});
		return () => {
			applyThemeSystemBars().catch(() => {});
		};
	});

	function onKeydown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			event.preventDefault();
			onClose();
		}
	}
</script>

<svelte:window onkeydown={onKeydown} />

<div
	class="dark fixed inset-0 z-30 flex flex-col bg-black"
	role="dialog"
	aria-modal="true"
	aria-label={ariaLabel}
	data-testid="staged-media-page"
>
	<div class="relative flex min-h-0 flex-1 flex-col overflow-hidden pt-safe-12">
		<div
			class="staged-header absolute inset-x-0 z-10 flex items-center gap-2 px-2"
		>
			{#if !isAndroid}
				<IconButton
					icon={mdiClose}
					onClick={onClose}
					label={m.close()}
					testid="staged-media-close"
					class="!p-2 !text-white opacity-85 hover:opacity-100"
				/>
			{/if}
			{#if destinationName}
				<div
					class="ms-auto me-2 flex min-w-0 items-center gap-1.5 rounded-full bg-[#3a3a3c] px-3 py-1.5"
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

		{#if media?.kind === 'photos'}
			<StagedPhotosCarousel bind:media bind:index {onAddMore} {onClose} />
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
		<div class="row gap-3 px-4 pt-3 pb-3" style="align-items: center;">
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
			<SendButton onClick={onSend} />
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
	.staged-header {
		height: 52px;
	}

	.dir-arrow {
		display: inline-flex;
	}
	:global([dir='rtl']) .dir-arrow {
		transform: scaleX(-1);
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
</style>
