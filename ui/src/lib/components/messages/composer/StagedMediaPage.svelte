<script lang="ts">
	import { onMount } from 'svelte';
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { Sheet, Block } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiClose, mdiArrowRight } from '@mdi/js';
	import { type DraftMedia } from '$lib/utils/media';
	import { isAndroid, isIos } from '$lib/utils/environment';
	import { lightSystemBars } from '$lib/actions/light-system-bars';
	import { keepKeyboardOpen } from '$lib/actions/keep-keyboard-open';
	import IconButton from '$lib/components/IconButton.svelte';
	import ExtensionSheet from '$lib/components/ExtensionSheet.svelte';
	import StagedPhotosCarousel from '$lib/components/messages/composer/StagedPhotosCarousel.svelte';
	import StagedPhotosStrip from '$lib/components/messages/composer/StagedPhotosStrip.svelte';
	import MessageInput from '$lib/components/messages/composer/MessageInput.svelte';
	import EmojiButton from '$lib/components/messages/composer/EmojiButton.svelte';
	import SendButton from '$lib/components/messages/composer/SendButton.svelte';
	import EmojiPickerWrapper from '$lib/components/messages/EmojiPickerWrapper.svelte';
	import { hideKeyboard } from 'tauri-plugin-virtual-keyboard';
	import { renderAboveKeyboard } from '$lib/utils/virtual-keyboard/render-above-keyboard';

	interface Props {
		media: DraftMedia | undefined;
		value?: string;
		/** Name of the chat the media will be sent to, shown in the header. */
		destinationName?: string;
		onSend: () => Promise<boolean>;
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

	// Blur explicitly on top of hideKeyboard(): after an activity round-trip
	// (camera capture) the plugin's open-state is stale, so hideKeyboard() can
	// no-op while the composer's input still holds focus — and the OS re-summons
	// the keyboard for a focused input when the window regains focus.
	onMount(() => {
		hideKeyboard();
		if (document.activeElement instanceof HTMLElement) {
			document.activeElement.blur();
		}
	});

	function onKeydown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			event.preventDefault();
			onClose();
		}
	}
</script>

<svelte:window onkeydown={onKeydown} />

{#snippet emojiButton()}
	<EmojiButton
		onClick={() => {
			hideKeyboard();
			showEmojiPicker = true;
		}}
	/>
{/snippet}

<div
	class="dark fixed inset-0 z-30 flex flex-col bg-black"
	use:lightSystemBars
	role="dialog"
	aria-modal="true"
	aria-label={ariaLabel}
	data-testid="staged-media-page"
>
	<div
		class="relative flex min-h-0 flex-1 flex-col overflow-hidden pt-safe-12 pb-keyboard-safe"
	>
		<div
			class="staged-header absolute inset-x-0 z-10 flex items-center gap-2 px-2"
		>
			{#if !isAndroid}
				<IconButton
					icon={mdiClose}
					onClick={onClose}
					label={m.close()}
					testid="staged-media-close"
					class="!text-white opacity-85 hover:!bg-white/10"
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
			<StagedPhotosCarousel bind:media bind:index />
		{:else if media?.kind === 'file'}
			<div
				class="flex min-h-0 flex-1 flex-col items-center justify-center px-8 text-center"
			>
				<div class="flex flex-col items-center gap-3" use:renderAboveKeyboard>
					<ExtensionSheet name={media.file.name} width={72} height={90} />
					<span
						class="break-all text-sm text-white"
						data-testid="staged-media-file-name">{media.file.name}</span
					>
				</div>
			</div>
		{/if}
	</div>

	<div
		class="staged-footer absolute inset-x-0 bottom-0 flex flex-col pb-keyboard-safe"
		use:renderAboveKeyboard
		use:keepKeyboardOpen
	>
		{#if media?.kind === 'photos'}
			<StagedPhotosStrip bind:media bind:index {onAddMore} {onClose} />
		{/if}
		<div class="row gap-3 px-4 pt-3 pb-3" style="align-items: center;">
			<MessageInput
				bind:value
				placeholder={m.typeMessage()}
				{onSend}
				before={isIos ? undefined : emojiButton}
			/>
			<SendButton {onSend} />
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
</style>
