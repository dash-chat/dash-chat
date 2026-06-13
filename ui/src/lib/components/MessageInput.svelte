<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import {
		mdiSend,
		mdiEmoticonHappyOutline,
		mdiPlus,
		mdiImage,
		mdiFile,
	} from '@mdi/js';
	import { useTheme } from 'konsta/svelte';
	import { isIos, isMobile } from '$lib/utils/environment';
	import { type DraftMedia, PHOTO_ACCEPT, revokeDraft } from '$lib/types/media';
	import { stageFiles } from '$lib/utils/stage-files';
	import StagedAttachments from '$lib/components/StagedAttachments.svelte';

	interface Props {
		value?: string;
		placeholder?: string;
		/** Composer-side draft. Owner is the parent; this component only
		 * proposes changes via `onMediaChange`. */
		media?: DraftMedia | undefined;
		onSend?: () => void;
		onEmojiClick?: () => void;
		onMediaChange?: (media: DraftMedia | undefined) => void;
	}

	let {
		value = $bindable(''),
		placeholder = m.typeMessage(),
		media = undefined,
		onSend,
		onEmojiClick,
		onMediaChange,
	}: Props = $props();

	const theme = $derived(useTheme());

	let hasContent = $derived(value.trim().length > 0 || media !== undefined);
	let textarea: HTMLTextAreaElement;
	let photoPicker: HTMLInputElement;
	let filePicker: HTMLInputElement;
	let attachMenuOpen = $state(false);

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === 'Enter' && !event.shiftKey) {
			event.preventDefault();
			triggerOnSend();
		}
	}

	function handleInput() {
		value = textarea.value;
		autoResize();
	}

	function autoResize() {
		if (textarea.scrollHeight > 100) return;
		textarea.style.height = 'auto';
		textarea.style.height = textarea.scrollHeight + 'px';
	}

	function handleSendClick() {
		triggerOnSend();
	}

	function triggerOnSend() {
		if (hasContent) {
			onSend?.();
			textarea.style.height = 'auto';
			textarea.focus();
		}
	}

	function keepKeyboardOpen(event: Event) {
		if (event.target !== textarea) {
			event.preventDefault();
		}
	}

	function onPhotosPicked() {
		if (!photoPicker.files || photoPicker.files.length === 0) return;
		onMediaChange?.(stageFiles(media, photoPicker.files));
		photoPicker.value = '';
		attachMenuOpen = false;
	}

	function onFilePicked() {
		if (!filePicker.files || !filePicker.files[0]) return;
		onMediaChange?.(stageFiles(media, filePicker.files));
		filePicker.value = '';
		attachMenuOpen = false;
	}

	function onPaste(event: ClipboardEvent) {
		const files = event.clipboardData?.files;
		if (!files || files.length === 0) return;
		event.preventDefault();
		onMediaChange?.(stageFiles(media, files));
	}

	function removeMedia() {
		if (media) revokeDraft(media);
		onMediaChange?.(undefined);
	}

	function removePhoto(index: number) {
		if (!media || media.kind !== 'photos') return;
		const removed = media.items[index];
		URL.revokeObjectURL(removed.previewUrl);
		const remaining = media.items.filter((_, i) => i !== index);
		onMediaChange?.(
			remaining.length > 0 ? { kind: 'photos', items: remaining } : undefined,
		);
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	style="display: flow-root"
	onmousedown={keepKeyboardOpen}
	ontouchstart={keepKeyboardOpen}
	onpointerdown={keepKeyboardOpen}
>
	<div class="message-input-bar m-2 pb-safe">
		<input
			type="file"
			accept={PHOTO_ACCEPT}
			multiple
			bind:this={photoPicker}
			class="hidden"
			data-testid="message-input-photo-picker"
			onchange={onPhotosPicked}
		/>
		<input
			type="file"
			bind:this={filePicker}
			class="hidden"
			data-testid="message-input-file-picker"
			onchange={onFilePicked}
		/>

		{#if media}
			<StagedAttachments
				{media}
				onRemovePhoto={removePhoto}
				onRemoveFile={removeMedia}
				onAddMore={() => photoPicker.click()}
				onClearAll={removeMedia}
			/>
		{/if}

		<div class="row gap-2" style="align-items: flex-end; margin: 0 auto">
			{#if onEmojiClick && !isIos}
				<button
					type="button"
					class="icon-button emoji-btn"
					onclick={onEmojiClick}
					aria-label="Emoji"
					data-testid="message-input-emoji"
				>
					<wa-icon src={wrapPathInSvg(mdiEmoticonHappyOutline)}></wa-icon>
				</button>
			{/if}

			<div
				class={theme === 'ios'
					? 'input-container bg-ios-light-glass shadow-ios-light-glass backdrop-blur-lg dark:bg-ios-dark-glass dark:shadow-ios-dark-glass'
					: 'input-container bg-gray-100 dark:bg-gray-800'}
			>
				<textarea
					class="message-textarea"
					data-testid="message-input-textarea"
					{placeholder}
					bind:value
					bind:this={textarea}
					rows="1"
					onkeydown={handleKeydown}
					oninput={handleInput}
					onpaste={onPaste}
				></textarea>
			</div>

			<div class="attach-wrapper">
				<button
					type="button"
					class="icon-button attach-button"
					data-testid="message-input-attach"
					aria-label={m.attachMenu()}
					aria-expanded={attachMenuOpen}
					onclick={() => (attachMenuOpen = !attachMenuOpen)}
				>
					<wa-icon src={wrapPathInSvg(mdiPlus)}></wa-icon>
				</button>
				{#if attachMenuOpen}
					<!-- svelte-ignore a11y_click_events_have_key_events -->
					<!-- svelte-ignore a11y_no_static_element_interactions -->
					<div
						class="attach-scrim"
						onclick={() => (attachMenuOpen = false)}
					></div>
					<div
						class="attach-menu"
						class:attach-menu-ios={theme === 'ios'}
						data-testid="message-input-attach-menu"
					>
						<button
							type="button"
							class="attach-menu-item"
							data-testid="message-input-attach-photos"
							onclick={() => {
								attachMenuOpen = false;
								photoPicker.click();
							}}
						>
							<wa-icon src={wrapPathInSvg(mdiImage)}></wa-icon>
							<span>{m.attachPhotos()}</span>
						</button>
						<button
							type="button"
							class="attach-menu-item"
							data-testid="message-input-attach-file"
							onclick={() => {
								attachMenuOpen = false;
								filePicker.click();
							}}
						>
							<wa-icon src={wrapPathInSvg(mdiFile)}></wa-icon>
							<span>{m.attachFile()}</span>
						</button>
					</div>
				{/if}
			</div>

			{#if isMobile}
				<button
					type="button"
					class="send-button"
					data-testid="message-input-send"
					class:active={hasContent}
					onclick={handleSendClick}
					disabled={!hasContent}
					aria-label="Send"
				>
					<wa-icon style="font-size: 24px" src={wrapPathInSvg(mdiSend)}
					></wa-icon>
				</button>
			{/if}
		</div>
	</div>
</div>

<style>
	.input-container {
		flex: 1;
		display: flex;
		align-items: center;
		min-width: 0;
		min-height: 42px;
		border: 1px solid var(--k-hairline-color);
		border-radius: 22px;
		transition: border-color 0.15s ease;
	}

	.input-container:focus-within {
		border-color: var(--k-theme-color, #3b82f6);
	}

	.icon-button {
		flex-shrink: 0;
		width: 34px;
		height: 34px;
		border: none;
		background: transparent;
		border-radius: 50%;
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		color: var(--k-text-color);
		transition: background-color 0.15s ease;
	}

	.icon-button:hover {
		background: rgba(128, 128, 128, 0.1);
	}

	.icon-button:active {
		background: rgba(128, 128, 128, 0.2);
	}

	.message-textarea {
		flex: 1;
		min-width: 0;
		border: none;
		outline: none;
		resize: none;
		line-height: 1.375;
		color: var(--k-text-color);
		font-family: inherit;
		min-height: 28px;
		padding: 8px 12px;
		max-height: 100px;
		overflow-y: auto;
	}

	.message-textarea::placeholder {
		color: var(--k-list-input-placeholder-color);
		opacity: 0.6;
	}

	.send-button {
		flex-shrink: 0;
		width: 42px;
		height: 42px;
		border: none;
		border-radius: 50%;
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		padding: 0;
		background: rgba(128, 128, 128, 0.15);
		color: var(--k-text-color);
		opacity: 0.4;
		transition:
			background-color 0.2s ease,
			opacity 0.2s ease,
			transform 0.1s ease;
	}

	.send-button:disabled {
		cursor: default;
	}

	.send-button.active {
		background: var(--k-theme-color, #3b82f6);
		color: white;
		opacity: 1;
	}

	.send-button.active:hover {
		filter: brightness(1.1);
	}

	.send-button.active:active {
		transform: scale(0.95);
	}

	/* Icon sizing */
	.icon-button :global(wa-icon) {
		width: 20px;
		height: 20px;
	}

	.send-button :global(wa-icon) {
		width: 22px;
		height: 22px;
		margin-inline-start: 2px; /* Optical centering for send arrow */
	}

	/* Emoji + attach buttons hug the bottom as the textarea grows. */
	.emoji-btn,
	.attach-wrapper {
		align-self: flex-end;
		margin-bottom: 4px;
	}

	/* Attach button + popover */
	.attach-wrapper {
		position: relative;
	}

	.attach-scrim {
		position: fixed;
		inset: 0;
		z-index: 10;
	}

	.attach-menu {
		position: absolute;
		bottom: calc(100% + 8px);
		inset-inline-end: 0;
		z-index: 20;
		min-width: 180px;
		border-radius: 14px;
		padding: 6px 0;
		background: var(--k-bars-bg-color, white);
		border: 1px solid var(--k-hairline-color);
		box-shadow: 0 6px 20px rgba(0, 0, 0, 0.15);
	}
	:global(.dark) .attach-menu {
		background: var(--k-bars-bg-color, #1c1c1e);
	}

	.attach-menu-item {
		display: flex;
		align-items: center;
		gap: 10px;
		width: 100%;
		padding: 8px 14px;
		border: none;
		background: transparent;
		cursor: pointer;
		font-size: 14px;
		color: var(--k-text-color);
		transition: background-color 0.1s ease;
	}

	.attach-menu-item:hover {
		background: rgba(128, 128, 128, 0.12);
	}

	.attach-menu-item :global(wa-icon) {
		width: 18px;
		height: 18px;
		opacity: 0.75;
	}
</style>
