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
		mdiClose,
	} from '@mdi/js';
	import { useTheme } from 'konsta/svelte';
	import { isIos } from '$lib/utils/environment';
	import {
		type DraftMedia,
		makeDraftPhotos,
		revokeDraft,
		formatFileSize,
	} from '$lib/types/media';

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
		if (media) revokeDraft(media);
		onMediaChange?.(makeDraftPhotos(photoPicker.files));
		photoPicker.value = '';
		attachMenuOpen = false;
	}

	function onFilePicked() {
		if (!filePicker.files || !filePicker.files[0]) return;
		if (media) revokeDraft(media);
		onMediaChange?.({ kind: 'file', file: filePicker.files[0] });
		filePicker.value = '';
		attachMenuOpen = false;
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
	<div
		class="message-input-bar m-2 pb-safe"
		class:bg-md-light-surface={theme === 'material'}
		class:dark:bg-md-dark-surface={theme === 'material'}
	>
		<input
			type="file"
			accept="image/*,video/*"
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
			<div class="media-preview" data-testid="message-input-media-preview">
				{#if media.kind === 'photos'}
					<div class="photo-row">
						{#each media.items as photo, i (photo.previewUrl)}
							<div class="photo-thumb">
								<img src={photo.previewUrl} alt={photo.file.name} />
								<button
									type="button"
									class="thumb-remove"
									aria-label={m.removeAttachment()}
									onclick={() => removePhoto(i)}
								>
									<wa-icon src={wrapPathInSvg(mdiClose)}></wa-icon>
								</button>
							</div>
						{/each}
					</div>
				{:else}
					<div class="file-pill">
						<wa-icon src={wrapPathInSvg(mdiFile)} class="file-pill-icon"
						></wa-icon>
						<div class="file-pill-info">
							<span class="file-pill-name">{media.file.name}</span>
							<span class="file-pill-size"
								>{formatFileSize(media.file.size)}</span
							>
						</div>
						<button
							type="button"
							class="thumb-remove"
							aria-label={m.removeAttachment()}
							onclick={removeMedia}
						>
							<wa-icon src={wrapPathInSvg(mdiClose)}></wa-icon>
						</button>
					</div>
				{/if}
			</div>
		{/if}

		<div class="row gap-2" style="align-items: flex-end; margin: 0 auto">
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

			<div
				class={theme === 'ios'
					? 'input-container bg-ios-light-glass shadow-ios-light-glass backdrop-blur-lg dark:bg-ios-dark-glass dark:shadow-ios-dark-glass'
					: 'input-container bg-white dark:bg-gray-800'}
			>
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

				<textarea
					class="message-textarea"
					data-testid="message-input-textarea"
					{placeholder}
					bind:value
					bind:this={textarea}
					rows="1"
					onkeydown={handleKeydown}
					oninput={handleInput}
				></textarea>
			</div>

			<button
				type="button"
				class="send-button"
				data-testid="message-input-send"
				class:active={hasContent}
				onclick={handleSendClick}
				disabled={!hasContent}
				aria-label="Send"
			>
				<wa-icon src={wrapPathInSvg(mdiSend)}></wa-icon>
			</button>
		</div>
	</div>
</div>

<style>
	.input-container {
		flex: 1;
		display: flex;
		align-items: flex-end;
		min-width: 0;
		border: 1px solid var(--k-hairline-color);
		border-radius: 22px;
		padding: 4px 4px 4px 6px;
		transition: border-color 0.15s ease;
	}

	.input-container:focus-within {
		border-color: var(--k-theme-color, #3b82f6);
	}

	.icon-button {
		flex-shrink: 0;
		width: 36px;
		height: 36px;
		border: none;
		background: transparent;
		border-radius: 50%;
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		color: var(--k-text-color);
		opacity: 0.5;
		transition:
			opacity 0.15s ease,
			background-color 0.15s ease;
		padding: 0;
	}

	.icon-button:hover {
		opacity: 0.8;
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
		font-size: 16px;
		line-height: 1.375;
		padding: 8px 8px;
		color: var(--k-text-color);
		font-family: inherit;
		min-height: 20px;
		max-height: 100px;
		overflow-y: auto;
	}

	.message-textarea::placeholder {
		color: var(--k-list-input-placeholder-color);
		opacity: 0.6;
	}

	.send-button {
		flex-shrink: 0;
		width: 40px;
		height: 40px;
		border: none;
		border-radius: 50%;
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		padding: 0;
		margin-bottom: 4px;
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
	.icon-button :global(wa-icon),
	.send-button :global(wa-icon) {
		width: 22px;
		height: 22px;
	}

	.send-button :global(wa-icon) {
		margin-inline-start: 2px; /* Optical centering for send arrow */
	}

	/* Attach button + popover */
	.attach-wrapper {
		position: relative;
		align-self: flex-end;
		margin-bottom: 4px;
	}

	.attach-button {
		width: 40px;
		height: 40px;
		opacity: 0.6;
	}
	.attach-button:hover {
		opacity: 0.85;
	}

	.attach-scrim {
		position: fixed;
		inset: 0;
		z-index: 10;
	}

	.attach-menu {
		position: absolute;
		bottom: calc(100% + 8px);
		inset-inline-start: 0;
		z-index: 20;
		min-width: 200px;
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
		gap: 12px;
		width: 100%;
		padding: 10px 16px;
		border: none;
		background: transparent;
		cursor: pointer;
		font-size: 15px;
		color: var(--k-text-color);
		transition: background-color 0.1s ease;
	}

	.attach-menu-item:hover {
		background: rgba(128, 128, 128, 0.12);
	}

	.attach-menu-item :global(wa-icon) {
		width: 22px;
		height: 22px;
		opacity: 0.75;
	}

	/* Media preview */
	.media-preview {
		padding: 8px 8px 4px;
	}

	.photo-row {
		display: flex;
		gap: 6px;
		overflow-x: auto;
		padding-bottom: 4px;
	}

	.photo-thumb {
		position: relative;
		flex-shrink: 0;
		width: 72px;
		height: 72px;
		border-radius: 8px;
		overflow: hidden;
	}

	.photo-thumb img {
		width: 100%;
		height: 100%;
		object-fit: cover;
		display: block;
	}

	.thumb-remove {
		position: absolute;
		top: -6px;
		inset-inline-end: -6px;
		width: 22px;
		height: 22px;
		border-radius: 50%;
		border: none;
		background: rgba(0, 0, 0, 0.65);
		color: white;
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		padding: 0;
	}

	.thumb-remove :global(wa-icon) {
		width: 14px;
		height: 14px;
	}

	.file-pill {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 10px 14px;
		border-radius: 12px;
		background: rgba(128, 128, 128, 0.12);
		position: relative;
	}

	.file-pill :global(.file-pill-icon) {
		width: 28px;
		height: 28px;
		opacity: 0.65;
		flex-shrink: 0;
	}

	.file-pill-info {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 1px;
	}

	.file-pill-name {
		font-size: 14px;
		font-weight: 500;
		color: var(--k-text-color);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.file-pill-size {
		font-size: 12px;
		color: var(--k-text-color);
		opacity: 0.6;
	}
</style>
